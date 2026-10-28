//! `Reference[]` → MCP tool definitions — the twin of
//! `packages/apitoolchain-uniform/src/mcp.ts` (`uniformToMcpTools`), pinned to
//! it byte for byte by the `5.mcp-tools.*` fixtures.
//!
//! One tool per REST operation (category `rest`, a `rest_<method>` type);
//! component schemas and every other category are skipped. Output follows the
//! references' order; object properties keep document order.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::operations::{
    array_item, array_of, definition_properties, effective_property, enum_base_type, enum_value,
    find_meta, is_required, is_rest_operation, js_ordered, js_trim, js_trim_end, json_schema_type,
    meta_flag, meta_value, object_children, operation_full_path, operation_group, operation_method,
    operation_path, request_body, str_of, text, unwrap_soft_breaks,
};

/// A predicate over a reference (a raw `Reference` JSON value).
pub type ReferencePredicate = Arc<dyn Fn(&Value) -> bool + Send + Sync>;
/// A tool name for a reference.
pub type ReferenceToolName = Arc<dyn Fn(&Value) -> String + Send + Sync>;
/// Extra data for a reference's tool.
pub type ReferenceExtras = Arc<dyn Fn(&Value) -> Option<Map<String, Value>> + Send + Sync>;

/// MCP tool annotations (hints a client may show or act on).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolAnnotations {
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
    pub open_world_hint: bool,
}

/// The kind of request body a tool sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpToolBody {
    #[serde(rename = "json")]
    Json,
}

/// Where a renamed input goes on the HTTP request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpInputLocation {
    Path,
    Query,
    Body,
}

impl McpInputLocation {
    fn as_str(self) -> &'static str {
        match self {
            McpInputLocation::Path => "path",
            McpInputLocation::Query => "query",
            McpInputLocation::Body => "body",
        }
    }
}

/// A renamed input: where it goes and under which name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolInput {
    #[serde(rename = "in")]
    pub location: McpInputLocation,
    pub name: String,
}

/// The renamed inputs by input name, in the order they were emitted (a JSON
/// object on the wire).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpToolInputs(pub Vec<(String, McpToolInput)>);

impl McpToolInputs {
    /// The input's entry, when it was renamed.
    pub fn get(&self, input: &str) -> Option<&McpToolInput> {
        self.0.iter().find(|(k, _)| k == input).map(|(_, v)| v)
    }
}

impl Serialize for McpToolInputs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(k, v)| (k, v)))
    }
}

impl<'de> Deserialize<'de> for McpToolInputs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InputsVisitor;
        impl<'de> Visitor<'de> for InputsVisitor {
            type Value = McpToolInputs;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of input names to { in, name }")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, McpToolInput>()? {
                    out.push((k, v));
                }
                Ok(McpToolInputs(out))
            }
        }
        deserializer.deserialize_map(InputsVisitor)
    }
}

/// How a call maps back onto the HTTP request. Not part of the MCP tool
/// definition: a server keeps it and strips it before listing the tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolBinding {
    /// Upper-case HTTP method.
    pub method: String,
    /// The operation path with its `{param}` placeholders.
    pub path: String,
    /// The first server's URL joined with the path (the path when unknown).
    pub full_path: String,
    /// The input properties that fill the path placeholders (input names —
    /// see `inputs` for one that was renamed).
    pub path_params: Vec<String>,
    /// The input properties sent as the query string (input names — see
    /// `inputs` for one that was renamed).
    pub query_params: Vec<String>,
    /// `json` when the operation takes a JSON request body (`null` otherwise).
    pub body: Option<McpToolBody>,
    /// The one input property holding the whole JSON body; absent when the
    /// body's properties sit at the top level (see the TypeScript docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_property: Option<String>,
    /// The inputs renamed in the flat layout (`<name>_query`, `<name>_body`)
    /// mapped back to their location and name; absent when none was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<McpToolInputs>,
    /// `context.group` without empty entries.
    pub group: Vec<String>,
    /// The reference's canonical.
    pub canonical: String,
}

/// One MCP tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub title: String,
    pub description: String,
    /// A JSON Schema object (`type: object`, `additionalProperties: false`).
    pub input_schema: Value,
    pub annotations: McpToolAnnotations,
    pub binding: McpToolBinding,
    /// What [`McpToolsOptions::extras`] returned for the reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Map<String, Value>>,
}

/// Options of [`uniform_to_mcp_tools`]. The data options deserialize from the
/// TypeScript option names (`flatten`, `limitCap`, `descriptionChars`,
/// `maxDepth`); the callbacks are set in code.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpToolsOptions {
    /// The tool name, used as given (an empty one falls back to the default:
    /// the canonical when it matches `^[A-Za-z0-9_-]{1,128}$`, else a slug).
    #[serde(skip)]
    pub tool_name: Option<ReferenceToolName>,
    /// Path, query and JSON-body properties at the top level (default `true`).
    pub flatten: Option<bool>,
    /// Which REST operations become tools. Default: every one whose request
    /// body, if any, has a JSON variant; a predicate given here is the whole
    /// decision (an operation it includes without a JSON body gets no body
    /// input and `body: null`).
    #[serde(skip)]
    pub include: Option<ReferencePredicate>,
    /// `readOnlyHint` (default: GET, HEAD, OPTIONS, TRACE).
    #[serde(skip)]
    pub read_only: Option<ReferencePredicate>,
    /// `destructiveHint` (default: DELETE, PUT, PATCH).
    #[serde(skip)]
    pub destructive: Option<ReferencePredicate>,
    /// `idempotentHint` (default: GET, HEAD, OPTIONS, TRACE, PUT, DELETE).
    #[serde(skip)]
    pub idempotent: Option<ReferencePredicate>,
    /// `openWorldHint` (default: false).
    #[serde(skip)]
    pub open_world: Option<ReferencePredicate>,
    /// Caps an integer parameter or top-level body property named `limit`.
    pub limit_cap: Option<u64>,
    /// The description's first sentence cut to this many characters
    /// (default 240; 0 = no cut).
    pub description_chars: Option<usize>,
    /// How deep objects are spelled out (default 7): a top-level input
    /// property is at depth 1; an object deeper than this loses its properties.
    pub max_depth: Option<usize>,
    /// Extra data carried on each tool as `extras`.
    #[serde(skip)]
    pub extras: Option<ReferenceExtras>,
}

impl fmt::Debug for McpToolsOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let set = |o: bool| if o { "Some(<fn>)" } else { "None" };
        f.debug_struct("McpToolsOptions")
            .field("tool_name", &set(self.tool_name.is_some()))
            .field("flatten", &self.flatten)
            .field("include", &set(self.include.is_some()))
            .field("read_only", &set(self.read_only.is_some()))
            .field("destructive", &set(self.destructive.is_some()))
            .field("idempotent", &set(self.idempotent.is_some()))
            .field("open_world", &set(self.open_world.is_some()))
            .field("limit_cap", &self.limit_cap)
            .field("description_chars", &self.description_chars)
            .field("max_depth", &self.max_depth)
            .field("extras", &set(self.extras.is_some()))
            .finish()
    }
}

const MAX_TOOL_NAME: usize = 128;
const SAFE_METHODS: &[&str] = &["GET", "HEAD", "OPTIONS", "TRACE"];
const DESTRUCTIVE_METHODS: &[&str] = &["DELETE", "PUT", "PATCH"];

/// A request body without a JSON variant (form, multipart, binary only).
fn only_non_json_body(reference: &Value) -> bool {
    request_body(reference, false).is_some() && request_body(reference, true).is_none()
}
const DEFAULT_DESCRIPTION_CHARS: usize = 240;
const DEFAULT_MAX_DEPTH: usize = 7;

struct Settings {
    flatten: bool,
    limit_cap: Option<u64>,
    description_chars: usize,
    max_depth: usize,
}

/// One MCP tool per REST operation in `references`, in reference order.
pub fn uniform_to_mcp_tools(
    references: &[Value],
    options: Option<McpToolsOptions>,
) -> Vec<McpTool> {
    let options = options.unwrap_or_default();
    let settings = Settings {
        flatten: options.flatten != Some(false),
        limit_cap: options.limit_cap.filter(|c| *c >= 1),
        description_chars: options
            .description_chars
            .unwrap_or(DEFAULT_DESCRIPTION_CHARS),
        max_depth: options.max_depth.unwrap_or(DEFAULT_MAX_DEPTH),
    };

    let mut used: HashSet<String> = HashSet::new();
    let mut tools = Vec::new();

    for reference in references {
        if !is_rest_operation(reference) {
            continue;
        }
        let included = match &options.include {
            Some(include) => include(reference),
            None => !only_non_json_body(reference),
        };
        if !included {
            continue;
        }

        let method = operation_method(reference);
        let path = operation_path(reference);
        let canonical = str_of(reference, "canonical").unwrap_or("").to_string();

        let custom = options
            .tool_name
            .as_ref()
            .map(|f| f(reference))
            .unwrap_or_default();
        let name = unique_name(
            if custom.is_empty() {
                default_tool_name(&canonical, &method, &path)
            } else {
                custom
            },
            &mut used,
        );

        let (input_schema, input) = input_of(reference, &settings);

        let title = str_of(reference, "title")
            .filter(|t| !t.is_empty())
            .map(String::from)
            .unwrap_or_else(|| name.clone());

        let flag = |predicate: &Option<ReferencePredicate>, default: bool| match predicate {
            Some(p) => p(reference),
            None => default,
        };

        let extras = options
            .extras
            .as_ref()
            .and_then(|f| f(reference))
            .filter(|m| !m.is_empty());

        tools.push(McpTool {
            description: tool_description(reference, &method, &path, settings.description_chars),
            input_schema,
            annotations: McpToolAnnotations {
                read_only_hint: flag(&options.read_only, SAFE_METHODS.contains(&method.as_str())),
                destructive_hint: flag(
                    &options.destructive,
                    DESTRUCTIVE_METHODS.contains(&method.as_str()),
                ),
                idempotent_hint: flag(
                    &options.idempotent,
                    SAFE_METHODS.contains(&method.as_str())
                        || method == "PUT"
                        || method == "DELETE",
                ),
                open_world_hint: flag(&options.open_world, false),
            },
            binding: McpToolBinding {
                full_path: operation_full_path(reference).unwrap_or_else(|| path.clone()),
                method,
                path,
                path_params: input.path_params,
                query_params: input.query_params,
                body: input.body,
                body_property: input.body_property,
                inputs: input.inputs,
                group: operation_group(reference),
                canonical,
            },
            name,
            title,
            extras,
        });
    }

    tools
}

// ---------------------------------------------------------------------------
// names and descriptions
// ---------------------------------------------------------------------------

/// `^[A-Za-z0-9_-]{1,128}$`
pub fn is_valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_TOOL_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn default_tool_name(canonical: &str, method: &str, path: &str) -> String {
    if is_valid_tool_name(canonical) {
        return canonical.to_string();
    }
    let slug = slug_tool_name(canonical);
    if !slug.is_empty() {
        return slug;
    }
    let slug = slug_tool_name(&format!("{method} {path}"));
    if !slug.is_empty() {
        return slug;
    }
    "tool".to_string()
}

/// Runs of characters outside `[A-Za-z0-9_-]` become one `_`; `_` trimmed at
/// both ends; at most 128 characters.
fn slug_tool_name(value: &str) -> String {
    let mut slug = String::new();
    let mut in_run = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            slug.push(c);
            in_run = false;
        } else if !in_run {
            slug.push('_');
            in_run = true;
        }
    }
    let slug = slug.trim_matches('_');
    let cut: String = slug.chars().take(MAX_TOOL_NAME).collect();
    cut.trim_end_matches('_').to_string()
}

fn unique_name(name: String, used: &mut HashSet<String>) -> String {
    let mut candidate = name.clone();
    let mut n = 2;
    while used.contains(&candidate) {
        let suffix = format!("_{n}");
        let keep = MAX_TOOL_NAME.saturating_sub(suffix.len());
        candidate = name.chars().take(keep).collect::<String>() + &suffix;
        n += 1;
    }
    used.insert(candidate.clone());
    candidate
}

fn tool_description(reference: &Value, method: &str, path: &str, chars: usize) -> String {
    // No title fallback: a REST reference without a description has no
    // summary either, so its title is the operationId or `method/path`.
    let mut summary = first_sentence(&first_line(&unwrap_soft_breaks(js_trim(text(
        reference.get("description"),
    )))));
    if chars > 0 {
        summary = truncate(&summary, chars);
    }
    if summary.is_empty() {
        format!("{method} {path}")
    } else {
        format!("{summary} — {method} {path}")
    }
}

fn first_line(value: &str) -> String {
    value
        .split('\n')
        .map(js_trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Up to the first sentence end, else the whole line: `.`, `!` or `?`
/// followed by the end, or by whitespace and the end or a sentence opener —
/// and, for a period, not after an abbreviation.
fn first_sentence(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if matches!(c, '.' | '!' | '?') && ends_sentence(&chars, i) {
            return chars[..=i].iter().collect();
        }
    }
    line.to_string()
}

/// Words whose period is not a sentence end (compared lower-case).
const ABBREVIATIONS: &[&str] = &[
    "approx", "ca", "cf", "excl", "fig", "incl", "no", "nr", "vs",
];
const SENTENCE_OPENERS: &str = "\"'\u{201C}\u{2018}([`";

fn is_blank(c: Option<&char>) -> bool {
    matches!(c, Some(' ' | '\t'))
}

fn ends_sentence(chars: &[char], i: usize) -> bool {
    let mut j = i + 1;
    if j < chars.len() {
        if !is_blank(chars.get(j)) {
            return false;
        }
        while is_blank(chars.get(j)) {
            j += 1;
        }
        if j < chars.len() && !opens_sentence(chars[j]) {
            return false;
        }
    }
    if chars[i] != '.' {
        return true;
    }
    let mut k = i;
    while k > 0 && !is_blank(chars.get(k - 1)) {
        k -= 1;
    }
    let word: String = chars[k..i].iter().collect();
    let word = word.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
    !(is_initials(word) || ABBREVIATIONS.contains(&word.to_lowercase().as_str()))
}

/// A single letter, or letters joined by periods (`e.g`, `i.e`, `U.S`):
/// `^[A-Za-z](\.[A-Za-z])*$`.
fn is_initials(word: &str) -> bool {
    !word.is_empty()
        && !word.ends_with('.')
        && word
            .as_bytes()
            .chunks(2)
            .all(|pair| pair[0].is_ascii_alphabetic() && pair.get(1).is_none_or(|b| *b == b'.'))
}

/// An uppercase letter (one with a lowercase form), a digit, a quote or a bracket.
fn opens_sentence(c: char) -> bool {
    c.is_ascii_digit() || SENTENCE_OPENERS.contains(c) || !c.to_lowercase().eq(std::iter::once(c))
}

/// At most `max` characters, the last one an ellipsis when cut.
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let kept: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", js_trim_end(&kept))
}

// ---------------------------------------------------------------------------
// the input schema
// ---------------------------------------------------------------------------

#[derive(Default)]
struct InputBinding {
    path_params: Vec<String>,
    query_params: Vec<String>,
    body: Option<McpToolBody>,
    body_property: Option<String>,
    inputs: Option<McpToolInputs>,
}

fn input_of(reference: &Value, settings: &Settings) -> (Value, InputBinding) {
    let path_props = definition_properties(reference, "$rest.param.path");
    let query_props = definition_properties(reference, "$rest.param.query");
    let body = request_body(reference, true);

    let mut binding = InputBinding {
        body: body.as_ref().map(|_| McpToolBody::Json),
        ..Default::default()
    };
    let mut properties: Map<String, Value> = Map::new();
    let mut required: Vec<String> = Vec::new();
    let mut additional_properties = Value::Bool(false);

    if settings.flatten {
        let mut inputs: Vec<(String, McpToolInput)> = Vec::new();
        add_parameters(
            &path_props,
            &mut properties,
            &mut required,
            Some(&mut binding.path_params),
            settings,
            Some((McpInputLocation::Path, &mut inputs)),
        );
        add_parameters(
            &query_props,
            &mut properties,
            &mut required,
            Some(&mut binding.query_params),
            settings,
            Some((McpInputLocation::Query, &mut inputs)),
        );

        if let Some(body) = &body {
            let names: Vec<&str> = body
                .properties
                .iter()
                .map(|p| str_of(p, "name").unwrap_or(""))
                .collect();
            let unique: HashSet<&str> = names.iter().copied().collect();
            let flattenable = body.root_property.is_none()
                && !body.properties.is_empty()
                && names.iter().all(|n| !n.is_empty())
                && unique.len() == names.len();

            if flattenable {
                // An optional body's required properties are required only
                // when the body is sent, so none is a required input.
                let mut optional = Vec::new();
                add_parameters(
                    &body.properties,
                    &mut properties,
                    if body.required {
                        &mut required
                    } else {
                        &mut optional
                    },
                    None,
                    settings,
                    Some((McpInputLocation::Body, &mut inputs)),
                );
                if let Some(open) = &body.additional_properties {
                    additional_properties = open.clone();
                }
            } else {
                let key = body_key(&properties);
                properties.insert(
                    key.clone(),
                    body_schema(
                        body.root_property,
                        &body.properties,
                        &body.description,
                        settings,
                        body.additional_properties.as_ref(),
                    ),
                );
                if body.required {
                    required.push(key.clone());
                }
                binding.body_property = Some(key);
            }
        }
        if !inputs.is_empty() {
            binding.inputs = Some(McpToolInputs(inputs));
        }
    } else {
        let header_props = definition_properties(reference, "$rest.param.header");
        let cookie_props = definition_properties(reference, "$rest.param.cookie");
        let groups: [(&str, &Vec<&Value>, u8); 4] = [
            ("pathParams", &path_props, 0),
            ("queryParams", &query_props, 1),
            ("headers", &header_props, 2),
            ("cookies", &cookie_props, 2),
        ];
        for (key, props, names) in groups {
            if props.is_empty() {
                continue;
            }
            let mut group_properties = Map::new();
            let mut group_required = Vec::new();
            let names = match names {
                0 => Some(&mut binding.path_params),
                1 => Some(&mut binding.query_params),
                _ => None,
            };
            add_parameters(
                props,
                &mut group_properties,
                &mut group_required,
                names,
                settings,
                None,
            );
            let has_required = !group_required.is_empty();
            properties.insert(
                key.to_string(),
                closed_object(group_properties, group_required, Value::Bool(false)),
            );
            if has_required {
                required.push(key.to_string());
            }
        }
        if let Some(body) = &body {
            properties.insert(
                "requestBody".into(),
                body_schema(
                    body.root_property,
                    &body.properties,
                    &body.description,
                    settings,
                    body.additional_properties.as_ref(),
                ),
            );
            if body.required {
                required.push("requestBody".into());
            }
            binding.body_property = Some("requestBody".into());
        }
    }

    (
        closed_object(properties, required, additional_properties),
        binding,
    )
}

/// `body`, else `requestBody`, else `body_2`, `body_3`… — whichever no
/// parameter has taken.
fn body_key(properties: &Map<String, Value>) -> String {
    if !properties.contains_key("body") {
        return "body".into();
    }
    if !properties.contains_key("requestBody") {
        return "requestBody".into();
    }
    let mut n = 2;
    while properties.contains_key(&format!("body_{n}")) {
        n += 1;
    }
    format!("body_{n}")
}

/// Adds one location's properties as inputs. A name repeated within the
/// location is skipped; a name an earlier location already took is renamed
/// `<name>_<location>` (`_2`, `_3`… if that is taken too) and recorded in
/// `renames` — only when given (the flat layout).
fn add_parameters(
    props: &[&Value],
    properties: &mut Map<String, Value>,
    required: &mut Vec<String>,
    mut names: Option<&mut Vec<String>>,
    settings: &Settings,
    mut renames: Option<(McpInputLocation, &mut Vec<(String, McpToolInput)>)>,
) {
    let mut seen: HashSet<&str> = HashSet::new();
    for prop in props {
        let name = str_of(prop, "name").unwrap_or("");
        if name.is_empty() || !seen.insert(name) {
            continue;
        }
        let mut input = name.to_string();
        if properties.contains_key(name) {
            let Some((location, inputs)) = renames.as_mut() else {
                continue;
            };
            input = format!("{name}_{}", location.as_str());
            let mut n = 2;
            while properties.contains_key(&input) {
                input = format!("{name}_{}_{n}", location.as_str());
                n += 1;
            }
            inputs.push((
                input.clone(),
                McpToolInput {
                    location: *location,
                    name: name.to_string(),
                },
            ));
        }
        properties.insert(
            input.clone(),
            property_schema(prop, 1, settings, true, "", None),
        );
        if let Some(names) = names.as_deref_mut() {
            names.push(input.clone());
        }
        if is_required(prop) {
            required.push(input);
        }
    }
}

fn closed_object(
    properties: Map<String, Value>,
    required: Vec<String>,
    additional_properties: Value,
) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), Value::String("object".into()));
    out.insert("properties".into(), Value::Object(js_ordered(properties)));
    if !required.is_empty() {
        out.insert(
            "required".into(),
            Value::Array(required.into_iter().map(Value::String).collect()),
        );
    }
    out.insert("additionalProperties".into(), additional_properties);
    Value::Object(out)
}

/// A body that is one input property: its root property, or an object of its
/// properties (open when it has none).
fn body_schema(
    root_property: Option<&Value>,
    props: &[&Value],
    description: &str,
    settings: &Settings,
    additional_properties: Option<&Value>,
) -> Value {
    let description = unwrap_soft_breaks(description);
    let description = description.as_str();
    if let Some(root) = root_property {
        return property_schema(root, 0, settings, false, description, None);
    }
    let mut fields = Fields {
        ty: Some(Value::String("object".into())),
        ..Default::default()
    };
    if !description.is_empty() {
        fields.description = Some(description.to_string());
    }
    if !props.is_empty() {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for prop in props {
            let name = str_of(prop, "name").unwrap_or("");
            if name.is_empty() || properties.contains_key(name) {
                continue;
            }
            properties.insert(
                name.to_string(),
                property_schema(prop, 1, settings, true, "", None),
            );
            if is_required(prop) {
                required.push(name.to_string());
            }
        }
        fields.properties = Some(properties);
        if !required.is_empty() {
            fields.required = Some(required);
        }
    }
    fields.additional_properties = additional_properties.cloned();
    fields.emit()
}

/// The fields of a JSON Schema node, emitted in this fixed order.
#[derive(Default)]
struct Fields {
    ty: Option<Value>,
    title: Option<String>,
    description: Option<String>,
    enum_values: Option<Vec<Value>>,
    constant: Option<Value>,
    items: Option<Value>,
    properties: Option<Map<String, Value>>,
    required: Option<Vec<String>>,
    additional_properties: Option<Value>,
    any_of: Option<Vec<Value>>,
    one_of: Option<Vec<Value>>,
    default: Option<Value>,
    minimum: Option<Value>,
    maximum: Option<Value>,
    examples: Option<Vec<Value>>,
    deprecated: Option<bool>,
}

impl Fields {
    fn emit(self) -> Value {
        let mut out = Map::new();
        if let Some(v) = self.ty {
            out.insert("type".into(), v);
        }
        if let Some(v) = self.title {
            out.insert("title".into(), Value::String(v));
        }
        if let Some(v) = self.description {
            out.insert("description".into(), Value::String(v));
        }
        if let Some(v) = self.enum_values {
            out.insert("enum".into(), Value::Array(v));
        }
        if let Some(v) = self.constant {
            out.insert("const".into(), v);
        }
        if let Some(v) = self.items {
            out.insert("items".into(), v);
        }
        if let Some(v) = self.properties {
            out.insert("properties".into(), Value::Object(js_ordered(v)));
        }
        if let Some(v) = self.required {
            out.insert(
                "required".into(),
                Value::Array(v.into_iter().map(Value::String).collect()),
            );
        }
        if let Some(v) = self.additional_properties {
            out.insert("additionalProperties".into(), v);
        }
        if let Some(v) = self.any_of {
            out.insert("anyOf".into(), Value::Array(v));
        }
        if let Some(v) = self.one_of {
            out.insert("oneOf".into(), Value::Array(v));
        }
        if let Some(v) = self.default {
            out.insert("default".into(), v);
        }
        if let Some(v) = self.minimum {
            out.insert("minimum".into(), v);
        }
        if let Some(v) = self.maximum {
            out.insert("maximum".into(), v);
        }
        if let Some(v) = self.examples {
            out.insert("examples".into(), Value::Array(v));
        }
        if let Some(v) = self.deprecated {
            out.insert("deprecated".into(), Value::Bool(v));
        }
        Value::Object(out)
    }
}

/// A uniform property as JSON Schema at nesting `level` (a top-level input
/// property is level 1). `top` marks a parameter or top-level body property —
/// the only place the `limit` cap applies; `fallback_description` stands in
/// for an empty description; `title` names a union alternative.
fn property_schema(
    raw: &Value,
    level: usize,
    settings: &Settings,
    top: bool,
    fallback_description: &str,
    title: Option<&str>,
) -> Value {
    let prop = effective_property(raw);
    let prop: &Value = &prop;
    let mut f = Fields::default();
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        f.title = Some(t.to_string());
    }
    let own = unwrap_soft_breaks(text(prop.get("description")));
    let description = if own.is_empty() {
        fallback_description
    } else {
        own.as_str()
    };
    let set_description = |f: &mut Fields| {
        if !description.is_empty() {
            f.description = Some(description.to_string());
        }
    };

    match str_of(prop, "type").unwrap_or("") {
        "$$enum" => {
            let base = enum_base_type(prop);
            if let Some(t) = json_schema_type(Some(&Value::String(base.clone()))) {
                f.ty = Some(Value::String(t.to_string()));
            }
            set_description(&mut f);
            // A nullable enum gets a real `null` below; the converter's
            // `"null"` child (a null member) would read as the string.
            let nullable = meta_flag(prop.get("meta"), "nullable");
            f.enum_values = Some(
                array_of(prop, "properties")
                    .iter()
                    .map(|v| str_of(v, "name").unwrap_or(""))
                    .filter(|name| !(nullable && *name == "null"))
                    .map(|name| enum_value(name, &base))
                    .collect(),
            );
        }
        "$$array" => {
            f.ty = Some(Value::String("array".into()));
            set_description(&mut f);
            if let Some(item) = array_item(prop) {
                f.items = Some(property_schema(&item, level + 1, settings, false, "", None));
            }
        }
        ty @ ("$$union" | "$$xor") => {
            set_description(&mut f);
            let own_name = str_of(prop, "name");
            let alternatives: Vec<Value> = array_of(prop, "properties")
                .iter()
                .map(|alt| {
                    // An alternative named apart from its union keeps the
                    // name as its `title`.
                    let alt_name =
                        str_of(alt, "name").filter(|n| !n.is_empty() && Some(*n) != own_name);
                    property_schema(alt, level, settings, false, "", alt_name)
                })
                .collect();
            if ty == "$$union" {
                f.any_of = Some(alternatives);
            } else {
                f.one_of = Some(alternatives);
            }
        }
        _ => {
            let children = object_children(prop);
            let ty = json_schema_type(prop.get("type")).or(if children.is_empty() {
                None
            } else {
                Some("object")
            });
            if let Some(t) = ty {
                f.ty = Some(Value::String(t.into()));
            }
            set_description(&mut f);
            if !children.is_empty() && level <= settings.max_depth {
                let mut properties = Map::new();
                let mut required = Vec::new();
                for child in children {
                    let name = str_of(child, "name").unwrap_or("");
                    if name.is_empty() || properties.contains_key(name) {
                        continue;
                    }
                    properties.insert(
                        name.to_string(),
                        property_schema(child, level + 1, settings, false, "", None),
                    );
                    if is_required(child) {
                        required.push(name.to_string());
                    }
                }
                f.properties = Some(properties);
                if !required.is_empty() {
                    f.required = Some(required);
                }
            }
        }
    }

    let meta = prop.get("meta");
    if let Some(v) = meta_value(meta, "const") {
        f.constant = Some(v.clone());
    }
    if let Some(v) = meta_value(meta, "defaults") {
        f.default = Some(v.clone());
    }
    if let Some(v) = meta_value(meta, "minimum") {
        f.minimum = Some(v.clone());
    }
    if let Some(v) = meta_value(meta, "maximum") {
        f.maximum = Some(v.clone());
    }
    f.examples = examples_of(prop);
    if meta_flag(meta, "deprecated") {
        f.deprecated = Some(true);
    }
    if meta_flag(meta, "nullable") {
        make_nullable(&mut f);
    }
    if let Some(cap) = settings.limit_cap {
        if top && str_of(prop, "name") == Some("limit") && is_integer_type(f.ty.as_ref()) {
            cap_limit(&mut f, cap);
        }
    }

    f.emit()
}

fn examples_of(prop: &Value) -> Option<Vec<Value>> {
    let meta = prop.get("meta");
    if let Some(v) = find_meta(meta, "examples").and_then(|m| m.get("value")) {
        return Some(match v {
            Value::Array(items) => items.clone(),
            other => vec![other.clone()],
        });
    }
    if let Some(v) = find_meta(meta, "example").and_then(|m| m.get("value")) {
        return Some(vec![v.clone()]);
    }
    match prop.get("examples") {
        Some(Value::Array(items)) if !items.is_empty() => Some(items.clone()),
        Some(Value::String(s)) if !s.is_empty() => Some(vec![Value::String(s.clone())]),
        _ => None,
    }
}

fn make_nullable(f: &mut Fields) {
    let is_null = |v: &Value| v.get("type").and_then(|t| t.as_str()) == Some("null");
    let string_type = match &f.ty {
        Some(Value::String(ty)) => Some(ty.clone()),
        _ => None,
    };
    if let Some(ty) = string_type {
        if ty != "null" {
            f.ty = Some(Value::Array(vec![
                Value::String(ty),
                Value::String("null".into()),
            ]));
        }
    } else if let Some(any_of) = f.any_of.as_mut().filter(|a| !a.iter().any(is_null)) {
        any_of.push(serde_json::json!({ "type": "null" }));
    } else if let Some(one_of) = f.one_of.as_mut().filter(|a| !a.iter().any(is_null)) {
        one_of.push(serde_json::json!({ "type": "null" }));
    }
    if let Some(values) = f.enum_values.as_mut() {
        if !values.iter().any(|v| v.is_null()) {
            values.push(Value::Null);
        }
    }
}

fn is_integer_type(ty: Option<&Value>) -> bool {
    match ty {
        Some(Value::String(t)) => t == "integer",
        Some(Value::Array(items)) => items.first().and_then(|t| t.as_str()) == Some("integer"),
        _ => false,
    }
}

/// The cap binds unless the API's own `maximum` is already within it: the
/// maximum and a larger default come down to the cap, and the description
/// says so in place of the "at most N" clause it may carry.
fn cap_limit(f: &mut Fields, cap: u64) {
    if let Some(Value::Number(m)) = &f.maximum {
        if m.as_f64().is_some_and(|m| m <= cap as f64) {
            return;
        }
    }
    f.maximum = Some(Value::from(cap));
    if let Some(Value::Number(d)) = &f.default {
        if d.as_f64().is_some_and(|d| d > cap as f64) {
            f.default = Some(Value::from(cap));
        }
    }
    let stripped = strip_at_most(f.description.as_deref().unwrap_or(""));
    f.description = Some(with_sentence(
        Some(&stripped),
        &format!("Capped at {cap} by this server."),
    ));
}

/// Drops an "at most N" clause (any case) that ends a clause — `…, at most
/// 200.`, `… and at most 200`, `…; at most 200` or `… (at most 200).` — with
/// what joins it to the sentence: the comma or semicolon, a conjunction
/// (`and`, `or`, `but`), the parentheses. Nothing but punctuation left reads
/// as no description.
const CONJUNCTIONS: &[&str] = &["and", "or", "but"];

pub fn strip_at_most(description: &str) -> String {
    let mut chars: Vec<char> = description.chars().collect();
    let pattern: Vec<char> = "at most".chars().collect();
    let at = |chars: &[char], i: isize| -> Option<char> {
        if i < 0 {
            None
        } else {
            chars.get(i as usize).copied()
        }
    };
    let space = |c: Option<char>| matches!(c, Some(' ' | '\t'));
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    let mut i: isize = 0;
    while (i as usize) < chars.len() {
        let boundary = !at(&chars, i - 1).is_some_and(|c| c.is_ascii_alphanumeric());
        let matches = pattern
            .iter()
            .enumerate()
            .all(|(k, p)| at(&chars, i + k as isize).map(|c| c.to_ascii_lowercase()) == Some(*p));
        if boundary && matches {
            let mut end = i + pattern.len() as isize;
            let spaced = space(at(&chars, end));
            while space(at(&chars, end)) {
                end += 1;
            }
            let numbered = digit(at(&chars, end));
            while digit(at(&chars, end)) {
                end += 1;
            }
            let mut start = i;
            if at(&chars, start - 1) == Some('(') && at(&chars, end) == Some(')') {
                start -= 1;
                end += 1;
            }
            let closes = (end as usize) == chars.len()
                || matches!(at(&chars, end), Some('.' | ',' | ';' | ')'));
            if spaced && numbered && closes {
                while start > 0 && space(at(&chars, start - 1)) {
                    start -= 1;
                }
                // a conjunction joining the clause on: " and at most 200"
                let mut word = start;
                while word > 0 && at(&chars, word - 1).is_some_and(|c| c.is_ascii_alphabetic()) {
                    word -= 1;
                }
                let joined: String = chars[word as usize..start as usize].iter().collect();
                let detached = !at(&chars, word - 1).is_some_and(|c| c.is_ascii_alphanumeric());
                if word < start
                    && detached
                    && CONJUNCTIONS.contains(&joined.to_ascii_lowercase().as_str())
                {
                    start = word;
                    while start > 0 && space(at(&chars, start - 1)) {
                        start -= 1;
                    }
                }
                if start > 0 && matches!(at(&chars, start - 1), Some(',' | ';')) {
                    start -= 1;
                }
                chars.drain(start as usize..end as usize);
                i = start;
                continue;
            }
        }
        i += 1;
    }
    let out: String = chars.into_iter().collect();
    if out.chars().all(|c| " \t.,;:()".contains(c)) {
        String::new()
    } else {
        out
    }
}

fn with_sentence(description: Option<&str>, sentence: &str) -> String {
    let d = js_trim_end(description.unwrap_or(""));
    if d.is_empty() {
        return sentence.to_string();
    }
    if d.ends_with(['.', '!', '?']) {
        format!("{d} {sentence}")
    } else {
        format!("{d}. {sentence}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn operation(canonical: &str) -> Value {
        json!({
            "title": "Do a thing",
            "canonical": canonical,
            "description": "Does a thing.",
            "category": "rest",
            "type": "rest_get",
            "context": { "method": "get", "path": "/things", "group": ["things"] },
            "definitions": [],
            "examples": { "groups": [] }
        })
    }

    #[test]
    fn names_are_slugged_and_unique() {
        let long = "x".repeat(200);
        let refs = [
            operation("things/list"),
            operation("things list"),
            operation(&long),
            operation(&long),
        ];
        let names: Vec<String> = uniform_to_mcp_tools(&refs, None)
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(
            names,
            [
                "things_list".to_string(),
                "things_list_2".to_string(),
                "x".repeat(128),
                format!("{}_2", "x".repeat(126)),
            ]
        );
    }

    #[test]
    fn options_deserialize_from_the_typescript_names() {
        let options: McpToolsOptions =
            serde_json::from_value(json!({ "flatten": false, "limitCap": 50, "maxDepth": 3 }))
                .unwrap();
        assert_eq!(options.flatten, Some(false));
        assert_eq!(options.limit_cap, Some(50));
        assert_eq!(options.max_depth, Some(3));
        assert!(options.include.is_none());
    }

    #[test]
    fn at_most_clauses_the_cap_overrides_are_dropped() {
        assert_eq!(strip_at_most("Page size, at most 200."), "Page size.");
        assert_eq!(strip_at_most("Page size (at most 200)."), "Page size.");
        assert_eq!(strip_at_most("Page size; At Most 200"), "Page size");
        assert_eq!(strip_at_most("At most 200."), "");
        assert_eq!(
            strip_at_most("10 by default and at most 200"),
            "10 by default"
        );
        assert_eq!(
            strip_at_most("Defaults to 10, and at most 200."),
            "Defaults to 10."
        );
        assert_eq!(
            strip_at_most("Up to 10 or at most 20; more is rejected."),
            "Up to 10; more is rejected."
        );
        assert_eq!(strip_at_most("Brand at most 2."), "Brand.");
        assert_eq!(
            strip_at_most("At most 200 items are returned."),
            "At most 200 items are returned."
        );
        assert_eq!(strip_at_most("Whatnot most 2."), "Whatnot most 2.");
    }

    #[test]
    fn abbreviations_never_end_the_first_sentence() {
        for (line, sentence) in [
            (
                "Returns pets, e.g. the ones on sale. More.",
                "Returns pets, e.g. the ones on sale.",
            ),
            (
                "Uses carrier No. 5 by default. More.",
                "Uses carrier No. 5 by default.",
            ),
            ("Signed by J. Doe. More.", "Signed by J. Doe."),
            ("Returns 3 items. 2 are new.", "Returns 3 items."),
            (
                "Returns the item.It has no space.",
                "Returns the item.It has no space.",
            ),
            ("Is it there? \"Yes\" it is.", "Is it there?"),
        ] {
            assert_eq!(first_sentence(line), sentence);
        }
    }

    #[test]
    fn inputs_roundtrip_in_order() {
        let raw = r#"{"b_query":{"in":"query","name":"b"},"a_body":{"in":"body","name":"a"}}"#;
        let inputs: McpToolInputs = serde_json::from_str(raw).unwrap();
        assert_eq!(
            inputs.get("a_body").unwrap().location,
            McpInputLocation::Body
        );
        assert_eq!(serde_json::to_string(&inputs).unwrap(), raw);
    }

    #[test]
    fn descriptions_are_cut_to_the_budget() {
        let mut r = operation("doThing");
        r["description"] = json!("A long first sentence that goes on and on");
        let options = McpToolsOptions {
            description_chars: Some(12),
            ..Default::default()
        };
        let tools = uniform_to_mcp_tools(&[r], Some(options));
        assert_eq!(tools[0].description, "A long firs… — GET /things");
    }
}
