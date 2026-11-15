//! OpenCLI document → Uniform `Reference[]`.
//!
//! A faithful Rust port of `opencliToReferences` from
//! `packages/opencli/src/converters.ts` (plus the slice of `generate.ts`'s
//! `generateUsage` it reaches). One Reference per command, so the docs engine
//! renders CLI reference pages the same way it renders OpenAPI/GraphQL
//! (`api.cli` → Atlas).
//!
//! The output is built as untyped `serde_json::Value` in the TypeScript's exact
//! key order, because the oracle is that package's frozen
//! `__fixtures__/references/**` goldens and JSON key order is observable there.
//!
//! Ported behaviors that are easy to get wrong (all golden-gated):
//! - `regions` is an EXACT-match filter on the space-joined command path.
//! - Root options with `recursive && !hidden` are the CLI's global options. By
//!   default they become ONE extra trailing reference (`global-options`); with
//!   [`OpencliToReferencesOptions::global_options_per_command`] they instead
//!   become a "Global options" definition on every command.
//! - Definition order is Arguments, Options, Inherited options, Commands,
//!   Global options, Interactive commands, Keys, Environment, Exit codes. Each
//!   appears only when it has rows.
//! - When the LAST required argument (reverse-find) enumerates `acceptedValues`,
//!   the "CLI Tool" group gets one example PER value, titled with it.
//!
//! What a page carries beyond a command's own arguments, options and
//! subcommands:
//! - "Inherited options": the recursive options of its ancestor COMMANDS,
//!   outermost first. A name resolves to its nearest declaration (the
//!   command's own, else the nearest ancestor's, recursive or not), and only
//!   a recursive, visible one is inherited. The root's are the global options
//!   above, not these; per command, a global option a nearer declaration
//!   shadows is left out.
//! - "Interactive commands" and "Keys", from `x-interactive`: the commands typed
//!   inside the session the command opens (each with its arguments and options
//!   as nested rows; they never get a page of their own) and its key bindings.
//! - "Environment", from `x-environment`.
//! - "Exit codes": the command's own `exitCodes`, else its nearest ancestor's,
//!   the root's last.
//! - Its `examples`, as an "Examples" sample group after the "CLI Tool" one.
//!
//! A command group with arguments or options of its own also runs without a
//! subcommand, so its placeholder reads `[command]` (title and usage) and its
//! CLI Tool sample leaves the placeholder out; a group without them keeps
//! `<command>`. `x-passthrough` changes nothing: the arguments, options and
//! subcommands under it document the program it hands over to, and render
//! like any other command's.
//!
//! The root gets a page of its own, before the commands, only when it
//! documents something of its own: arguments, examples, a session or
//! environment variables. Its region and canonical are its id
//! ([`root_page_id`]), the CLI's title as a URL path segment; its title and
//! usage keep the title itself. Its options that are not global render there
//! too. Options or exit codes alone do not make one: a root `--version` keeps
//! a document's shape, and the commands inherit the exit codes.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use apitoolchain_uniform::canon;

const GLOBAL_OPTIONS_REGION: &str = "global-options";

/// Options for [`opencli_to_references`]. Mirrors the TS
/// `OpencliToReferencesOptions`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OpencliToReferencesOptions {
    /// Restrict output to specific commands. Each region is a command path,
    /// space-joined from the CLI root (e.g. `"install"`, `"remote add"`).
    /// The root's own page, when there is one, is its id: the CLI's title as
    /// a URL path segment (`"acme"`, or `"acme-agent"` for the title
    /// `Acme Agent`). An empty list means "no filter" (matching the TS
    /// `?.length` guard).
    pub regions: Option<Vec<String>>,

    /// Render the CLI's global (root recursive) options on every command page.
    /// When false (the default), a single "Global options" reference is emitted
    /// instead — so the options appear once in the sidebar rather than repeated
    /// on every command.
    pub global_options_per_command: bool,
}

/// Convert an OpenCLI document to uniform `Reference[]` — one Reference per
/// command. Mirrors `oapSchemaToReferences`.
pub fn opencli_to_references(spec: &Value, options: &OpencliToReferencesOptions) -> Vec<Value> {
    if !truthy(Some(spec)) {
        return Vec::new();
    }

    let cli_title = match spec.pointer("/info/title").and_then(Value::as_str) {
        Some(t) if !t.is_empty() => t,
        _ => "cli",
    };

    // `options.regions?.length ? new Set(...) : null` — an empty array is no filter.
    let region_set: Option<&Vec<String>> = match &options.regions {
        Some(r) if !r.is_empty() => Some(r),
        _ => None,
    };
    let in_regions = |region: &str| match region_set {
        Some(set) => set.iter().any(|r| r == region),
        None => true,
    };

    let per_command = options.global_options_per_command;

    // Root-level recursive options (a CLI's global flags) apply to every command.
    let global_options: Vec<&Value> = array(spec.get("options"))
        .iter()
        .filter(|o| truthy(o.get("recursive")) && !truthy(o.get("hidden")))
        .collect();

    let mut refs: Vec<Value> = Vec::new();
    let per_command_globals: &[&Value] = if per_command { &global_options } else { &[] };

    // The CLI's own page, ahead of its commands — only when the root documents
    // something of its own.
    let root_id = root_has_own_page(spec).then(|| root_page_id(cli_title));
    if let Some(root_id) = root_id.as_deref().filter(|id| in_regions(id)) {
        let page = Page {
            cmd: spec,
            path: &[],
            ancestors: &[],
            doc: spec,
            is_root: true,
            root_id,
        };
        refs.push(page_reference(&page, cli_title, per_command_globals));
    }

    walk(
        spec.get("commands"),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut |cmd, cmd_path, ancestors| {
            let region = cmd_path.join(" ");
            if in_regions(&region) {
                let page = Page {
                    cmd,
                    path: cmd_path,
                    ancestors,
                    doc: spec,
                    is_root: false,
                    root_id: "",
                };
                refs.push(page_reference(&page, cli_title, per_command_globals));
            }
        },
    );

    // Default: the global options live on a single dedicated page in the sidebar.
    if !per_command && !global_options.is_empty() && in_regions(GLOBAL_OPTIONS_REGION) {
        refs.push(global_options_reference(&global_options));
    }

    refs
}

/// Depth-first walk over the command tree, skipping hidden commands. The
/// callback sees each visited command with its full path from the CLI root and
/// its ancestor commands (outermost first; the root is not one).
fn walk<'a, F>(
    commands: Option<&'a Value>,
    parent_path: &mut Vec<&'a str>,
    ancestors: &mut Vec<&'a Value>,
    visit: &mut F,
) where
    F: FnMut(&'a Value, &[&'a str], &[&'a Value]),
{
    for cmd in array(commands) {
        if truthy(cmd.get("hidden")) {
            continue;
        }
        // `[...parentPath, cmd.name].join(' ')` — Array#join renders a missing
        // name as the empty string.
        parent_path.push(str_or_empty(cmd.get("name")));
        visit(cmd, parent_path, ancestors);
        if !array(cmd.get("commands")).is_empty() {
            ancestors.push(cmd);
            walk(cmd.get("commands"), parent_path, ancestors, visit);
            ancestors.pop();
        }
        parent_path.pop();
    }
}

/// Whether the root documents something of its own, which earns it a page:
/// arguments, examples, a session or environment variables. Options alone do
/// not (a root `--version` or `--help` keeps a document's shape); its
/// non-global ones render on the page once it exists.
fn root_has_own_page(doc: &Value) -> bool {
    let session = doc.get("x-interactive");
    !visible(doc.get("arguments")).is_empty()
        || !example_lines(doc).is_empty()
        || !visible(session.and_then(|s| s.get("commands"))).is_empty()
        || !visible(session.and_then(|s| s.get("keys"))).is_empty()
        || !visible(doc.get("x-environment")).is_empty()
}

/// The id of the root's page: its region and its canonical, so a URL path
/// segment. The CLI's title is free text (`Acme Agent`, `@acme/agent`), and a
/// page's canonical becomes its route, its `.md` twin and its links, so the id
/// is the title with ASCII letters lowercased and every run of anything but
/// `a-z`, `0-9`, `_` and `-` one `-`, trimmed of `-`; `cli` when nothing is
/// left.
fn root_page_id(cli_title: &str) -> String {
    let segment = path_segment(cli_title);
    if segment.is_empty() {
        "cli".to_string()
    } else {
        segment
    }
}

fn path_segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_run = false;
    for c in text.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn global_options_reference(global_options: &[&Value]) -> Value {
    json!({
        "title": "Global options",
        "canonical": GLOBAL_OPTIONS_REGION,
        "description": "Options available on every command.",
        "category": "cli",
        "context": {
            "path": GLOBAL_OPTIONS_REGION,
            "fullPath": "",
            "group": ["Global options"],
        },
        "examples": { "groups": [] },
        "definitions": [{
            "title": "Global options",
            "properties": global_options.iter().map(|o| option_to_property(o)).collect::<Vec<_>>(),
        }],
    })
}

/// What one page documents: a command, or the document root.
struct Page<'a> {
    /// The command, or the document root.
    cmd: &'a Value,
    /// Its path from the CLI root; empty for the root.
    path: &'a [&'a str],
    /// Its ancestor commands, outermost first; the root is not one.
    ancestors: &'a [&'a Value],
    /// The document, where exit codes are looked up last.
    doc: &'a Value,
    is_root: bool,
    /// The root's page id ([`root_page_id`]), its region and canonical; empty
    /// for a command.
    root_id: &'a str,
}

fn page_reference(page: &Page, cli_title: &str, global_options: &[&Value]) -> Value {
    let cmd = page.cmd;
    let cmd_path = page.path;
    // A command's page is named and answers by its path. The root's is named by
    // the CLI's title and answers its id, a URL path segment of that title.
    let name = if page.is_root {
        cli_title.to_string()
    } else {
        cmd_path.join(" ")
    };
    let region = if page.is_root && !page.root_id.is_empty() {
        page.root_id.to_string()
    } else {
        name.clone()
    };
    let display_path = if page.is_root {
        cli_title.to_string()
    } else {
        format!("{cli_title} {name}").trim().to_string()
    };

    let args = visible(cmd.get("arguments"));
    // The root's recursive options are the global options, documented apart.
    let opts: Vec<&Value> = visible(cmd.get("options"))
        .into_iter()
        .filter(|o| !page.is_root || !truthy(o.get("recursive")))
        .collect();
    let subs = visible(cmd.get("commands"));
    let has_subcommands = !subs.is_empty();
    // A group with arguments or options of its own also runs on its own.
    let placeholder = command_placeholder(has_subcommands, !args.is_empty() || !opts.is_empty());
    let session = cmd.get("x-interactive");

    let mut definitions: Vec<Value> = Vec::new();
    push_definition(
        &mut definitions,
        "Arguments",
        args.iter().map(|a| argument_to_property(a)).collect(),
    );
    push_definition(
        &mut definitions,
        "Options",
        opts.iter().map(|o| option_to_property(o)).collect(),
    );
    push_definition(
        &mut definitions,
        "Inherited options",
        inherited_options(cmd, page.ancestors)
            .iter()
            .map(|o| option_to_property(o))
            .collect(),
    );
    push_definition(
        &mut definitions,
        "Commands",
        subs.iter().map(|s| subcommand_property(s)).collect(),
    );
    push_definition(
        &mut definitions,
        "Global options",
        page_global_options(page, global_options)
            .iter()
            .map(|o| option_to_property(o))
            .collect(),
    );
    push_definition(
        &mut definitions,
        "Interactive commands",
        visible(session.and_then(|s| s.get("commands")))
            .iter()
            .map(|c| interactive_command_property(c))
            .collect(),
    );
    push_definition(
        &mut definitions,
        "Keys",
        visible(session.and_then(|s| s.get("keys")))
            .iter()
            .map(|k| key_property(k))
            .collect(),
    );
    push_definition(
        &mut definitions,
        "Environment",
        visible(cmd.get("x-environment"))
            .iter()
            .map(|e| environment_property(e))
            .collect(),
    );
    push_definition(
        &mut definitions,
        "Exit codes",
        exit_codes(page)
            .iter()
            .filter(|e| e.get("code").is_some_and(Value::is_number))
            .map(exit_code_property)
            .collect(),
    );

    // Sidebar grouping: a leaf top-level command sits directly under "Commands".
    // A command that owns subcommands becomes a nested group (named after
    // itself) under "Commands", and each of its subcommands sits inside that
    // same group — e.g. `components` + `components install`. The root's page
    // (an empty path) sits directly under "Commands".
    let group: Vec<&str> = if cmd_path.len() == 1 && !has_subcommands {
        vec!["Commands"]
    } else {
        let tail = if has_subcommands {
            cmd_path
        } else {
            // `cmdPath.slice(0, -1)` — empty-safe, like JS.
            cmd_path.split_last().map(|(_, rest)| rest).unwrap_or(&[])
        };
        std::iter::once("Commands")
            .chain(tail.iter().copied())
            .collect()
    };

    // The runnable invocation ("CLI Tool") at groups[0], then the command's own
    // examples, plus — when the OpenAPI binding carried one — an "Example
    // response" group. Keep CLI Tool first.
    let mut groups: Vec<Value> = vec![json!({
        "examples": cli_tool_examples(cli_title, cmd, cmd_path, placeholder == Some(REQUIRED_SUBCOMMAND)),
    })];
    if let Some(examples_group) = examples_group(cmd) {
        groups.push(examples_group);
    }
    if let Some(response_group) = response_example_group(cmd) {
        groups.push(response_group);
    }

    // A command that owns subcommands reads as "<path> <command>" (or
    // "<path> [command]" when it also runs on its own) so it's distinct from
    // its group and shows it takes a subcommand.
    let title = {
        let t = match placeholder {
            Some(p) => format!("{name} {p}"),
            None => name.clone(),
        };
        if t.is_empty() {
            cli_title.to_string()
        } else {
            t
        }
    };
    let canonical = {
        let c = if page.is_root {
            region.clone()
        } else {
            cmd_path.join("/")
        };
        if c.is_empty() {
            cli_title.to_string()
        } else {
            c
        }
    };
    let description = if page.is_root {
        nonempty_str(cmd.pointer("/info/description"))
            .or_else(|| nonempty_str(cmd.pointer("/info/summary")))
            .unwrap_or("")
    } else {
        str_or_empty(cmd.get("description"))
    };

    json!({
        "title": title,
        "canonical": canonical,
        "description": description,
        "category": "cli",
        // context.path is the region key the docs engine writes into the page
        // frontmatter (`cli: <spec>#<path>`) and reads back to re-resolve the
        // command. api.cli shows the more specific form — a command group
        // carries its placeholder.
        "context": {
            "path": region,
            "fullPath": generate_usage(cmd, &display_path, placeholder),
            "group": group,
        },
        "examples": { "groups": groups },
        "definitions": definitions,
    })
}

const REQUIRED_SUBCOMMAND: &str = "<command>";
const OPTIONAL_SUBCOMMAND: &str = "[command]";

/// The subcommand placeholder of a page: `<command>` for a group that needs a
/// subcommand, `[command]` for one that also runs on its own, none for a leaf.
fn command_placeholder(has_subcommands: bool, runs_on_its_own: bool) -> Option<&'static str> {
    match (has_subcommands, runs_on_its_own) {
        (false, _) => None,
        (true, true) => Some(OPTIONAL_SUBCOMMAND),
        (true, false) => Some(REQUIRED_SUBCOMMAND),
    }
}

/// A `{title, properties}` definition, left out when it has no rows.
fn push_definition(definitions: &mut Vec<Value>, title: &str, properties: Vec<Value>) {
    if !properties.is_empty() {
        definitions.push(json!({ "title": title, "properties": properties }));
    }
}

fn subcommand_property(s: &Value) -> Value {
    let mut p = Map::new();
    insert_defined(&mut p, "name", s.get("name"));
    p.insert("type".into(), json!("command"));
    p.insert(
        "description".into(),
        json!(str_or_empty(s.get("description"))),
    );
    Value::Object(p)
}

/// The options a command inherits from its ancestor commands, outermost first.
///
/// Each name resolves to its nearest declaration, the way a CLI library
/// propagates a recursive option: the command's own, else the nearest
/// ancestor that declares the name, recursive or not. Only a recursive,
/// visible nearest declaration is inherited; a plain one ends the chain, and a
/// redeclaration hides the outer one. The root's are the global options, not
/// these.
fn inherited_options<'a>(cmd: &'a Value, ancestors: &[&'a Value]) -> Vec<&'a Value> {
    let mut taken: Vec<&str> = option_names(cmd).collect();
    let mut nearest_first: Vec<Vec<&Value>> = Vec::new();
    for ancestor in ancestors.iter().rev() {
        let mut here: Vec<&Value> = Vec::new();
        for opt in array(ancestor.get("options")) {
            let name = str_or_empty(opt.get("name"));
            if taken.contains(&name) {
                continue;
            }
            taken.push(name);
            if truthy(opt.get("recursive")) && !truthy(opt.get("hidden")) {
                here.push(opt);
            }
        }
        nearest_first.push(here);
    }
    nearest_first.into_iter().rev().flatten().collect()
}

/// The global options one page lists with `global_options_per_command`: the
/// root's, less every name the command or one of its ancestor commands
/// declares. That nearer declaration is the one the command gets (listed
/// under Options or Inherited options), or, when it is not recursive, the end
/// of the chain. The root's own page lists them all.
fn page_global_options<'g>(page: &Page, global_options: &[&'g Value]) -> Vec<&'g Value> {
    if page.is_root {
        return global_options.to_vec();
    }
    let taken: Vec<&str> = std::iter::once(page.cmd)
        .chain(page.ancestors.iter().copied())
        .flat_map(option_names)
        .collect();
    global_options
        .iter()
        .copied()
        .filter(|o| !taken.contains(&str_or_empty(o.get("name"))))
        .collect()
}

/// The names of every option a command declares, hidden ones included.
fn option_names(cmd: &Value) -> impl Iterator<Item = &str> {
    array(cmd.get("options"))
        .iter()
        .map(|o| str_or_empty(o.get("name")))
}

/// The exit codes a page shows: the command's own, else its nearest
/// ancestor's, the root's last.
fn exit_codes<'a>(page: &Page<'a>) -> &'a [Value] {
    std::iter::once(page.cmd)
        .chain(page.ancestors.iter().rev().copied())
        .chain(std::iter::once(page.doc))
        .map(|c| array(c.get("exitCodes")))
        .find(|codes| !codes.is_empty())
        .unwrap_or(EMPTY)
}

/// One "Exit codes" row; the caller keeps only entries whose `code` is a number.
fn exit_code_property(entry: &Value) -> Value {
    json!({
        "name": entry.get("code").map(js_string).unwrap_or_default(),
        "type": "integer",
        "description": str_or_empty(entry.get("description")),
    })
}

/// One "Interactive commands" row: a command typed inside the session, with
/// its arguments, options and subcommands as nested rows. Its aliases are
/// words, not flags.
fn interactive_command_property(c: &Value) -> Value {
    let aliases = array(c.get("aliases"));
    let alias_note = if aliases.is_empty() {
        String::new()
    } else {
        let rendered = aliases.iter().map(js_string).collect::<Vec<_>>().join(", ");
        format!(" (alias: {rendered})")
    };

    let nested: Vec<Value> = visible(c.get("arguments"))
        .iter()
        .map(|a| argument_to_property(a))
        .chain(
            visible(c.get("options"))
                .iter()
                .map(|o| option_to_property(o)),
        )
        .chain(
            visible(c.get("commands"))
                .iter()
                .map(|s| interactive_command_property(s)),
        )
        .collect();

    let mut out = Map::new();
    insert_defined(&mut out, "name", c.get("name"));
    out.insert("type".into(), json!("command"));
    out.insert(
        "description".into(),
        json!(format!(
            "{}{alias_note}",
            str_or_empty(c.get("description"))
        )),
    );
    if !nested.is_empty() {
        out.insert("properties".into(), Value::Array(nested));
    }
    Value::Object(out)
}

/// One "Keys" row: the keys (any one of them), what they do, and where, when
/// the binding does not apply everywhere.
fn key_property(binding: &Value) -> Value {
    let keys = array(binding.get("keys"))
        .iter()
        .map(js_string)
        .collect::<Vec<_>>()
        .join(", ");
    let context = nonempty_str(binding.get("context"))
        .map(|c| format!(" (context: {c})"))
        .unwrap_or_default();
    json!({
        "name": keys,
        "type": "key",
        "description": format!("{}{context}", str_or_empty(binding.get("description"))),
    })
}

/// One "Environment" row. The values a variable accepts surface like an
/// argument's accepted values: an `examples` meta.
fn environment_property(var: &Value) -> Value {
    let mut out = Map::new();
    insert_defined(&mut out, "name", var.get("name"));
    out.insert("type".into(), json!("string"));
    out.insert(
        "description".into(),
        json!(str_or_empty(var.get("description"))),
    );
    let values = array(var.get("values"));
    if !values.is_empty() {
        out.insert(
            "meta".into(),
            json!([{ "name": "examples", "value": values }]),
        );
    }
    Value::Object(out)
}

/// The non-empty string entries of a command's `examples`.
fn example_lines(cmd: &Value) -> Vec<&str> {
    array(cmd.get("examples"))
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The "Examples" group: the command's own examples, one per line, in one
/// shell sample. `None` when it has none.
fn examples_group(cmd: &Value) -> Option<Value> {
    let lines = example_lines(cmd);
    if lines.is_empty() {
        return None;
    }
    Some(json!({
        "description": "Examples",
        "examples": [{
            "codeblock": {
                "tabs": [{ "title": "CLI Tool", "language": "shell", "code": lines.join("\n") }],
            },
        }],
    }))
}

/// Usage line for a command (`generateUsage` in `generate.ts`), with the
/// subcommand placeholder `api.cli` asks for.
fn generate_usage(command: &Value, command_path: &str, placeholder: Option<&str>) -> String {
    let mut parts: Vec<String> = vec![command_path.to_string()];

    // A command that owns subcommands takes one as its next token.
    if let Some(p) = placeholder {
        parts.push(p.to_string());
    }

    if !visible(command.get("options")).is_empty() {
        parts.push("[options]".to_string());
    }

    for arg in visible(command.get("arguments")) {
        let name = str_or_empty(arg.get("name")).to_lowercase();
        // `...` suffix when the argument accepts multiple values (variadic).
        let suffix = if truthy(arg.get("variadic")) {
            "..."
        } else {
            ""
        };
        parts.push(if truthy(arg.get("required")) {
            format!("<{name}{suffix}>")
        } else {
            format!("[{name}{suffix}]")
        });
    }

    parts.join(" ")
}

/// Map a response content type to a code-sample language.
fn response_language(content_type: Option<&str>) -> &'static str {
    let Some(ct) = content_type.filter(|s| !s.is_empty()) else {
        return "json";
    };
    let lower = ct.to_lowercase();
    if lower.contains("json") {
        "json"
    } else if lower.contains("xml") {
        "xml"
    } else {
        "text"
    }
}

/// The "Example response" group built from the command's `x-openapi.responses`
/// binding (emitted by openapi2opencli). `None` when no example is available.
fn response_example_group(cmd: &Value) -> Option<Value> {
    let responses = array(cmd.pointer("/x-openapi/responses"));
    if responses.is_empty() {
        return None;
    }

    let mut examples: Vec<Value> = Vec::new();
    for res in responses {
        // Skip missing / null bodies (no useful sample).
        let example = match res.get("example") {
            Some(v) if !v.is_null() => v,
            _ => continue,
        };
        let code = match example.as_str() {
            Some(s) => s.to_string(),
            // JSON.stringify(value, null, 2); canonicalize first so integral
            // floats render as JS would (`1.0` → `1`).
            None => serde_json::to_string_pretty(&canon::canonicalize(example))
                .unwrap_or_else(|_| String::new()),
        };
        let content_type = nonempty_str(res.get("contentType"));
        examples.push(json!({
            "codeblock": {
                "title": nonempty_str(res.get("status")).unwrap_or("200"),
                "tabs": [{
                    "title": content_type.unwrap_or("application/json"),
                    "language": response_language(content_type),
                    "code": code,
                }],
            },
        }));
    }

    if examples.is_empty() {
        return None;
    }
    Some(json!({ "description": "Example response", "examples": examples }))
}

/// Shell-quote a value only when it needs it (spaces / special chars).
fn shell_quote(value: &str) -> String {
    // JS: /[\s'"$`\\<>|&;()]/
    let needs = value
        .chars()
        .any(|c| js_space(c) || "'\"$`\\<>|&;()".contains(c));
    if needs {
        format!("'{}'", value.replace('\'', "'\\''"))
    } else {
        value.to_string()
    }
}

/// JS regex `\s`: Unicode White_Space plus U+FEFF, minus U+0085 (which JS's
/// `\s` excludes but Rust's `char::is_whitespace` includes).
fn js_space(c: char) -> bool {
    c == '\u{feff}' || (c != '\u{85}' && c.is_whitespace())
}

/// A representative value for an argument: an explicit example, an
/// accepted/enum value, else a placeholder.
fn example_value(arg: Option<&Value>) -> String {
    if let Some(example) = arg.and_then(arg_example_value) {
        return example.to_string();
    }
    let accepted = arg
        .map(|a| array(a.get("acceptedValues")))
        .unwrap_or_default();
    if let Some(first) = accepted.first() {
        return js_string(first);
    }
    "Example data".to_string()
}

/// An argument's explicit example value (from metadata), or `None`. Mirrors the
/// TS guard: the metadata value must be a NON-EMPTY string.
fn arg_example_value(arg: &Value) -> Option<&str> {
    array(arg.get("metadata"))
        .iter()
        .find(|m| m.get("name").and_then(Value::as_str) == Some("example"))
        .and_then(|m| m.get("value"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// The "CLI Tool" example(s) for a command. When a required argument enumerates
/// its accepted values (e.g. `completion <zsh|fish>`), each value becomes its
/// own example (`codeblock.title` = the value) — so Atlas renders an
/// example-level switcher rather than tabs inside one code block.
fn cli_tool_examples(
    cli_title: &str,
    cmd: &Value,
    cmd_path: &[&str],
    needs_subcommand: bool,
) -> Vec<Value> {
    let req_args: Vec<&Value> = array(cmd.get("arguments"))
        .iter()
        .filter(|a| truthy(a.get("required")) && !truthy(a.get("hidden")))
        .collect();

    // The argument whose representative value(s) label the example(s) — an enum
    // gives one per value, a single example value gives one. Reverse-find: the
    // LAST qualifying required argument wins.
    let variant_arg = req_args
        .iter()
        .rev()
        .find(|a| !array(a.get("acceptedValues")).is_empty() || arg_example_value(a).is_some())
        .copied();

    if let Some(arg) = variant_arg {
        let accepted = array(arg.get("acceptedValues"));
        let values: Vec<String> = if !accepted.is_empty() {
            accepted.iter().map(js_string).collect()
        } else {
            // Guaranteed non-null by the find predicate above.
            vec![arg_example_value(arg).unwrap_or("null").to_string()]
        };
        let arg_name = str_or_empty(arg.get("name"));
        return values
            .into_iter()
            .map(|value| {
                let code = generate_cli_example(
                    cli_title,
                    cmd,
                    cmd_path,
                    needs_subcommand,
                    Some((arg_name, &value)),
                );
                json!({
                    "codeblock": {
                        "title": value,
                        "tabs": [{ "title": "CLI Tool", "language": "shell", "code": code }],
                    },
                })
            })
            .collect();
    }

    vec![json!({
        "codeblock": {
            "tabs": [{
                "title": "CLI Tool",
                "language": "shell",
                "code": generate_cli_example(cli_title, cmd, cmd_path, needs_subcommand, None),
            }],
        },
    })]
}

/// A runnable, concrete CLI invocation — the command path plus its required
/// arguments and options filled with example values, one option per line.
/// `needs_subcommand` adds the `<command>` placeholder of a group that does not
/// run on its own.
/// `override_arg` pins a specific value for a named argument (used to render one
/// example per accepted value).
fn generate_cli_example(
    cli_title: &str,
    cmd: &Value,
    cmd_path: &[&str],
    needs_subcommand: bool,
    override_arg: Option<(&str, &str)>,
) -> String {
    let mut head = std::iter::once(cli_title)
        .chain(cmd_path.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    // A command group that needs a subcommand shows a placeholder (unquoted,
    // like the usage formula), e.g. `xyd components <command>`. One that also
    // runs on its own is invoked without one.
    if needs_subcommand {
        head.push(' ');
        head.push_str(REQUIRED_SUBCOMMAND);
    }

    for arg in array(cmd.get("arguments"))
        .iter()
        .filter(|a| truthy(a.get("required")) && !truthy(a.get("hidden")))
    {
        let name = str_or_empty(arg.get("name"));
        let value = match override_arg {
            Some((k, v)) if k == name => v.to_string(),
            _ => example_value(Some(arg)),
        };
        head.push(' ');
        head.push_str(&shell_quote(&value));
    }

    let mut lines = vec![head];
    for opt in array(cmd.get("options"))
        .iter()
        .filter(|o| truthy(o.get("required")) && !truthy(o.get("hidden")))
    {
        let name = str_or_empty(opt.get("name"));
        let opt_args = array(opt.get("arguments"));
        match opt_args.first() {
            // boolean flag
            None => lines.push(format!("--{name}")),
            Some(first) => lines.push(format!(
                "--{name} {}",
                shell_quote(&example_value(Some(first)))
            )),
        }
    }

    lines.join(" \\\n  ")
}

fn argument_to_property(arg: &Value) -> Value {
    let mut meta: Vec<Value> = Vec::new();
    if truthy(arg.get("required")) {
        meta.push(json!({ "name": "required", "value": "true" }));
    }
    // Surface the argument's accepted values / example in the Arguments section.
    let accepted = array(arg.get("acceptedValues"));
    if !accepted.is_empty() {
        meta.push(json!({ "name": "examples", "value": accepted }));
    } else if let Some(example) = array(arg.get("metadata"))
        .iter()
        .find(|m| m.get("name").and_then(Value::as_str) == Some("example"))
        .and_then(|m| m.get("value"))
        // `example != null && example !== ''`
        .filter(|v| !v.is_null() && v.as_str() != Some(""))
    {
        meta.push(json!({ "name": "example", "value": example }));
    }

    let mut out = Map::new();
    insert_defined(&mut out, "name", arg.get("name"));
    out.insert(
        "type".into(),
        json!(if truthy(arg.get("arity")) {
            "array"
        } else {
            "string"
        }),
    );
    out.insert(
        "description".into(),
        json!(str_or_empty(arg.get("description"))),
    );
    if !meta.is_empty() {
        out.insert("meta".into(), Value::Array(meta));
    }
    Value::Object(out)
}

fn option_to_property(opt: &Value) -> Value {
    let aliases = array(opt.get("aliases"));
    let alias_note = if aliases.is_empty() {
        String::new()
    } else {
        let rendered = aliases
            .iter()
            .map(|a| {
                let s = js_string(a);
                // JS `a.length` is a UTF-16 code-unit count.
                if s.encode_utf16().count() == 1 {
                    format!("-{s}")
                } else {
                    format!("--{s}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(" (alias: {rendered})")
    };

    let mut out = Map::new();
    out.insert(
        "name".into(),
        json!(format!("--{}", str_or_empty(opt.get("name")))),
    );
    // an option that takes a value vs. a boolean flag
    out.insert(
        "type".into(),
        json!(if array(opt.get("arguments")).is_empty() {
            "boolean"
        } else {
            "string"
        }),
    );
    out.insert(
        "description".into(),
        json!(format!(
            "{}{alias_note}",
            str_or_empty(opt.get("description"))
        )),
    );
    if truthy(opt.get("required")) {
        out.insert(
            "meta".into(),
            json!([{ "name": "required", "value": "true" }]),
        );
    }
    Value::Object(out)
}

// ---------------------------------------------------------------------------
// JS-semantics helpers
// ---------------------------------------------------------------------------

const EMPTY: &[Value] = &[];

/// `value || []` for an array-valued field.
fn array(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(EMPTY)
}

/// Insert `key` only when the source value is present — JSON.stringify drops
/// `undefined` properties rather than writing `null`.
fn insert_defined(map: &mut Map<String, Value>, key: &str, v: Option<&Value>) {
    if let Some(v) = v {
        map.insert(key.to_string(), v.clone());
    }
}

/// `(list || []).filter(x => !x.hidden)`.
fn visible(v: Option<&Value>) -> Vec<&Value> {
    array(v)
        .iter()
        .filter(|x| !truthy(x.get("hidden")))
        .collect()
}

/// `value || ''` for a string-valued field.
fn str_or_empty(v: Option<&Value>) -> &str {
    v.and_then(Value::as_str).unwrap_or("")
}

/// `value || <fallback>` for a string-valued field — an empty string is falsy.
fn nonempty_str(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// JS truthiness for a possibly-absent JSON value.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true, // arrays and objects are always truthy
    }
}

/// `String(value)` for a JSON value.
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(_) => canon::canonicalize(v).to_string(),
        Value::Array(items) => items
            .iter()
            .map(|i| match i {
                Value::Null => String::new(),
                other => js_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_spec_yields_no_references() {
        let opts = OpencliToReferencesOptions::default();
        assert!(opencli_to_references(&Value::Null, &opts).is_empty());
        assert!(opencli_to_references(&json!({}), &opts).is_empty());
    }

    #[test]
    fn shell_quote_only_when_needed() {
        assert_eq!(shell_quote("plain"), "plain");
        assert_eq!(shell_quote("has space"), "'has space'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("<command>"), "'<command>'");
    }

    #[test]
    fn response_language_mapping() {
        assert_eq!(response_language(None), "json");
        assert_eq!(response_language(Some("application/json")), "json");
        assert_eq!(response_language(Some("APPLICATION/XML")), "xml");
        assert_eq!(response_language(Some("text/plain")), "text");
    }

    /// The committed goldens never contain a command where TWO required
    /// arguments qualify as the "variant" argument, so they do NOT discriminate
    /// the reverse-find direction. Pin it here: the TS is `[...reqArgs]
    /// .reverse().find(...)`, i.e. the LAST qualifying required argument wins.
    #[test]
    fn variant_argument_is_the_last_qualifying_required_argument() {
        let spec = json!({
            "info": { "title": "demo" },
            "commands": [{
                "name": "copy",
                "arguments": [
                    { "name": "from", "required": true, "acceptedValues": ["a", "b"] },
                    { "name": "to", "required": true, "acceptedValues": ["x", "y"] },
                ],
            }],
        });
        let refs = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        let examples = refs[0]["examples"]["groups"][0]["examples"].clone();

        // One example per accepted value of the LAST qualifying argument (`to`),
        // and the OTHER argument keeps its own first accepted value.
        assert_eq!(
            examples,
            json!([
                { "codeblock": { "title": "x", "tabs": [
                    { "title": "CLI Tool", "language": "shell", "code": "demo copy a x" }]}},
                { "codeblock": { "title": "y", "tabs": [
                    { "title": "CLI Tool", "language": "shell", "code": "demo copy a y" }]}},
            ])
        );
    }

    /// Likewise unexercised by the goldens: an explicit `metadata.example` makes
    /// an argument qualify even without `acceptedValues`, yielding ONE example.
    #[test]
    fn metadata_example_qualifies_an_argument_without_accepted_values() {
        let spec = json!({
            "info": { "title": "demo" },
            "commands": [{
                "name": "run",
                "arguments": [{
                    "name": "path",
                    "required": true,
                    "metadata": [{ "name": "example", "value": "./docs dir" }],
                }],
            }],
        });
        let refs = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        assert_eq!(
            refs[0]["examples"]["groups"][0]["examples"],
            json!([{ "codeblock": { "title": "./docs dir", "tabs": [{
                "title": "CLI Tool",
                "language": "shell",
                // the title is raw; only the invocation is shell-quoted
                "code": "demo run './docs dir'",
            }]}}])
        );
    }

    /// `globalOptionsPerCommand` moves the root recursive options from a single
    /// trailing reference onto every command as a LAST definition.
    #[test]
    fn global_options_move_between_a_page_and_per_command_definitions() {
        let spec = json!({
            "info": { "title": "demo" },
            "options": [
                { "name": "verbose", "recursive": true },
                { "name": "hidden-one", "recursive": true, "hidden": true },
                { "name": "not-global" },
            ],
            "commands": [{ "name": "build", "arguments": [{ "name": "target" }] }],
        });

        let default = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        assert_eq!(default.len(), 2, "one command + one global-options page");
        assert_eq!(default[1]["canonical"], json!("global-options"));
        // `recursive && !hidden` — exactly one qualifies.
        assert_eq!(
            default[1]["definitions"][0]["properties"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(default[0]["definitions"].as_array().unwrap().len(), 1); // Arguments only

        let per_command = opencli_to_references(
            &spec,
            &OpencliToReferencesOptions {
                regions: None,
                global_options_per_command: true,
            },
        );
        assert_eq!(per_command.len(), 1, "no extra global-options page");
        let defs = per_command[0]["definitions"].as_array().unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0]["title"], json!("Arguments"));
        assert_eq!(
            defs[1]["title"],
            json!("Global options"),
            "Global options come LAST"
        );
    }

    fn titles(refs: &[Value]) -> Vec<&str> {
        refs.iter().map(|r| r["title"].as_str().unwrap()).collect()
    }

    fn definition_titles(reference: &Value) -> Vec<&str> {
        reference["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["title"].as_str().unwrap())
            .collect()
    }

    /// A group with an option of its own runs on its own: `[command]` in the
    /// title and usage, no placeholder in its sample. One without keeps
    /// `<command>` everywhere.
    #[test]
    fn a_group_with_options_of_its_own_runs_without_a_subcommand() {
        let spec = json!({
            "info": { "title": "demo" },
            "commands": [
                { "name": "chat", "options": [{ "name": "fast" }], "commands": [{ "name": "log" }] },
                { "name": "remote", "commands": [{ "name": "add" }] },
            ],
        });
        let refs = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        let sample = |r: &Value| {
            r["examples"]["groups"][0]["examples"][0]["codeblock"]["tabs"][0]["code"].clone()
        };
        assert_eq!(refs[0]["title"], json!("chat [command]"));
        assert_eq!(
            refs[0]["context"]["fullPath"],
            json!("demo chat [command] [options]")
        );
        assert_eq!(sample(&refs[0]), json!("demo chat"));
        assert_eq!(refs[2]["title"], json!("remote <command>"));
        assert_eq!(
            refs[2]["context"]["fullPath"],
            json!("demo remote <command>")
        );
        assert_eq!(sample(&refs[2]), json!("demo remote <command>"));
    }

    /// Options or exit codes alone give the root no page; examples do, ahead of
    /// the commands, answering the CLI's title as its region.
    #[test]
    fn the_root_gets_a_page_only_for_what_it_documents_itself() {
        let quiet = json!({
            "info": { "title": "demo" },
            "options": [{ "name": "version" }, { "name": "verbose", "recursive": true }],
            "exitCodes": [{ "code": 0, "description": "ok" }],
            "commands": [{ "name": "build" }],
        });
        let refs = opencli_to_references(&quiet, &OpencliToReferencesOptions::default());
        assert_eq!(titles(&refs), vec!["build", "Global options"]);

        let mut documented = quiet.clone();
        documented["examples"] = json!(["demo build", ""]);
        let refs = opencli_to_references(&documented, &OpencliToReferencesOptions::default());
        assert_eq!(
            titles(&refs),
            vec!["demo [command]", "build", "Global options"]
        );
        let root = &refs[0];
        assert_eq!(root["canonical"], json!("demo"));
        assert_eq!(root["context"]["path"], json!("demo"));
        assert_eq!(root["context"]["group"], json!(["Commands"]));
        // its own option, not the global one; its exit codes; its examples
        assert_eq!(
            definition_titles(root),
            vec!["Options", "Commands", "Exit codes"]
        );
        assert_eq!(
            root["definitions"][0]["properties"][0]["name"],
            json!("--version")
        );
        assert_eq!(
            root["examples"]["groups"][1]["examples"][0]["codeblock"]["tabs"][0]["code"],
            json!("demo build")
        );

        let only_root = opencli_to_references(
            &documented,
            &OpencliToReferencesOptions {
                regions: Some(vec!["demo".into()]),
                global_options_per_command: false,
            },
        );
        assert_eq!(titles(&only_root), vec!["demo [command]"]);
    }

    /// The root's page answers a URL path segment of the CLI's title, and is
    /// titled with the title itself.
    #[test]
    fn the_root_page_answers_a_url_path_segment_of_the_title() {
        let titled = |title: &str| {
            json!({
                "info": { "title": title },
                "arguments": [{ "name": "prompt" }],
                "commands": [{ "name": "build" }],
            })
        };
        let default = OpencliToReferencesOptions::default();
        let root = |title: &str| opencli_to_references(&titled(title), &default).remove(0);

        let spaced = root("Acme Agent");
        assert_eq!(spaced["canonical"], json!("acme-agent"));
        assert_eq!(spaced["context"]["path"], json!("acme-agent"));
        assert_eq!(spaced["title"], json!("Acme Agent [command]"));
        assert_eq!(
            spaced["context"]["fullPath"],
            json!("Acme Agent [command] [prompt]")
        );

        assert_eq!(root("@acme/agent")["canonical"], json!("acme-agent"));
        assert_eq!(root("My_CLI v2.0")["canonical"], json!("my_cli-v2-0"));
        assert_eq!(root("acme--agent")["canonical"], json!("acme--agent"));
        assert_eq!(root("Ünï 工具 ✓")["canonical"], json!("n"));
        assert_eq!(root("工具")["canonical"], json!("cli"));
        assert_eq!(root("")["canonical"], json!("cli"));

        // the id is the region; the raw title names no page
        let only = |region: &str| {
            opencli_to_references(
                &titled("Acme Agent"),
                &OpencliToReferencesOptions {
                    regions: Some(vec![region.into()]),
                    global_options_per_command: false,
                },
            )
        };
        assert_eq!(titles(&only("acme-agent")), vec!["Acme Agent [command]"]);
        assert!(only("Acme Agent").is_empty());
    }

    /// The nearest non-empty list wins, and an entry without a numeric code is
    /// not a row.
    #[test]
    fn exit_codes_come_from_the_nearest_list_and_need_a_numeric_code() {
        let spec = json!({
            "info": { "title": "demo" },
            "exitCodes": [{ "code": 0, "description": "ok" }],
            "commands": [{
                "name": "db",
                "exitCodes": [{ "code": "3", "description": "a string" }, { "code": 5 }],
                "commands": [{ "name": "dump", "exitCodes": [] }],
            }, { "name": "build" }],
        });
        let refs = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        let codes =
            |r: &Value| r["definitions"].as_array().unwrap().last().unwrap()["properties"].clone();
        assert_eq!(
            codes(&refs[1]),
            json!([{ "name": "5", "type": "integer", "description": "" }])
        );
        assert_eq!(
            codes(&refs[2]),
            json!([{ "name": "0", "type": "integer", "description": "ok" }])
        );
    }

    /// A command's own option shadows an inherited one of the same name; a
    /// hidden recursive option is not inherited; the root's are global options.
    #[test]
    fn inherited_options_skip_shadowed_hidden_and_root_options() {
        let spec = json!({
            "info": { "title": "demo" },
            "options": [{ "name": "profile", "recursive": true }],
            "commands": [{
                "name": "db",
                "options": [
                    { "name": "host", "recursive": true },
                    { "name": "port", "recursive": true },
                    { "name": "trace", "recursive": true, "hidden": true },
                ],
                "commands": [{ "name": "dump", "options": [{ "name": "port", "description": "own" }] }],
            }],
        });
        let refs = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        let dump = &refs[1];
        assert_eq!(
            definition_titles(dump),
            vec!["Options", "Inherited options"]
        );
        assert_eq!(
            dump["definitions"][1]["properties"],
            json!([
                { "name": "--host", "type": "boolean", "description": "" },
            ])
        );
    }

    /// The page of `path` (space-joined) in `refs`.
    fn page<'r>(refs: &'r [Value], path: &str) -> &'r Value {
        refs.iter()
            .find(|r| r["context"]["path"] == json!(path))
            .unwrap_or_else(|| panic!("no page {path}"))
    }

    /// The rows of the definition titled `title` on a page; none when absent.
    fn rows(reference: &Value, title: &str) -> Value {
        reference["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["title"] == json!(title))
            .map(|d| d["properties"].clone())
            .unwrap_or(Value::Null)
    }

    /// The nearest declaration of a name wins, as a CLI library propagates a
    /// recursive option: a nearer recursive redeclaration replaces the outer
    /// one, and a nearer plain one ends the chain.
    #[test]
    fn inherited_options_resolve_each_name_to_its_nearest_declaration() {
        let spec = json!({
            "info": { "title": "demo" },
            "commands": [{
                "name": "a",
                "options": [
                    { "name": "host", "recursive": true, "description": "A host" },
                    { "name": "color", "recursive": true, "description": "A color" },
                ],
                "commands": [
                    {
                        "name": "b",
                        "options": [{
                            "name": "host",
                            "recursive": true,
                            "description": "B host",
                            "arguments": [{ "name": "url", "required": true }],
                        }],
                        "commands": [{ "name": "c" }],
                    },
                    {
                        "name": "b2",
                        "options": [{ "name": "host", "description": "B2 host" }],
                        "commands": [{ "name": "c2" }],
                    },
                    {
                        "name": "b3",
                        "options": [{ "name": "host", "recursive": true, "hidden": true }],
                        "commands": [{ "name": "c3" }],
                    },
                ],
            }],
        });
        let refs = opencli_to_references(&spec, &OpencliToReferencesOptions::default());
        // Outermost first: a's --color, then b's own --host.
        assert_eq!(
            rows(page(&refs, "a b c"), "Inherited options"),
            json!([
                { "name": "--color", "type": "boolean", "description": "A color" },
                { "name": "--host", "type": "string", "description": "B host" },
            ])
        );
        // b2's plain --host shadows a's and is not passed on.
        assert_eq!(
            rows(page(&refs, "a b2 c2"), "Inherited options"),
            json!([{ "name": "--color", "type": "boolean", "description": "A color" }])
        );
        // b3's hidden --host shadows a's and stays hidden.
        assert_eq!(
            rows(page(&refs, "a b3 c3"), "Inherited options"),
            json!([{ "name": "--color", "type": "boolean", "description": "A color" }])
        );
        // The redeclaring commands list their own --host, not a's.
        assert_eq!(
            rows(page(&refs, "a b2"), "Inherited options"),
            json!([{ "name": "--color", "type": "boolean", "description": "A color" }])
        );
    }

    /// Per command, a global option that the command or an ancestor command
    /// redeclares is listed once: as that nearer declaration, or not at all
    /// when it is plain and above the page. The root's own page keeps them all.
    #[test]
    fn per_command_global_options_leave_out_a_redeclared_name() {
        let spec = json!({
            "info": { "title": "demo" },
            "arguments": [{ "name": "prompt" }],
            "options": [
                { "name": "verbose", "recursive": true, "description": "root verbose" },
                { "name": "quiet", "recursive": true, "description": "root quiet" },
            ],
            "commands": [
                {
                    "name": "a",
                    "options": [{ "name": "verbose", "recursive": true, "description": "a verbose" }],
                    "commands": [{ "name": "b" }],
                },
                {
                    "name": "p",
                    "options": [{ "name": "quiet", "description": "p quiet" }],
                    "commands": [{ "name": "q" }],
                },
            ],
        });
        let refs = opencli_to_references(
            &spec,
            &OpencliToReferencesOptions {
                regions: None,
                global_options_per_command: true,
            },
        );
        let quiet = json!({ "name": "--quiet", "type": "boolean", "description": "root quiet" });
        let verbose =
            json!({ "name": "--verbose", "type": "boolean", "description": "root verbose" });
        let a_verbose =
            json!({ "name": "--verbose", "type": "boolean", "description": "a verbose" });

        assert_eq!(
            rows(page(&refs, "demo"), "Global options"),
            json!([verbose, quiet])
        );
        assert_eq!(rows(page(&refs, "a"), "Options"), json!([a_verbose]));
        assert_eq!(rows(page(&refs, "a"), "Global options"), json!([quiet]));
        assert_eq!(
            rows(page(&refs, "a b"), "Inherited options"),
            json!([a_verbose])
        );
        assert_eq!(rows(page(&refs, "a b"), "Global options"), json!([quiet]));
        // p's plain --quiet ends the root's chain: q gets neither.
        assert_eq!(rows(page(&refs, "p q"), "Inherited options"), Value::Null);
        assert_eq!(rows(page(&refs, "p q"), "Global options"), json!([verbose]));
    }

    /// `x-passthrough` documents a handover, not a different page.
    #[test]
    fn a_passthrough_renders_like_any_command() {
        let plain = json!({
            "info": { "title": "demo" },
            "commands": [{ "name": "agent", "arguments": [{ "name": "prompt" }], "commands": [{ "name": "resume" }] }],
        });
        let mut passthrough = plain.clone();
        passthrough["commands"][0]["x-passthrough"] = json!({ "program": "demo-agent" });
        let opts = OpencliToReferencesOptions::default();
        assert_eq!(
            opencli_to_references(&passthrough, &opts),
            opencli_to_references(&plain, &opts)
        );
    }

    #[test]
    fn regions_filter_is_exact_match_on_the_space_joined_path() {
        let spec = json!({
            "info": { "title": "demo" },
            "commands": [{ "name": "remote", "commands": [{ "name": "add" }] }],
        });
        let refs = opencli_to_references(
            &spec,
            &OpencliToReferencesOptions {
                regions: Some(vec!["remote add".into()]),
                global_options_per_command: false,
            },
        );
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0]["canonical"], json!("remote/add"));
    }
}
