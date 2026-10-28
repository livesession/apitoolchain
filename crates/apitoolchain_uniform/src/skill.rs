//! `Reference[]` → an agent skill — the twin of
//! `packages/apitoolchain-uniform/src/skill/{index,markdown}.ts`
//! (`skillFiles`), pinned to it line for line by the `6.skill.*` fixtures.
//!
//! The files, by path relative to the skill root: `SKILL.md` (frontmatter, an
//! intro, a table of the reference files), `<dir>/index.md` (every
//! operation), one `<dir>/<group>.md` per operation group, and the extras.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::jsrt::{is_js_whitespace, truthy};
use crate::mcp::ReferencePredicate;
use crate::operations::{
    array_item, array_of, definition_properties, effective_property, enum_base_type, enum_value,
    find_meta, first_group, is_json_content_type, is_required, is_rest_operation, js_ordered,
    js_trim, json_schema_type, meta_flag, meta_value, object_at, object_children,
    operation_full_path, operation_method, operation_path, request_body, response_variants, str_of,
    text, variant_content_type,
};

/// The group a reference is filed under.
pub type ReferenceGroup = Arc<dyn Fn(&Value) -> String + Send + Sync>;
/// The text of a generated file's heading comment, from its group name.
pub type SkillHeader = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// The generated reference files.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillReferencesOptions {
    /// Where they go, relative to SKILL.md (default `references`).
    pub dir: Option<String>,
    /// One file per operation, `<dir>/<group>/<operation>.md`, instead of one
    /// per group (default `false`): the group directory a slug of the group
    /// name, the operation file its canonical with case kept.
    pub per_operation: Option<bool>,
    /// Their heading comment (default [`DEFAULT_SKILL_HEADER`]); called with
    /// the group name, `index` for the index.
    #[serde(skip)]
    pub header: Option<SkillHeader>,
}

/// Options of [`skill_files`]; the data options deserialize from the
/// TypeScript option names, the callbacks are set in code.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOptions {
    /// The skill's name (frontmatter `name`).
    pub name: String,
    /// When to use the skill (frontmatter `description`).
    pub description: String,
    /// A `# title` under the frontmatter (none by default).
    #[serde(default)]
    pub title: Option<String>,
    /// Markdown placed under the frontmatter (and the title), byte for byte.
    #[serde(default)]
    pub intro: Option<String>,
    /// The group of an operation (default `context.group[0]`, else `general`).
    #[serde(skip)]
    pub group_by: Option<ReferenceGroup>,
    /// Which REST operations are documented (default: all).
    #[serde(skip)]
    pub include: Option<ReferencePredicate>,
    #[serde(default)]
    pub references: Option<SkillReferencesOptions>,
    /// Hand-written files by path, byte for byte; one at a generated path
    /// replaces it.
    #[serde(default)]
    pub extras: Option<BTreeMap<String, String>>,
    /// Each operation's "Example request" (default [`ExampleRequest::Auto`]).
    #[serde(default)]
    pub example_request: Option<ExampleRequest>,
}

/// Where an operation's "Example request" comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExampleRequest {
    /// The first request example the references carry (a shell / curl tab
    /// when there is one), else a synthesized curl request.
    #[default]
    Auto,
    /// Always a synthesized curl request: the required inputs plus `limit`,
    /// each with its example or default when it states one (`limit`: else 10)
    /// and a typed placeholder otherwise.
    Synthesized,
    /// No example request.
    None,
}

impl fmt::Debug for SkillOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillOptions")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("title", &self.title)
            .field("intro", &self.intro)
            .field("group_by", &self.group_by.as_ref().map(|_| "<fn>"))
            .field("include", &self.include.as_ref().map(|_| "<fn>"))
            .field(
                "references.dir",
                &self.references.as_ref().and_then(|r| r.dir.as_ref()),
            )
            .field(
                "references.header",
                &self
                    .references
                    .as_ref()
                    .and_then(|r| r.header.as_ref())
                    .map(|_| "<fn>"),
            )
            .field(
                "extras",
                &self.extras.as_ref().map(|e| e.keys().collect::<Vec<_>>()),
            )
            .field("example_request", &self.example_request)
            .finish()
    }
}

pub const DEFAULT_SKILL_HEADER: &str = "Generated from the API reference. Do not edit by hand.";

const DEFAULT_GROUP: &str = "general";

struct Group<'a> {
    name: String,
    /// The group's slug: its file's name (`<slug>.md`), or its directory.
    slug: String,
    file: String,
    operations: Vec<&'a Value>,
    /// Per operation, its own file under the group directory.
    files: Vec<String>,
    taken_files: HashSet<String>,
}

/// The files of an agent skill documenting the REST operations in
/// `references`, by path relative to the skill root.
pub fn skill_files(references: &[Value], options: SkillOptions) -> BTreeMap<String, String> {
    let dir = {
        let mut d = options
            .references
            .as_ref()
            .and_then(|r| r.dir.as_deref())
            .unwrap_or("references");
        while let Some(rest) = d.strip_prefix("./") {
            d = rest;
        }
        d.trim_matches('/').to_string()
    };
    let header = |group: &str| -> String {
        match options.references.as_ref().and_then(|r| r.header.as_ref()) {
            Some(h) => h(group),
            None => DEFAULT_SKILL_HEADER.to_string(),
        }
    };
    let per_operation = options
        .references
        .as_ref()
        .and_then(|r| r.per_operation)
        .unwrap_or(false);
    let example_mode = options.example_request.unwrap_or_default();
    let at = |file: &str| -> String {
        if dir.is_empty() {
            file.to_string()
        } else {
            format!("{dir}/{file}")
        }
    };

    let mut groups: Vec<Group<'_>> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut taken: HashSet<String> = HashSet::from(["index".to_string()]);
    for reference in references {
        if !is_rest_operation(reference) {
            continue;
        }
        if let Some(include) = &options.include {
            if !include(reference) {
                continue;
            }
        }
        let mut name = match &options.group_by {
            Some(g) => g(reference),
            None => first_group(reference),
        };
        if name.is_empty() {
            name = DEFAULT_GROUP.to_string();
        }
        let index = match by_name.get(&name) {
            Some(i) => *i,
            None => {
                let slug = unique_slug(&name, &mut taken);
                groups.push(Group {
                    name: name.clone(),
                    file: format!("{slug}.md"),
                    slug,
                    operations: Vec::new(),
                    files: Vec::new(),
                    taken_files: HashSet::new(),
                });
                by_name.insert(name, groups.len() - 1);
                groups.len() - 1
            }
        };
        let group = &mut groups[index];
        group.operations.push(reference);
        let file_name = operation_file_name(&operation_id(reference), &mut group.taken_files);
        group.files.push(format!("{}/{file_name}.md", group.slug));
    }

    let mut files = BTreeMap::new();
    files.insert(
        "SKILL.md".to_string(),
        skill_md(&options, &groups, &at, per_operation),
    );
    files.insert(
        at("index.md"),
        index_md(&groups, &header("index"), per_operation),
    );
    for group in &groups {
        if per_operation {
            for (reference, file) in group.operations.iter().zip(&group.files) {
                let mut blocks = vec![comment(&header(&group.name))];
                blocks.extend(operation_blocks(reference, 1, example_mode));
                files.insert(at(file), blocks.join("\n\n") + "\n");
            }
        } else {
            files.insert(
                at(&group.file),
                group_md(group, &header(&group.name), example_mode),
            );
        }
    }
    for (path, contents) in options.extras.iter().flatten() {
        files.insert(path.clone(), contents.clone());
    }
    files
}

/// An operation's file name: its id with every run of characters outside
/// `[A-Za-z0-9._-]` a `-`, `-` and `.` trimmed from both ends (`operation`
/// when nothing is left), case kept; `-2`, `-3`… against the names taken in
/// its group, compared without case (case-insensitive file systems).
fn operation_file_name(id: &str, taken: &mut HashSet<String>) -> String {
    let mut name = String::new();
    let mut in_run = false;
    for c in id.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            name.push(c);
            in_run = false;
        } else if !in_run {
            name.push('-');
            in_run = true;
        }
    }
    let name = name.trim_matches(|c| c == '-' || c == '.');
    let name = if name.is_empty() { "operation" } else { name };
    let mut candidate = name.to_string();
    let mut n = 2;
    while taken.contains(&candidate.to_ascii_lowercase()) {
        candidate = format!("{name}-{n}");
        n += 1;
    }
    taken.insert(candidate.to_ascii_lowercase());
    candidate
}

/// A file-name slug: ASCII letters and digits lower-cased, every other run a
/// `-`; `group` when nothing is left; `-2`, `-3`… against the slugs taken.
fn unique_slug(name: &str, taken: &mut HashSet<String>) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        let next = if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            '-'
        };
        if next == '-' && slug.ends_with('-') {
            continue;
        }
        slug.push(next);
    }
    let slug = slug.strip_prefix('-').unwrap_or(&slug);
    let slug = slug.strip_suffix('-').unwrap_or(slug);
    let slug = if slug.is_empty() { "group" } else { slug };
    let mut candidate = slug.to_string();
    let mut n = 2;
    while taken.contains(&candidate) {
        candidate = format!("{slug}-{n}");
        n += 1;
    }
    taken.insert(candidate.clone());
    candidate
}

// ---------------------------------------------------------------------------
// SKILL.md and the index
// ---------------------------------------------------------------------------

fn skill_md(
    options: &SkillOptions,
    groups: &[Group<'_>],
    at: &dyn Fn(&str) -> String,
    per_operation: bool,
) -> String {
    let mut blocks: Vec<String> = vec![format!(
        "---\nname: {}\ndescription: {}\n---",
        yaml_scalar(&options.name),
        yaml_scalar(&options.description)
    )];
    if let Some(title) = options.title.as_deref().filter(|t| !t.is_empty()) {
        blocks.push(format!("# {}", collapse(title)));
    }

    let mut sections: Vec<String> = Vec::new();
    let mut table = vec![table_header(&[
        "Group",
        "Operations",
        if per_operation { "Directory" } else { "File" },
    ])];
    for group in groups {
        let place = if per_operation {
            inline_code(&(at(&group.slug) + "/"))
        } else {
            let file = at(&group.file);
            link(&file, &file)
        };
        table.push(row(&[
            cell(&group.name),
            group.operations.len().to_string(),
            cell(&place),
        ]));
    }
    sections.push("## API reference".into());
    sections.push(table.join("\n"));
    let index = at("index.md");
    sections.push(format!(
        "Every operation in one table: {}.",
        link(&index, &index)
    ));

    let extras: Vec<&String> = options
        .extras
        .iter()
        .flat_map(|e| e.keys())
        .filter(|p| p.as_str() != "SKILL.md")
        .collect();
    if !extras.is_empty() {
        sections.push("## Other files".into());
        sections.push(
            extras
                .iter()
                .map(|p| format!("- {}", link(p, p)))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    let mut out = blocks.join("\n\n") + "\n\n";
    if let Some(intro) = options.intro.as_deref().filter(|i| !i.is_empty()) {
        out.push_str(intro);
        out.push_str(if intro.ends_with('\n') { "\n" } else { "\n\n" });
    }
    out + &sections.join("\n\n") + "\n"
}

/// Every operation, by group. Per operation, the operation links to its file
/// (and the group is plain text); else the group links to its file.
fn index_md(groups: &[Group<'_>], header: &str, per_operation: bool) -> String {
    let mut lines = vec![table_header(&[
        "Operation",
        "Method",
        "Path",
        "Group",
        "Title",
    ])];
    for group in groups {
        for (reference, file) in group.operations.iter().zip(&group.files) {
            let id = operation_id(reference);
            lines.push(row(&[
                cell(&if per_operation {
                    code_link(&id, file)
                } else {
                    inline_code(&id)
                }),
                operation_method(reference),
                cell(&inline_code(&operation_path(reference))),
                cell(&if per_operation {
                    group.name.clone()
                } else {
                    link(&group.name, &group.file)
                }),
                cell(str_of(reference, "title").unwrap_or("")),
            ]));
        }
    }
    [comment(header), "# Operations".into(), lines.join("\n")].join("\n\n") + "\n"
}

fn operation_id(reference: &Value) -> String {
    match str_of(reference, "canonical").filter(|c| !c.is_empty()) {
        Some(c) => c.to_string(),
        None => format!(
            "{} {}",
            operation_method(reference),
            operation_path(reference)
        ),
    }
}

// ---------------------------------------------------------------------------
// a group's reference file
// ---------------------------------------------------------------------------

fn group_md(group: &Group<'_>, header: &str, example_mode: ExampleRequest) -> String {
    let mut blocks = vec![comment(header), format!("# {}", collapse(&group.name))];
    for reference in &group.operations {
        blocks.extend(operation_blocks(reference, 2, example_mode));
    }
    blocks.join("\n\n") + "\n"
}

/// An operation's blocks, its heading at `level` (2 in a group's file, 1 in
/// its own) and its sections one level below.
fn operation_blocks(reference: &Value, level: usize, example_mode: ExampleRequest) -> Vec<String> {
    let h = "#".repeat(level);
    let sub = "#".repeat(level + 1);
    let method = operation_method(reference);
    let path = operation_path(reference);
    let title = collapse(str_of(reference, "title").unwrap_or(""));
    let mut blocks = Vec::new();

    let id = operation_id(reference);
    blocks.push(if !title.is_empty() && title != id {
        format!("{h} {} — {title}", inline_code(&id))
    } else {
        format!("{h} {}", inline_code(&id))
    });
    blocks.push(inline_code(&format!("{method} {path}")));

    let description = js_trim(text(reference.get("description")));
    if !description.is_empty() && description != js_trim(str_of(reference, "title").unwrap_or("")) {
        blocks.push(description.to_string());
    }

    for (ty, heading) in [
        ("$rest.param.path", "Path parameters"),
        ("$rest.param.query", "Query parameters"),
        ("$rest.param.header", "Headers"),
        ("$rest.param.cookie", "Cookies"),
    ] {
        let props = definition_properties(reference, ty);
        if !props.is_empty() {
            blocks.push(format!("{sub} {heading}"));
            blocks.push(property_table(&props, None));
        }
    }

    if let Some(body) = request_body(reference, false) {
        blocks.push(format!("{sub} Request body"));
        let mut facts = Vec::new();
        if !body.content_type.is_empty() {
            facts.push(format!(
                "Content type: {}.",
                inline_code(&body.content_type)
            ));
        }
        facts.push(
            if body.required {
                "Required."
            } else {
                "Optional."
            }
            .to_string(),
        );
        blocks.push(facts.join(" "));
        if !js_trim(&body.description).is_empty() {
            blocks.push(collapse(&body.description));
        }
        if let Some(root) = body.root_property {
            blocks.push(property_table(&[root], Some(("(body)", body.required))));
        } else if !body.properties.is_empty() {
            blocks.push(property_table(&body.properties, None));
        }
    }

    let variants = response_variants(reference);
    if !variants.is_empty() {
        let mut lines = vec![table_header(&[
            "Status",
            "Content type",
            "Type",
            "Description",
        ])];
        for variant in variants {
            lines.push(response_row(variant));
        }
        blocks.push(format!("{sub} Responses"));
        blocks.push(lines.join("\n"));
        // Each response's fields, one level deep, like the request body's.
        for variant in response_variants(reference) {
            if let Some(fields) = response_fields(variant) {
                blocks.push(format!("{sub}# {}", response_heading(variant)));
                blocks.push(fields);
            }
        }
    }

    let example = match example_mode {
        ExampleRequest::None => None,
        ExampleRequest::Auto => Some(
            example_request(reference)
                .map(|(code, language)| (code.to_string(), language.to_string()))
                .unwrap_or_else(|| synthesized_request(reference)),
        ),
        ExampleRequest::Synthesized => Some(synthesized_request(reference)),
    };
    if let Some((code, language)) = example {
        blocks.push(format!("{sub} Example request"));
        blocks.push(code_block(&code, &language));
    }

    blocks
}

// ---------------------------------------------------------------------------
// property tables
// ---------------------------------------------------------------------------

/// Name, type, required, description — each property, then its object
/// children one level down as `name.child` (`name[].child` for an array of
/// objects). `root` names a lone root property (a body that is not an object).
fn property_table(props: &[&Value], root: Option<(&str, bool)>) -> String {
    let mut lines = vec![table_header(&["Name", "Type", "Required", "Description"])];
    for prop in props {
        let name = if root.is_some() {
            ""
        } else {
            str_of(prop, "name").unwrap_or("")
        };
        if root.is_none() && name.is_empty() {
            continue;
        }
        let name_cell = match root {
            Some((root_name, _)) => cell(root_name),
            None => cell(&inline_code(name)),
        };
        let required = match root {
            Some((_, required)) => required,
            None => is_required(prop),
        };
        lines.push(property_row(&name_cell, prop, required));

        let eff = effective_property(prop);
        let (children, joiner): (Vec<Value>, &str) = if str_of(&eff, "type") == Some("$$array") {
            let children = array_item(&eff)
                .map(|item| object_children(&effective_property(&item)).to_vec())
                .unwrap_or_default();
            (children, "[].")
        } else {
            (object_children(&eff).to_vec(), ".")
        };
        for child in &children {
            let child_name = str_of(child, "name").unwrap_or("");
            if child_name.is_empty() {
                continue;
            }
            lines.push(property_row(
                &cell(&inline_code(&format!("{name}{joiner}{child_name}"))),
                child,
                is_required(child),
            ));
        }
    }
    lines.join("\n")
}

fn property_row(name_cell: &str, prop: &Value, required: bool) -> String {
    row(&[
        name_cell.to_string(),
        cell(&type_label(prop)),
        if required { "yes" } else { "no" }.to_string(),
        cell(&describe(prop)),
    ])
}

/// A property's description plus what its meta says.
fn describe(raw: &Value) -> String {
    let prop = effective_property(raw);
    let meta = prop.get("meta");
    let mut parts: Vec<String> = Vec::new();
    if meta_flag(meta, "deprecated") {
        parts.push("Deprecated.".into());
    }
    let description = collapse(text(prop.get("description")));
    let mut facts: Vec<String> = Vec::new();
    match str_of(&prop, "type") {
        Some("$$enum") => facts.push(format!("One of: {}.", enum_values(&prop))),
        Some("$$array") => {
            if let Some(item) = array_item(&prop) {
                let eff = effective_property(&item);
                if str_of(&eff, "type") == Some("$$enum") {
                    facts.push(format!("Each one of: {}.", enum_values(&eff)));
                }
            }
        }
        _ => {}
    }
    for (name, label) in [
        ("defaults", "Default"),
        ("minimum", "Minimum"),
        ("maximum", "Maximum"),
    ] {
        if let Some(v) = meta_value(meta, name) {
            facts.push(format!("{label}: {}.", code_value(v)));
        }
    }
    if let Some(example) = first_example(&prop) {
        facts.push(format!("Example: {}.", code_value(&example)));
    }
    if !description.is_empty() {
        // A sentence of its own before the facts that follow it.
        if !facts.is_empty() && !description.ends_with(['.', '!', '?']) {
            parts.push(format!("{description}."));
        } else {
            parts.push(description);
        }
    }
    parts.extend(facts);
    parts.join(" ")
}

fn enum_values(prop: &Value) -> String {
    array_of(prop, "properties")
        .iter()
        .map(|v| code_value(&Value::String(str_of(v, "name").unwrap_or("").to_string())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A value as inline code; the empty string reads `""`.
fn code_value(value: &Value) -> String {
    let text = value_text(value);
    inline_code(if text.is_empty() { "\"\"" } else { &text })
}

fn first_example(prop: &Value) -> Option<Value> {
    let meta = prop.get("meta");
    if let Some(v) = find_meta(meta, "examples").and_then(|m| m.get("value")) {
        return match v {
            Value::Array(items) => items.first().cloned(),
            other => Some(other.clone()),
        };
    }
    if let Some(v) = find_meta(meta, "example").and_then(|m| m.get("value")) {
        return Some(v.clone());
    }
    match prop.get("examples") {
        Some(Value::Array(items)) => items.first().cloned(),
        Some(Value::String(s)) if !s.is_empty() => Some(Value::String(s.clone())),
        _ => None,
    }
}

/// A value as text: strings as they are, everything else as JSON.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "null".into()),
    }
}

/// A short type: `string`, `array of Pet`, `Card or Transfer`, `… or null`.
pub fn type_label(raw: &Value) -> String {
    let prop = effective_property(raw);
    let mut label = match str_of(&prop, "type").unwrap_or("") {
        "$$enum" => {
            let base = enum_base_type(&prop);
            if base.is_empty() {
                "enum".to_string()
            } else {
                base
            }
        }
        "$$array" => match array_item(&prop) {
            Some(item) => format!("array of {}", type_label(&item)),
            None => "array".to_string(),
        },
        "$$union" | "$$xor" => {
            let mut labels: Vec<String> = Vec::new();
            for alt in array_of(&prop, "properties") {
                let l = type_label(alt);
                if !labels.contains(&l) {
                    labels.push(l);
                }
            }
            if labels.is_empty() {
                "any".to_string()
            } else {
                labels.join(" or ")
            }
        }
        "$$function" => "function".to_string(),
        ty => {
            let children = object_children(&prop);
            let symbol = prop
                .get("symbolDef")
                .and_then(|s| str_of(s, "label"))
                .filter(|l| !l.is_empty());
            match symbol {
                Some(symbol) if ty == "object" || !children.is_empty() => symbol.to_string(),
                _ => match json_schema_type(prop.get("type")) {
                    Some(t) => t.to_string(),
                    None if !children.is_empty() => "object".to_string(),
                    None if !ty.is_empty() => ty.to_string(),
                    None => "any".to_string(),
                },
            }
        }
    };
    if meta_flag(prop.get("meta"), "nullable") && !label.split(" or ").any(|l| l == "null") {
        label.push_str(" or null");
    }
    label
}

// ---------------------------------------------------------------------------
// responses and the example request
// ---------------------------------------------------------------------------

fn response_row(variant: &Value) -> String {
    let status = meta_value(variant.get("meta"), "status")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| str_of(variant, "title").unwrap_or(""));
    let content_type = variant_content_type(variant);
    let mut ty = "—".to_string();
    if !content_type.is_empty() {
        let root = object_at(variant, "rootProperty");
        let symbol = variant
            .get("symbolDef")
            .and_then(|s| str_of(s, "label"))
            .filter(|l| !l.is_empty());
        if let Some(root) = root.filter(|r| str_of(r, "type") == Some("$$array")) {
            let item: Option<Cow<'_, Value>> = array_item(root);
            let item_label = item
                .as_ref()
                .and_then(|i| i.get("symbolDef"))
                .and_then(|s| str_of(s, "label"))
                .filter(|l| !l.is_empty())
                .map(String::from)
                .or_else(|| symbol.map(String::from))
                .or_else(|| item.as_ref().map(|i| type_label(i)))
                .unwrap_or_default();
            ty = if item_label.is_empty() {
                "array".into()
            } else {
                format!("array of {item_label}")
            };
        } else if let Some(root) = root {
            ty = type_label(root);
        } else if let Some(symbol) = symbol {
            ty = symbol.to_string();
        } else if array_of(variant, "properties")
            .iter()
            .any(|p| truthy(p.get("name")))
        {
            ty = "object".into();
        }
    }
    let own = text(variant.get("description"));
    let description = if own.is_empty() {
        variant
            .get("rootProperty")
            .map(|r| text(r.get("description")))
            .unwrap_or("")
    } else {
        own
    };
    row(&[
        cell(status),
        if content_type.is_empty() {
            "—".to_string()
        } else {
            cell(&inline_code(&content_type))
        },
        cell(&ty),
        cell(description),
    ])
}

const SHELL_LANGUAGES: &[&str] = &["shell", "bash", "sh", "curl"];

/// The first request example's tab — a shell (curl) one when there is one.
fn example_request(reference: &Value) -> Option<(&str, &str)> {
    let groups = reference
        .get("examples")
        .map(|e| array_of(e, "groups"))
        .unwrap_or(&[]);
    let group = groups
        .iter()
        .find(|g| str_of(g, "kind") == Some("request"))
        .or_else(|| {
            if groups.iter().any(|g| truthy(g.get("kind"))) {
                None
            } else {
                groups.first()
            }
        })?;
    let example = array_of(group, "examples").first()?;
    let tabs = example
        .get("codeblock")
        .map(|c| array_of(c, "tabs"))
        .unwrap_or(&[]);
    let tab = tabs
        .iter()
        .find(|t| {
            let language = str_of(t, "language").unwrap_or("").to_lowercase();
            let title = str_of(t, "title").unwrap_or("").to_lowercase();
            SHELL_LANGUAGES.contains(&language.as_str())
                || title.contains("curl")
                || title.contains("shell")
        })
        .or_else(|| tabs.first())?;
    Some((
        str_of(tab, "code").unwrap_or(""),
        str_of(tab, "language").unwrap_or(""),
    ))
}

/// `200 — \`application/json\``: a response's status and content type.
fn response_heading(variant: &Value) -> String {
    let status = meta_value(variant.get("meta"), "status")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| str_of(variant, "title").unwrap_or(""));
    let name = collapse(status);
    let content_type = variant_content_type(variant);
    if content_type.is_empty() {
        name
    } else {
        format!("{name} — {}", inline_code(&content_type))
    }
}

/// A response's fields (name, type, description), each with its object
/// children one level down — the root object's properties, or an array
/// root's item properties as `[].name`.
fn response_fields(variant: &Value) -> Option<String> {
    let named = |props: &[Value]| -> Vec<Value> {
        props
            .iter()
            .filter(|p| truthy(p.get("name")))
            .cloned()
            .collect()
    };
    let mut props = named(array_of(variant, "properties"));
    let mut prefix = "";
    if props.is_empty() {
        if let Some(root) = object_at(variant, "rootProperty") {
            let root = effective_property(root);
            if str_of(&root, "type") == Some("$$array") {
                props = array_item(&root)
                    .map(|item| named(object_children(&effective_property(&item))))
                    .unwrap_or_default();
                prefix = "[].";
            } else {
                props = named(object_children(&root));
            }
        }
    }
    if props.is_empty() {
        return None;
    }
    let field = |name: &str, prop: &Value| {
        row(&[
            cell(&inline_code(name)),
            cell(&type_label(prop)),
            cell(&describe(prop)),
        ])
    };
    let mut lines = vec![table_header(&["Name", "Type", "Description"])];
    for prop in &props {
        let name = format!("{prefix}{}", str_of(prop, "name").unwrap_or(""));
        lines.push(field(&name, prop));
        let eff = effective_property(prop);
        let (children, joiner): (Vec<Value>, &str) = if str_of(&eff, "type") == Some("$$array") {
            let children = array_item(&eff)
                .map(|item| object_children(&effective_property(&item)).to_vec())
                .unwrap_or_default();
            (children, "[].")
        } else {
            (object_children(&eff).to_vec(), ".")
        };
        for child in &children {
            let child_name = str_of(child, "name").unwrap_or("");
            if !child_name.is_empty() {
                lines.push(field(&format!("{name}{joiner}{child_name}"), child));
            }
        }
    }
    Some(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// the synthesized example request
// ---------------------------------------------------------------------------

/// How deep a synthesized body spells objects and arrays out.
const SAMPLE_DEPTH: usize = 3;
/// `limit` when it states no example or default.
const SAMPLE_LIMIT: u64 = 10;

/// A curl request sending only the required inputs plus `limit`.
fn synthesized_request(reference: &Value) -> (String, String) {
    let mut url = operation_full_path(reference).unwrap_or_else(|| operation_path(reference));
    for prop in definition_properties(reference, "$rest.param.path") {
        let name = str_of(prop, "name").unwrap_or("");
        if !name.is_empty() {
            url = url.replace(&format!("{{{name}}}"), &url_text(&param_text(prop)));
        }
    }
    let mut query: Vec<String> = Vec::new();
    for prop in definition_properties(reference, "$rest.param.query") {
        let name = str_of(prop, "name").unwrap_or("");
        if name.is_empty() || (!is_required(prop) && name != "limit") {
            continue;
        }
        let value = if name == "limit" {
            limit_value(prop)
        } else {
            stated_value(&effective_property(prop))
                .unwrap_or_else(|| Value::String(format!("<{name}>")))
        };
        let items = match value {
            Value::Array(items) => items,
            other => vec![other],
        };
        for item in items {
            query.push(format!(
                "{}={}",
                url_text(name),
                url_text(&value_text(&item))
            ));
        }
    }
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query.join("&"));
    }

    let mut lines = vec![
        format!("curl --request {}", operation_method(reference)),
        format!("--url '{url}'"),
    ];
    for prop in definition_properties(reference, "$rest.param.header") {
        let name = str_of(prop, "name").unwrap_or("");
        if !name.is_empty() && is_required(prop) {
            lines.push(format!(
                "--header '{}'",
                shell_text(&format!("{name}: {}", param_text(prop)))
            ));
        }
    }
    if let Some(body) = request_body(reference, false) {
        if body.required && is_json_content_type(&body.content_type) {
            let data = match body.root_property {
                Some(root) => sample_value(root, "body", 1),
                None => Value::Object(sample_object(&body.properties, 1, true)),
            };
            lines.push(format!(
                "--header '{}'",
                shell_text(&format!("content-type: {}", body.content_type))
            ));
            lines.push(format!(
                "--data '{}'",
                shell_text(&serde_json::to_string_pretty(&data).unwrap_or_default())
            ));
        }
    }
    (lines.join(" \\\n     "), "shell".to_string())
}

/// A parameter's value as text: what it states, else `<name>`.
fn param_text(prop: &Value) -> String {
    match stated_value(&effective_property(prop)) {
        Some(v) => value_text(&v),
        None => format!("<{}>", str_of(prop, "name").unwrap_or("")),
    }
}

fn limit_value(prop: &Value) -> Value {
    stated_value(&effective_property(prop)).unwrap_or_else(|| Value::from(SAMPLE_LIMIT))
}

/// What a property states as its value: its first example, else its default.
/// A text example or default of an object or array is not used.
fn stated_value(prop: &Value) -> Option<Value> {
    let ty = str_of(prop, "type");
    let structured =
        ty == Some("object") || ty == Some("$$array") || !object_children(prop).is_empty();
    if let Some(example) = first_example(prop) {
        if !(structured && example.is_string()) {
            return Some(example);
        }
    }
    match meta_value(prop.get("meta"), "defaults") {
        Some(v) if !(structured && v.is_string()) => Some(v.clone()),
        _ => None,
    }
}

/// A value for a property: what it states, else the first enum value or
/// union alternative, a one-item array, an object of its required
/// properties, or a typed placeholder.
fn sample_value(raw: &Value, name: &str, depth: usize) -> Value {
    let prop = effective_property(raw);
    if let Some(stated) = stated_value(&prop) {
        return stated;
    }
    match str_of(&prop, "type").unwrap_or("") {
        "$$enum" => {
            return match array_of(&prop, "properties").first() {
                Some(first) => {
                    enum_value(str_of(first, "name").unwrap_or(""), &enum_base_type(&prop))
                }
                None => Value::Null,
            };
        }
        "$$array" => {
            return match array_item(&prop) {
                Some(item) if depth < SAMPLE_DEPTH => {
                    Value::Array(vec![sample_value(&item, name, depth + 1)])
                }
                _ => Value::Array(vec![]),
            };
        }
        "$$union" | "$$xor" => {
            return match array_of(&prop, "properties").first() {
                Some(first) => sample_value(first, name, depth),
                None => Value::Null,
            };
        }
        _ => {}
    }
    let children = object_children(&prop);
    let ty = str_of(&prop, "type").unwrap_or("");
    if !children.is_empty() || ty == "object" {
        if depth < SAMPLE_DEPTH {
            let children: Vec<&Value> = children.iter().collect();
            return Value::Object(sample_object(&children, depth + 1, false));
        }
        return Value::Object(Map::new());
    }
    match ty {
        "integer" | "number" => Value::from(0),
        "boolean" => Value::Bool(false),
        "null" => Value::Null,
        _ => Value::String(format!(
            "<{}>",
            if name.is_empty() { "value" } else { name }
        )),
    }
}

/// An object of the required properties (and `limit`, at the top level).
fn sample_object(props: &[&Value], depth: usize, with_limit: bool) -> Map<String, Value> {
    let mut out = Map::new();
    for prop in props {
        let name = str_of(prop, "name").unwrap_or("");
        if name.is_empty() || out.contains_key(name) {
            continue;
        }
        if with_limit && name == "limit" {
            out.insert(name.to_string(), limit_value(prop));
        } else if is_required(prop) {
            out.insert(name.to_string(), sample_value(prop, name, depth));
        }
    }
    js_ordered(out)
}

/// Percent-encodes what would break the URL or its single quotes.
fn url_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            '&' => out.push_str("%26"),
            '\'' => out.push_str("%27"),
            '+' => out.push_str("%2B"),
            '?' => out.push_str("%3F"),
            other => out.push(other),
        }
    }
    out
}

/// Text inside a single-quoted shell string.
fn shell_text(value: &str) -> String {
    value.replace('\'', "'\\''")
}

// ---------------------------------------------------------------------------
// markdown and YAML text (src/skill/markdown.ts)
// ---------------------------------------------------------------------------

/// Inline code that survives backticks in its text.
pub fn inline_code(value: &str) -> String {
    let longest = longest_run(value, '`');
    if longest == 0 {
        return format!("`{value}`");
    }
    let fence = "`".repeat(longest + 1);
    let pad = if value.starts_with('`') || value.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{value}{pad}{fence}")
}

/// A fenced code block whose fence outlasts any backtick run in the code.
pub fn code_block(code: &str, language: &str) -> String {
    let fence = "`".repeat((longest_run(code, '`') + 1).max(3));
    let info = if language.contains('`') {
        String::new()
    } else {
        collapse(language)
    };
    let newline = if code.ends_with('\n') { "" } else { "\n" };
    format!("{fence}{info}\n{code}{newline}{fence}")
}

fn longest_run(value: &str, ch: char) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for c in value.chars() {
        current = if c == ch { current + 1 } else { 0 };
        longest = longest.max(current);
    }
    longest
}

/// Whitespace runs (newlines included) become one space; trimmed.
pub fn collapse(value: &str) -> String {
    value
        .split(is_js_whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A table cell: one line, pipes escaped.
pub fn cell(value: &str) -> String {
    collapse(value).replace('|', "\\|")
}

fn row(cells: &[String]) -> String {
    format!("| {} |", cells.join(" | "))
}

fn table_header(names: &[&str]) -> String {
    let cells: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    format!(
        "{}\n|{}|",
        row(&cells),
        names.iter().map(|_| "---").collect::<Vec<_>>().join("|")
    )
}

/// A link to a relative path, the target in angle brackets when it holds
/// whitespace or parentheses.
fn link(label: &str, target: &str) -> String {
    let text = label.replace('[', "\\[").replace(']', "\\]");
    format!("[{text}]({})", link_target(target))
}

/// A link whose label is inline code, nothing escaped inside the code span.
fn code_link(code: &str, target: &str) -> String {
    format!("[{}]({})", inline_code(code), link_target(target))
}

fn link_target(target: &str) -> String {
    if target
        .chars()
        .any(|c| is_js_whitespace(c) || c == '(' || c == ')')
    {
        format!("<{target}>")
    } else {
        target.to_string()
    }
}

/// An HTML comment whose text cannot close it early.
fn comment(value: &str) -> String {
    format!("<!-- {} -->", collapse(value).replace("-->", "-- >"))
}

const YAML_INDICATORS: &str = "-?:,[]{}#&*!|>'\"%@`";
const YAML_RESERVED: &[&str] = &[
    "true", "false", "yes", "no", "on", "off", "y", "n", "null", "~",
];

/// A YAML scalar: plain when that reads back as the same string, else
/// double-quoted (JSON escapes are valid YAML escapes).
pub fn yaml_scalar(value: &str) -> String {
    if needs_quotes(value) {
        serde_json::to_string(value).unwrap_or_default()
    } else {
        value.to_string()
    }
}

fn needs_quotes(value: &str) -> bool {
    let (Some(first), Some(last)) = (value.chars().next(), value.chars().last()) else {
        return true;
    };
    if YAML_INDICATORS.contains(first)
        || is_js_whitespace(first)
        || is_js_whitespace(last)
        || last == ':'
    {
        return true;
    }
    if value.contains(": ") || value.contains(" #") {
        return true;
    }
    if value.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f) {
        return true;
    }
    if YAML_RESERVED.contains(&value.to_lowercase().as_str()) {
        return true;
    }
    let second = value.chars().nth(1);
    first.is_ascii_digit()
        || ((first == '+' || first == '.') && second.is_some_and(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_code_fences_around_backticks() {
        assert_eq!(inline_code("plain"), "`plain`");
        assert_eq!(inline_code("a `b` c"), "``a `b` c``");
        assert_eq!(inline_code("`x`"), "`` `x` ``");
    }

    #[test]
    fn cells_are_one_escaped_line() {
        assert_eq!(cell("a | b\n  c"), "a \\| b c");
    }

    #[test]
    fn code_blocks_outlast_backtick_runs() {
        assert_eq!(code_block("echo ```", "shell"), "````shell\necho ```\n````");
    }

    #[test]
    fn yaml_scalars_quote_only_when_needed() {
        assert_eq!(yaml_scalar("pet-store"), "pet-store");
        assert_eq!(yaml_scalar("Use it: now"), "\"Use it: now\"");
        assert_eq!(yaml_scalar("yes"), "\"yes\"");
        assert_eq!(yaml_scalar("1.0"), "\"1.0\"");
        assert_eq!(yaml_scalar("- item"), "\"- item\"");
        assert_eq!(yaml_scalar("ends with:"), "\"ends with:\"");
        assert_eq!(yaml_scalar("say \"hi\""), "say \"hi\"");
    }

    #[test]
    fn empty_values_render_as_quoted_strings() {
        assert_eq!(code_value(&Value::String(String::new())), "`\"\"`");
        assert_eq!(code_value(&Value::Null), "`null`");
        assert_eq!(code_value(&Value::from(20)), "`20`");
    }

    #[test]
    fn shell_and_url_text_escape_quotes() {
        assert_eq!(shell_text("it's"), "it'\\''s");
        assert_eq!(url_text("rex's & co?"), "rex%27s%20%26%20co%3F");
    }

    #[test]
    fn operation_files_keep_case_and_stay_apart() {
        let mut taken = HashSet::new();
        assert_eq!(operation_file_name("getThing", &mut taken), "getThing");
        assert_eq!(operation_file_name("GetThing", &mut taken), "GetThing-2");
        assert_eq!(
            operation_file_name("things/list", &mut taken),
            "things-list"
        );
        assert_eq!(operation_file_name("../up", &mut taken), "up");
        assert_eq!(operation_file_name("///", &mut taken), "operation");
    }

    #[test]
    fn groups_never_take_the_index() {
        let mut taken = HashSet::from(["index".to_string()]);
        assert_eq!(unique_slug("Index", &mut taken), "index-2");
        assert_eq!(unique_slug("index!", &mut taken), "index-3");
        assert_eq!(
            unique_slug("Store / Inventory", &mut taken),
            "store-inventory"
        );
        assert_eq!(unique_slug("ü", &mut taken), "group");
    }
}
