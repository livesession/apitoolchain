//! Shared reading of REST operations for the agent emitters ([`crate::mcp`],
//! [`crate::skill`]) — the twin of `packages/apitoolchain-uniform/src/operations.ts`.
//! Operates on raw JSON values like [`crate::converters`]: the emitters read
//! fields loosely, the way the TypeScript reads a parsed `Reference[]`.

use std::borrow::Cow;

use serde_json::{json, Map, Value};

use crate::jsrt::{is_js_whitespace, js_object_keys, truthy};

const REST_OPERATION_TYPES: &[&str] = &[
    "rest_get",
    "rest_post",
    "rest_put",
    "rest_patch",
    "rest_delete",
    "rest_options",
    "rest_head",
    "rest_trace",
];

const OF_TYPES: &[&str] = &["$$array", "$$union", "$$xor"];

const JSON_SCHEMA_TYPES: &[&str] = &[
    "string", "integer", "number", "boolean", "object", "array", "null",
];

pub(crate) const EMPTY: &[Value] = &[];

pub(crate) fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|v| v.as_str())
}

/// `value[key] || []` for an array-valued key.
pub(crate) fn array_of<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(EMPTY)
}

/// A present, non-null key (`if (x.key)` for object-valued keys).
pub(crate) fn object_at<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|v| truthy(Some(v)))
}

/// A string as text; anything else (a ReactNode never crosses here) is `""`.
pub(crate) fn text(value: Option<&Value>) -> &str {
    value.and_then(|v| v.as_str()).unwrap_or("")
}

/// JS `String.prototype.trim`.
pub(crate) fn js_trim(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// JS `String.prototype.trimEnd`.
pub(crate) fn js_trim_end(value: &str) -> &str {
    value.trim_end_matches(is_js_whitespace)
}

/// A REST operation: category `rest` and a `rest_<method>` type.
pub(crate) fn is_rest_operation(reference: &Value) -> bool {
    str_of(reference, "category") == Some("rest")
        && str_of(reference, "type").is_some_and(|t| REST_OPERATION_TYPES.contains(&t))
}

fn context(reference: &Value) -> Option<&Value> {
    reference.get("context").filter(|c| c.is_object())
}

/// The HTTP method, upper-case (`context.method`, else the type).
pub(crate) fn operation_method(reference: &Value) -> String {
    match context(reference).and_then(|c| str_of(c, "method")) {
        Some(method) if !method.is_empty() => method.to_uppercase(),
        _ => {
            let ty = str_of(reference, "type").unwrap_or("");
            ty.strip_prefix("rest_").unwrap_or(ty).to_uppercase()
        }
    }
}

pub(crate) fn operation_path(reference: &Value) -> String {
    context(reference)
        .and_then(|c| str_of(c, "path"))
        .unwrap_or("")
        .to_string()
}

pub(crate) fn operation_full_path(reference: &Value) -> Option<String> {
    context(reference)
        .and_then(|c| str_of(c, "fullPath"))
        .filter(|p| !p.is_empty())
        .map(String::from)
}

/// `context.group` without its empty entries.
pub(crate) fn operation_group(reference: &Value) -> Vec<String> {
    context(reference)
        .map(|c| array_of(c, "group"))
        .unwrap_or(EMPTY)
        .iter()
        .filter_map(|g| g.as_str())
        .filter(|g| !g.is_empty())
        .map(String::from)
        .collect()
}

/// `context.group[0]` when it is a string, else `""`.
pub(crate) fn first_group(reference: &Value) -> String {
    context(reference)
        .map(|c| array_of(c, "group"))
        .and_then(|g| g.first())
        .and_then(|g| g.as_str())
        .unwrap_or("")
        .to_string()
}

pub(crate) fn find_meta<'a>(meta: Option<&'a Value>, name: &str) -> Option<&'a Value> {
    meta.and_then(|m| m.as_array())?
        .iter()
        .find(|m| str_of(m, "name") == Some(name))
}

/// A meta entry's `value` when the key is present (an explicit `null` included).
pub(crate) fn meta_value<'a>(meta: Option<&'a Value>, name: &str) -> Option<&'a Value> {
    find_meta(meta, name)?.get("value")
}

/// A meta flag: `"true"` (what the converters write) or `true`.
pub(crate) fn meta_flag(meta: Option<&Value>, name: &str) -> bool {
    match meta_value(meta, name) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    }
}

/// Required per its meta; an `anyOf` property without meta is required when
/// one of its alternatives is (the converter puts the flag on those).
pub(crate) fn is_required(prop: &Value) -> bool {
    if meta_flag(prop.get("meta"), "required") {
        return true;
    }
    if str_of(prop, "type") == Some("$$union") && prop.get("meta").is_none() {
        return array_of(prop, "properties")
            .iter()
            .any(|alt| meta_flag(alt.get("meta"), "required"));
    }
    false
}

/// The properties of every definition of one REST type, in document order.
pub(crate) fn definition_properties<'a>(reference: &'a Value, ty: &str) -> Vec<&'a Value> {
    array_of(reference, "definitions")
        .iter()
        .filter(|d| str_of(d, "type") == Some(ty))
        .flat_map(|d| array_of(d, "properties").iter())
        .collect()
}

pub(crate) fn is_json_content_type(content_type: &str) -> bool {
    let base = content_type.split(';').next().unwrap_or("");
    let base = js_trim(base).to_lowercase();
    base == "application/json" || base.ends_with("+json")
}

pub(crate) fn variant_content_type(variant: &Value) -> String {
    meta_value(variant.get("meta"), "contentType")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub(crate) struct RequestBody<'a> {
    pub content_type: String,
    pub description: String,
    pub properties: Vec<&'a Value>,
    pub root_property: Option<&'a Value>,
    pub required: bool,
    /// What the body allows beyond its properties, when it says: `true` or a
    /// JSON Schema, from an `additionalProperties` meta on the variant (else
    /// on the definition).
    pub additional_properties: Option<Value>,
}

/// An `additionalProperties` meta that opens an object: `true` or a schema object.
fn openness(meta: Option<&Value>) -> Option<Value> {
    match meta_value(meta, "additionalProperties") {
        Some(v @ (Value::Bool(true) | Value::Object(_))) => Some(v.clone()),
        _ => None,
    }
}

/// The request body: its JSON variant, else (`json_only` false) its first one.
pub(crate) fn request_body(reference: &Value, json_only: bool) -> Option<RequestBody<'_>> {
    let def = array_of(reference, "definitions")
        .iter()
        .find(|d| str_of(d, "type") == Some("$rest.request.body"))?;
    let required = meta_flag(def.get("meta"), "required");
    let variants = array_of(def, "variants");
    if !variants.is_empty() {
        let json = variants.iter().find(|v| {
            let ct = variant_content_type(v);
            let ct = if ct.is_empty() {
                str_of(v, "title").unwrap_or("").to_string()
            } else {
                ct
            };
            is_json_content_type(&ct)
        });
        let variant = match json {
            Some(v) => v,
            None if json_only => return None,
            None => &variants[0],
        };
        let content_type = variant_content_type(variant);
        return Some(RequestBody {
            content_type: if content_type.is_empty() {
                str_of(variant, "title").unwrap_or("").to_string()
            } else {
                content_type
            },
            description: text(variant.get("description")).to_string(),
            properties: array_of(variant, "properties").iter().collect(),
            root_property: object_at(variant, "rootProperty"),
            required,
            additional_properties: openness(variant.get("meta"))
                .or_else(|| openness(def.get("meta"))),
        });
    }
    let content_type = meta_value(def.get("meta"), "contentType")
        .and_then(|v| v.as_str())
        .unwrap_or("application/json");
    Some(RequestBody {
        content_type: content_type.to_string(),
        description: text(def.get("description")).to_string(),
        properties: array_of(def, "properties").iter().collect(),
        root_property: object_at(def, "rootProperty"),
        required,
        additional_properties: openness(def.get("meta")),
    })
}

/// The response variants (one per status and content type).
pub(crate) fn response_variants(reference: &Value) -> Vec<&Value> {
    array_of(reference, "definitions")
        .iter()
        .filter(|d| str_of(d, "type") == Some("return"))
        .flat_map(|d| array_of(d, "variants").iter())
        .collect()
}

/// What a plain-typed property with an `ofProperty` stands for: an `allOf`
/// wrapper around an array or a union reads as that array or union, with the
/// wrapper's name, its description when it has one, and its meta first.
pub(crate) fn effective_property(prop: &Value) -> Cow<'_, Value> {
    let mut current: Cow<'_, Value> = Cow::Borrowed(prop);
    for _ in 0..16 {
        let ty = str_of(&current, "type").unwrap_or("");
        let next = {
            let Some(of) = object_at(&current, "ofProperty") else {
                return current;
            };
            let of_ty = str_of(of, "type").unwrap_or("");
            if ty == "$$enum" || ty == "$$array" || !OF_TYPES.contains(&of_ty) {
                return current;
            }
            let mut next = of.clone();
            if let Some(obj) = next.as_object_mut() {
                match current.get("name") {
                    Some(name) => {
                        obj.insert("name".into(), name.clone());
                    }
                    None => {
                        obj.remove("name");
                    }
                }
                let own = text(current.get("description"));
                let description = if own.is_empty() {
                    text(of.get("description"))
                } else {
                    own
                };
                obj.insert("description".into(), Value::String(description.to_string()));
                let mut meta: Vec<Value> = array_of(&current, "meta").to_vec();
                meta.extend(array_of(of, "meta").iter().cloned());
                obj.insert("meta".into(), Value::Array(meta));
            }
            next
        };
        current = Cow::Owned(next);
    }
    current
}

/// An array's item: its `ofProperty`, else a single unnamed child, else an
/// object of its children (the shape a request/response root array takes).
pub(crate) fn array_item(prop: &Value) -> Option<Cow<'_, Value>> {
    if let Some(of) = object_at(prop, "ofProperty") {
        return Some(Cow::Borrowed(of));
    }
    let children = array_of(prop, "properties");
    if children.len() == 1 && !truthy(children[0].get("name")) {
        return Some(Cow::Borrowed(&children[0]));
    }
    if !children.is_empty() {
        return Some(Cow::Owned(json!({
            "name": "",
            "type": "object",
            "description": "",
            "properties": children,
        })));
    }
    None
}

pub(crate) fn json_schema_type(ty: Option<&Value>) -> Option<&'static str> {
    let ty = ty?.as_str()?;
    JSON_SCHEMA_TYPES.iter().copied().find(|t| *t == ty)
}

/// The base type of a `$$enum`: its `ofProperty` type, else its `enum-type` meta.
pub(crate) fn enum_base_type(prop: &Value) -> String {
    if let Some(of) = prop
        .get("ofProperty")
        .and_then(|o| str_of(o, "type"))
        .filter(|t| !t.is_empty())
    {
        return of.to_string();
    }
    meta_value(prop.get("meta"), "enum-type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// `^-?\d+(\.\d+)?$`
fn is_numeric(name: &str) -> bool {
    let digits = name.strip_prefix('-').unwrap_or(name);
    let (int, frac) = match digits.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (digits, None),
    };
    !int.is_empty()
        && int.bytes().all(|b| b.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

/// A JS number as JSON: integral values within ±2^53 print without a fraction.
fn js_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// An enum value back from the converter's string child names.
pub(crate) fn enum_value(name: &str, base: &str) -> Value {
    if (base == "integer" || base == "number") && is_numeric(name) {
        if let Ok(n) = name.parse::<f64>() {
            return js_number(n);
        }
    }
    if base == "boolean" && (name == "true" || name == "false") {
        return Value::Bool(name == "true");
    }
    if !base.is_empty() && base != "string" && name == "null" {
        return Value::Null;
    }
    Value::String(name.to_string())
}

/// An object's children; none for the `$$` kinds.
pub(crate) fn object_children(prop: &Value) -> &[Value] {
    match str_of(prop, "type") {
        Some("$$enum" | "$$array" | "$$union" | "$$xor") => EMPTY,
        _ => array_of(prop, "properties"),
    }
}

/// A map in JS object-key order (integer-like keys first, ascending) — what
/// `JSON.stringify` prints for an object the TypeScript built key by key.
pub(crate) fn js_ordered(map: Map<String, Value>) -> Map<String, Value> {
    let keys: Vec<String> = js_object_keys(&map).into_iter().cloned().collect();
    let mut map = map;
    let mut out = Map::with_capacity(keys.len());
    for key in keys {
        if let Some(value) = map.remove(&key) {
            out.insert(key, value);
        }
    }
    out
}

/// A code fence opening: up to three spaces, then three or more backticks or
/// tildes — `(the fence character, its run length)`.
fn fence_of(line: &str) -> Option<(u8, usize)> {
    let bytes = line.as_bytes();
    let indent = bytes.iter().take(3).take_while(|b| **b == b' ').count();
    let ch = *bytes.get(indent)?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let length = bytes[indent..].iter().take_while(|b| **b == ch).count();
    (length >= 3).then_some((ch, length))
}

/// Whether a line closes a fence: the same character, at least as many, alone.
fn closes_fence(line: &str, (ch, length): (u8, usize)) -> bool {
    let t = js_trim(line);
    t.chars().count() >= length && t.chars().all(|c| c == ch as char)
}

/// A setext heading underline (`===`, `---`) or a thematic break (`***`,
/// `___`, `- - -`).
fn is_rule_line(line: &str) -> bool {
    let t = js_trim(line);
    if t.is_empty() {
        return false;
    }
    if t.chars().all(|c| c == '=') || t.chars().all(|c| c == '-') {
        return true;
    }
    ['-', '*', '_'].iter().any(|mark| {
        t.chars().all(|c| c == *mark || c == ' ' || c == '\t')
            && t.chars().filter(|c| c == mark).count() >= 3
    })
}

/// `^[-*+][ \t]+\S` — a bullet item with content.
fn is_bullet_item(line: &str) -> bool {
    let mut chars = line.chars();
    if !matches!(chars.next(), Some('-' | '*' | '+')) {
        return false;
    }
    let rest = chars.as_str();
    let content = rest.trim_start_matches([' ', '\t']);
    content.len() < rest.len() && content.chars().next().is_some_and(|c| !c.is_whitespace())
}

/// `^([0-9]+)[.)][ \t]+\S` — an ordered item with content: its digits.
fn ordered_item(line: &str) -> Option<&str> {
    let digits = line.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || !matches!(line.as_bytes().get(digits), Some(b'.' | b')')) {
        return None;
    }
    let rest = &line[digits + 1..];
    let content = rest.trim_start_matches([' ', '\t']);
    (content.len() < rest.len() && content.chars().next().is_some_and(|c| !c.is_whitespace()))
        .then(|| &line[..digits])
}

/// Whether a line opens a markdown block of its own after a non-blank line:
/// indented, a heading, a quote, a table row, a code fence, a rule line or a
/// list item. As in CommonMark, an ordered item interrupts a paragraph only
/// when it is numbered 1 — any number continues a list already open.
fn starts_block(line: &str, in_ordered_list: bool) -> bool {
    if line.starts_with([' ', '\t', '#', '>', '|'])
        || fence_of(line).is_some()
        || is_rule_line(line)
        || is_bullet_item(line)
    {
        return true;
    }
    ordered_item(line).is_some_and(|n| in_ordered_list || n.trim_start_matches('0') == "1")
}

/// Whether nothing may be joined onto a line: a heading, a table row, a code
/// fence or a rule line.
fn ends_block(line: &str) -> bool {
    line.starts_with(['#', '|']) || fence_of(line).is_some() || is_rule_line(line)
}

/// Joins hard-wrapped lines: a single newline becomes a space, unless a line
/// around it is blank, the next line opens a block of its own or the previous
/// one ends one. Paragraph breaks stay, and fenced code is kept line for line.
pub(crate) fn unwrap_soft_breaks(value: &str) -> String {
    let lines: Vec<&str> = value.split('\n').collect();
    let mut out = lines[0].to_string();
    let mut fence = fence_of(lines[0]);
    // Whether the last line that started a block of its own (not indented,
    // not blank) is an ordered list item: the list is open.
    let mut in_ordered_list = ordered_item(lines[0]).is_some();
    for i in 1..lines.len() {
        let line = lines[i];
        if let Some(open) = fence {
            out.push('\n');
            out.push_str(line);
            if closes_fence(line, open) {
                fence = None;
            }
            continue;
        }
        fence = fence_of(line);
        let previous = lines[i - 1];
        if fence.is_some()
            || js_trim(previous).is_empty()
            || js_trim(line).is_empty()
            || starts_block(line, in_ordered_list)
            || ends_block(previous)
        {
            out.push('\n');
            out.push_str(line);
            if !js_trim(line).is_empty() && !line.starts_with([' ', '\t']) {
                in_ordered_list = ordered_item(line).is_some();
            }
        } else {
            let kept = js_trim_end(&out).len();
            out.truncate(kept);
            out.push(' ');
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_breaks_join_but_blocks_stay() {
        assert_eq!(
            unwrap_soft_breaks("Returns the things that match\nthe filters. More\n\nNext."),
            "Returns the things that match the filters. More\n\nNext."
        );
        assert_eq!(
            unwrap_soft_breaks("a\n  indented\n1. item\n# heading\n- dash\n10) ten"),
            "a\n  indented\n1. item\n# heading\n- dash 10) ten"
        );
        // `1)` may interrupt a paragraph, `10)` may not
        assert_eq!(unwrap_soft_breaks("a\n1) one"), "a\n1) one");
        assert_eq!(unwrap_soft_breaks("a\n1.5 kg"), "a 1.5 kg");
    }

    #[test]
    fn fences_headings_and_rules_keep_their_lines() {
        assert_eq!(
            unwrap_soft_breaks("Sample\n```json\n{\n  \"a\": 1\n}\n```\nAfter\nthat."),
            "Sample\n```json\n{\n  \"a\": 1\n}\n```\nAfter that."
        );
        assert_eq!(
            unwrap_soft_breaks("Title\n=====\nText\nmore"),
            "Title\n=====\nText more"
        );
        assert_eq!(unwrap_soft_breaks("Title\n---\nText"), "Title\n---\nText");
        assert_eq!(
            unwrap_soft_breaks("# Head\nnext\nline"),
            "# Head\nnext line"
        );
        assert_eq!(unwrap_soft_breaks("a\n***\nb"), "a\n***\nb");
        assert_eq!(
            unwrap_soft_breaks("Up to 10 and at most\n5000. Larger is rejected."),
            "Up to 10 and at most 5000. Larger is rejected."
        );
        assert_eq!(
            unwrap_soft_breaks("Steps:\n1. one\n2. two\nlazy\n3. three"),
            "Steps:\n1. one\n2. two lazy\n3. three"
        );
        assert_eq!(unwrap_soft_breaks("5. five\n6. six"), "5. five\n6. six");
        assert_eq!(
            unwrap_soft_breaks("Text\n\n7. seven\n8. eight"),
            "Text\n\n7. seven\n8. eight"
        );
        assert_eq!(
            unwrap_soft_breaks("1. one\n\nText\n2. two"),
            "1. one\n\nText 2. two"
        );
        assert_eq!(
            unwrap_soft_breaks("~~~~\nx\n~~~\ny\n~~~~\nz\nw"),
            "~~~~\nx\n~~~\ny\n~~~~\nz w"
        );
    }

    #[test]
    fn enum_values_come_back_typed() {
        assert_eq!(enum_value("1", "integer"), json!(1));
        assert_eq!(enum_value("1.50", "number"), json!(1.5));
        assert_eq!(enum_value("1.0", "number"), json!(1));
        assert_eq!(enum_value("1e3", "number"), json!("1e3"));
        assert_eq!(enum_value("true", "boolean"), json!(true));
        assert_eq!(enum_value("null", "integer"), Value::Null);
        assert_eq!(enum_value("null", "string"), json!("null"));
    }

    #[test]
    fn js_trim_matches_string_prototype_trim() {
        assert_eq!(js_trim("\u{FEFF} a \u{3000}"), "a");
        // U+0085 is Unicode whitespace but not JS whitespace.
        assert_eq!(js_trim("\u{0085}a"), "\u{0085}a");
    }

    #[test]
    fn js_ordered_puts_integer_keys_first() {
        let mut map = Map::new();
        map.insert("b".into(), json!(1));
        map.insert("2".into(), json!(2));
        map.insert("1".into(), json!(3));
        let keys: Vec<String> = js_ordered(map).keys().cloned().collect();
        assert_eq!(keys, ["1", "2", "b"]);
    }
}
