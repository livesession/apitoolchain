//! Port of `uniformPluginXDocsSidebar` (packages/openapi/src/impl-js/
//! xdocs/pluginSidebar.ts) — the x-docs sidebar reordering plugin — minus the
//! `x-docs` EXAMPLES builders (`meta.examples` / `componentMeta.example`):
//! example mutations only affect `ref.examples`, which the fused pages/
//! sidebar/frontmatter outputs never read, and endpoint examples are a JS
//! post-pass anyway. Refs are handled as JSON values (the JS plugin mutates
//! plain objects).

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::doc::DocCtx;
use apitoolchain_uniform::jsrt::truthy;

const COMPONENT_SCHEMA_PREFIX: &str = "#/components/schemas/";

/// `getXDocs(doc)?.route` (string) — the fileRouting side-channel.
pub fn get_xdocs_route(doc: &Value) -> Option<String> {
    doc.get("x-docs")?
        .get("route")?
        .as_str()
        .map(|s| s.to_string())
}

/// Apply the plugin to `refs` in place: the per-ref x-docs meta pass, then
/// (when `x-docs.sidebar` exists) the sidebar-driven ref-list rebuild —
/// otherwise the generated navigation minus the `x-docs.hidden` refs.
pub fn apply_xdocs_sidebar(ctx: &DocCtx, doc: &Value, refs: &mut Vec<Value>) {
    // ---- per-ref pass (pluginXDocsSidebarInner) -------------------------
    // ref index maps mirror refByOperationId / refByComponentSchema
    // (last-write-wins on duplicate keys, like JS object assignment).
    let mut by_operation_id: std::collections::HashMap<String, usize> = Default::default();
    let mut by_component_schema: std::collections::HashMap<String, usize> = Default::default();
    // `x-docs.hidden` refs (operations and component schemas alike) — dropped
    // from the GENERATED navigation below. An `x-docs.sidebar` that lists one
    // explicitly still shows it: the sidebar is a complete enumeration and
    // wins over the per-schema flag.
    let mut hidden: Vec<usize> = Vec::new();

    for (i, r) in refs.iter_mut().enumerate() {
        let (component_schema, method, path) = {
            let c = r.get("context");
            (
                c.and_then(|c| c.get("componentSchema"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                c.and_then(|c| c.get("method"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                c.and_then(|c| c.get("path"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            )
        };

        if let Some(cs) = &component_schema {
            by_component_schema.insert(cs.clone(), i);
            // The schema's own `x-docs` — honoured here, outside sidebar mode,
            // exactly like an operation's (`name` → title, `group` → sidebar
            // group, `hidden`). The sidebar pass re-reads `name` for the
            // object pages it lists.
            if let Some(meta) = component_xdocs(ctx, doc, cs) {
                if apply_docs_meta(r, meta) {
                    hidden.push(i);
                }
            }
        }

        // selector("[method] [path]") — the operation node; absent for
        // component-schema refs.
        let (Some(method), Some(path)) = (method, path) else {
            continue;
        };
        let Some(op) = operation_node(ctx, doc, &path, &method) else {
            continue;
        };

        if let Some(operation_id) = op.get("operationId").and_then(|v| v.as_str()) {
            if !operation_id.is_empty() {
                by_operation_id.insert(operation_id.to_string(), i);
            }
        }
        // `(httpMethod.toUpperCase() + " " + path).trim()`
        let method_id = format!("{} {}", method.to_uppercase(), path)
            .trim()
            .to_string();
        if !method_id.is_empty() {
            by_operation_id.insert(method_id, i);
        }

        let Some(meta) = op.get("x-docs") else {
            continue;
        };

        if apply_docs_meta(r, meta) {
            hidden.push(i);
        }
        if !truthy(r.get("description")) {
            let summary = op
                .get("summary")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            r["description"] = Value::String(summary);
        }
        if let Some(returns) = meta.get("returns").filter(|v| truthy(Some(v))) {
            let defs = r.get_mut("definitions").and_then(|d| d.as_array_mut());
            match defs {
                Some(defs) if !defs.is_empty() => {
                    let last = defs.len() - 1;
                    let title = defs[last].get("title").cloned().unwrap_or(Value::Null);
                    defs[last] = json!({
                        "title": title,
                        "description": returns,
                        "properties": [],
                    });
                }
                _ => {
                    r["definitions"] = json!([{
                        "title": "Response",
                        "description": returns,
                        "properties": [],
                    }]);
                }
            }
        }
        // meta.examples: NOT ported (see module header).
    }

    // ---- defer pass: the sidebar-driven rebuild -------------------------
    let xdocs = doc.get("x-docs");
    let sidebar = xdocs
        .and_then(|x| x.get("sidebar"))
        .and_then(|s| s.as_array());
    let Some(sidebar) = sidebar else {
        // No sidebar: the generated navigation, minus the hidden refs.
        if !hidden.is_empty() {
            let mut i = 0;
            refs.retain(|_| {
                let keep = !hidden.contains(&i);
                i += 1;
                keep
            });
        }
        relink_schema_links(refs);
        return;
    };
    let xdocs = xdocs.unwrap_or(&Value::Null);

    let inherit = xdocs.get("sidebarPathStrategy").and_then(|v| v.as_str()) == Some("inherit");

    let mut output: Vec<Value> = Vec::new();
    for group in sidebar {
        // JS builds navigationMap and warns/continues when a group is falsy —
        // structurally: skip non-objects, require array pages.
        if !group.is_object() {
            continue;
        }
        let Some(pages) = group.get("pages").and_then(|p| p.as_array()) else {
            continue;
        };
        let group_name = group
            .get("group")
            .and_then(|g| g.as_str())
            .unwrap_or("")
            .to_string();
        process_group_pages(
            ctx,
            doc,
            refs,
            &by_operation_id,
            &by_component_schema,
            pages,
            &[group_name],
            None,
            inherit,
            &mut output,
        );
    }

    // `references.length = 0; references.push(...output)`
    *refs = output;
    relink_schema_links(refs);
}

/// Nested schema links, once the reference list is final. A property's
/// `symbolDef` whose `id` is a component schema (`#/components/schemas/<Name>`)
/// takes the `canonical` of that schema's surviving reference — so a sidebar
/// that moves an object page under another path is followed — and loses its
/// `canonical` when the schema has no page any more (`x-docs.hidden`, or not
/// listed by an `x-docs.sidebar`): a link must never land on a page that does
/// not exist. The `id` stays either way.
fn relink_schema_links(refs: &mut [Value]) {
    let mut canonical_by_schema: HashMap<String, String> = HashMap::new();
    for r in refs.iter() {
        let name = r
            .get("context")
            .and_then(|c| c.get("componentSchema"))
            .and_then(|v| v.as_str());
        let canonical = r
            .get("canonical")
            .and_then(|v| v.as_str())
            .filter(|c| !c.is_empty());
        if let (Some(name), Some(canonical)) = (name, canonical) {
            canonical_by_schema
                .entry(name.to_string())
                .or_insert_with(|| canonical.to_string());
        }
    }

    for r in refs.iter_mut() {
        let Some(definitions) = r.get_mut("definitions").and_then(|d| d.as_array_mut()) else {
            continue;
        };
        for definition in definitions {
            relink_rows(definition.get_mut("properties"), &canonical_by_schema);
            relink_property(definition.get_mut("rootProperty"), &canonical_by_schema);
            if let Some(variants) = definition
                .get_mut("variants")
                .and_then(|v| v.as_array_mut())
            {
                for variant in variants {
                    relink_rows(variant.get_mut("properties"), &canonical_by_schema);
                    relink_property(variant.get_mut("rootProperty"), &canonical_by_schema);
                }
            }
        }
    }
}

fn relink_rows(rows: Option<&mut Value>, canonical_by_schema: &HashMap<String, String>) {
    if let Some(rows) = rows.and_then(|r| r.as_array_mut()) {
        for row in rows {
            relink_property(Some(row), canonical_by_schema);
        }
    }
}

fn relink_property(property: Option<&mut Value>, canonical_by_schema: &HashMap<String, String>) {
    let Some(property) = property.filter(|p| p.is_object()) else {
        return;
    };
    if let Some(symbol_def) = property
        .get_mut("symbolDef")
        .and_then(|s| s.as_object_mut())
    {
        let schema = symbol_def
            .get("id")
            .and_then(|i| i.as_str())
            .and_then(|i| i.strip_prefix(COMPONENT_SCHEMA_PREFIX))
            .map(|s| s.to_string());
        if let Some(schema) = schema {
            match canonical_by_schema.get(&schema) {
                Some(target) => {
                    symbol_def.insert("canonical".to_string(), Value::String(target.clone()));
                }
                None => {
                    symbol_def.remove("canonical");
                }
            }
        }
    }
    relink_rows(property.get_mut("properties"), canonical_by_schema);
    relink_property(property.get_mut("ofProperty"), canonical_by_schema);
}

/// The common `x-docs` keys of an operation or a component schema: `name` →
/// the reference title, `group` → its sidebar group. Returns whether the ref
/// is `hidden`.
fn apply_docs_meta(r: &mut Value, meta: &Value) -> bool {
    if let Some(name) = meta.get("name").filter(|n| truthy(Some(n))) {
        r["title"] = name.clone();
    }
    if let Some(group) = meta.get("group").filter(|g| truthy(Some(g))) {
        // JS: `if (ref.context) ref.context.group = [meta.group]`
        if r.get("context").map(|c| truthy(Some(c))).unwrap_or(false) {
            r["context"]["group"] = json!([group]);
        }
    }
    truthy(meta.get("hidden"))
}

/// A component schema's `x-docs`: on the schema itself, else on the first
/// `allOf` member that carries one (a composed model may document itself on
/// a member).
fn component_xdocs<'a>(ctx: &DocCtx<'a>, doc: &'a Value, name: &str) -> Option<&'a Value> {
    let component = doc.get("components")?.get("schemas")?.get(name)?;
    let component = ctx.resolve(component);
    if let Some(meta) = component.get("x-docs").filter(|m| truthy(Some(m))) {
        return Some(meta);
    }
    for item in component.get("allOf")?.as_array()? {
        if let Some(meta) = ctx.resolve(item).get("x-docs").filter(|m| truthy(Some(m))) {
            return Some(meta);
        }
    }
    None
}

/// Resolve `doc.paths[path][method]` through possible `$ref` path items.
fn operation_node<'a>(
    ctx: &DocCtx<'a>,
    doc: &'a Value,
    path: &str,
    method: &str,
) -> Option<&'a Value> {
    let path_item = doc.get("paths")?.get(path)?;
    let path_item = ctx.resolve(path_item);
    let op = path_item.get(method)?;
    let op = ctx.resolve(op);
    if op.is_object() {
        Some(op)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn process_group_pages(
    ctx: &DocCtx,
    doc: &Value,
    refs: &[Value],
    by_operation_id: &std::collections::HashMap<String, usize>,
    by_component_schema: &std::collections::HashMap<String, usize>,
    pages: &[Value],
    group_path: &[String],
    parent_path: Option<&str>,
    inherit: bool,
    output: &mut Vec<Value>,
) {
    for page in pages {
        let nested = page.get("pages").and_then(|p| p.as_array());
        if let Some(nested_pages) = nested {
            // nested group
            let mut gp = group_path.to_vec();
            gp.push(
                page.get("group")
                    .and_then(|g| g.as_str())
                    .unwrap_or("")
                    .to_string(),
            );
            let pp = page.get("path").and_then(|p| p.as_str());
            process_group_pages(
                ctx,
                doc,
                refs,
                by_operation_id,
                by_component_schema,
                nested_pages,
                &gp,
                pp,
                inherit,
                output,
            );
        } else if page.get("type").is_some() && page.get("key").is_some() {
            process_page(
                ctx,
                doc,
                refs,
                by_operation_id,
                by_component_schema,
                page,
                group_path,
                parent_path,
                inherit,
                output,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_page(
    ctx: &DocCtx,
    doc: &Value,
    refs: &[Value],
    by_operation_id: &std::collections::HashMap<String, usize>,
    by_component_schema: &std::collections::HashMap<String, usize>,
    page: &Value,
    group_path: &[String],
    parent_path: Option<&str>,
    inherit: bool,
    output: &mut Vec<Value>,
) {
    let page_type = page.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let key = page.get("key").and_then(|k| k.as_str()).unwrap_or("");

    let mut uniform_ref: Value = match page_type {
        "endpoint" => {
            let Some(&idx) = by_operation_id.get(key) else {
                return; // JS warns + returns
            };
            refs[idx].clone()
        }
        "object" => {
            let Some(&idx) = by_component_schema.get(key) else {
                return;
            };
            // selector("[component]") = doc.components.schemas[key]
            if doc
                .get("components")
                .and_then(|c| c.get("schemas"))
                .and_then(|s| s.get(key))
                .is_none()
            {
                return;
            }

            // The same lookup as the generated navigation (on the schema
            // itself, else on the first `allOf` member that carries one), so
            // a listed page reads its `name` from the same place in both
            // modes. A page without any `x-docs` keeps its defaults — the
            // sidebar lists it, so it exists.
            let component_meta = component_xdocs(ctx, doc, key);

            let mut r = refs[idx].clone();
            if let Some(meta) = component_meta {
                // `componentRef.title = componentMeta.name || componentRef.title`
                if let Some(name) = meta.get("name").filter(|n| truthy(Some(n))) {
                    r["title"] = name.clone();
                }
                // componentMeta.example: NOT ported (see module header).
            }
            r
        }
        _ => return, // JS warns on unknown page type
    };

    // canonical rewriting
    let page_path = page.get("path").and_then(|p| p.as_str());
    if inherit {
        let ctx_path = uniform_ref
            .get("context")
            .and_then(|c| c.get("path"))
            .and_then(|p| p.as_str())
            .unwrap_or("");
        let first_part = parent_path.filter(|p| !p.is_empty()).unwrap_or(ctx_path);
        let canonical = join_paths(&[Some(first_part), page_path]);
        if !canonical.is_empty() {
            uniform_ref["canonical"] = Value::String(canonical);
        }
    } else if page_path.map(|p| !p.is_empty()).unwrap_or(false) {
        uniform_ref["canonical"] = Value::String(join_paths(&[parent_path, page_path]));
    } else if let Some(pp) = parent_path.filter(|p| !p.is_empty()) {
        uniform_ref["canonical"] = Value::String(pp.to_string());
    }

    if !uniform_ref
        .get("context")
        .map(|c| truthy(Some(c)))
        .unwrap_or(false)
    {
        uniform_ref["context"] = Value::Object(Map::new());
    }
    uniform_ref["context"]["group"] = Value::Array(
        group_path
            .iter()
            .map(|g| Value::String(g.clone()))
            .collect(),
    );

    output.push(uniform_ref);
}

/// Port of pluginSidebar's `joinPaths`: strip outer slashes per part, prefix
/// `/`, join, collapse `//`, remove `/{param}` and `/:param` segments.
fn join_paths(parts: &[Option<&str>]) -> String {
    let mut joined = String::new();
    for part in parts.iter().flatten() {
        if part.is_empty() {
            continue; // .filter(Boolean)
        }
        let trimmed = part.trim_matches('/');
        if !trimmed.is_empty() {
            joined.push('/');
            joined.push_str(trimmed);
        }
    }
    // collapse duplicate slashes
    let mut collapsed = String::with_capacity(joined.len());
    let mut prev_slash = false;
    for c in joined.chars() {
        if c == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        collapsed.push(c);
    }
    // remove "/{param}" segments
    let collapsed = remove_segments(&collapsed, '{', Some('}'));
    // remove "/:param" segments
    remove_segments(&collapsed, ':', None)
}

/// Remove `/<marker>...` segments: `/{...}` (until the closing brace) or
/// `/:...` (until the next `/`).
fn remove_segments(s: &str, marker: char, closer: Option<char>) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == marker {
            let mut j = i + 2;
            match closer {
                Some(close) => {
                    // /\{[^}]+\}/ — requires at least one char + the closer
                    let start = j;
                    while j < chars.len() && chars[j] != close {
                        j += 1;
                    }
                    if j < chars.len() && j > start {
                        i = j + 1; // skip past the closer
                        continue;
                    }
                }
                None => {
                    // /\/:[^/]+/ — at least one non-slash char
                    let start = j;
                    while j < chars.len() && chars[j] != '/' {
                        j += 1;
                    }
                    if j > start {
                        i = j;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oap_schema_to_references;

    /// Convert, then run the x-docs pass the way `fused.rs` does.
    fn refs_after_xdocs(doc: &Value) -> Vec<Value> {
        let refs = oap_schema_to_references(doc, None);
        let mut refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        let (processed, stamps) = DocCtx::preprocess(doc);
        let ctx = DocCtx::with_merged_stamps(&processed, &stamps);
        apply_xdocs_sidebar(&ctx, &processed, &mut refs);
        refs
    }

    fn widgets_doc(sidebar: Option<Value>) -> Value {
        let mut doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Widgets", "version": "1.0.0" },
            "tags": [{ "name": "widgets" }],
            "paths": {
                "/widgets": {
                    "get": {
                        "operationId": "listWidgets",
                        "summary": "List widgets",
                        "tags": ["widgets"],
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/WidgetList" }
                                    }
                                }
                            }
                        }
                    }
                },
                "/internal/health": {
                    "get": {
                        "operationId": "health",
                        "summary": "Health",
                        "tags": ["widgets"],
                        "x-docs": { "hidden": true },
                        "responses": { "200": { "description": "ok" } }
                    }
                }
            },
            "components": {
                "schemas": {
                    "Widget": {
                        "x-docs": { "name": "The widget object", "group": "widgets" },
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "tags": { "type": "array", "items": { "$ref": "#/components/schemas/Tag" } }
                        }
                    },
                    "WidgetList": {
                        "x-docs": { "hidden": true },
                        "type": "object",
                        "properties": { "data": { "type": "array", "items": { "$ref": "#/components/schemas/Widget" } } }
                    },
                    "Part": {
                        "allOf": [
                            { "$ref": "#/components/schemas/PartBase" },
                            { "x-docs": { "name": "The part object", "group": "parts" }, "type": "object", "properties": { "sku": { "type": "string" } } }
                        ]
                    },
                    "PartBase": {
                        "type": "object",
                        "properties": { "id": { "type": "string" } }
                    },
                    "Tag": {
                        "x-docs": { "hidden": true },
                        "type": "object",
                        "properties": { "name": { "type": "string" } }
                    }
                }
            }
        });
        if let Some(sidebar) = sidebar {
            doc["x-docs"] = json!({ "sidebar": sidebar });
        }
        doc
    }

    /// Every nested `symbolDef` under a ref, as `(id, canonical)`.
    fn links_in(v: &Value, out: &mut Vec<(String, Option<String>)>) {
        match v {
            Value::Object(map) => {
                if let Some(sym) = map.get("symbolDef").and_then(|s| s.as_object()) {
                    if let Some(id) = sym.get("id").and_then(|i| i.as_str()) {
                        out.push((
                            id.to_string(),
                            sym.get("canonical")
                                .and_then(|c| c.as_str())
                                .map(|c| c.to_string()),
                        ));
                    }
                }
                for (key, child) in map {
                    if key != "symbolDef" {
                        links_in(child, out);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    links_in(item, out);
                }
            }
            _ => {}
        }
    }

    fn links_of(refs: &[Value], canonical: &str) -> Vec<(String, Option<String>)> {
        let r = refs
            .iter()
            .find(|r| r["canonical"] == canonical)
            .unwrap_or_else(|| panic!("no reference {canonical}"));
        let mut out = Vec::new();
        links_in(&r["definitions"], &mut out);
        out
    }

    fn titles(refs: &[Value]) -> Vec<String> {
        refs.iter()
            .map(|r| r["title"].as_str().unwrap_or("").to_string())
            .collect()
    }

    fn group_of(refs: &[Value], title: &str) -> Vec<String> {
        refs.iter()
            .find(|r| r["title"] == title)
            .and_then(|r| r["context"]["group"].as_array())
            .map(|g| g.iter().map(|v| v.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    }

    /// Outside sidebar mode a component schema's `x-docs.name` / `.group`
    /// retitle and regroup its page; `hidden` drops it; the rest keep the
    /// defaults (`<Name>` under "Objects").
    #[test]
    fn component_xdocs_outside_sidebar_mode() {
        let refs = refs_after_xdocs(&widgets_doc(None));
        assert_eq!(
            titles(&refs),
            vec![
                "List widgets",
                "The widget object",
                "The part object",
                "PartBase"
            ]
        );
        assert_eq!(group_of(&refs, "The widget object"), vec!["widgets"]);
        assert_eq!(group_of(&refs, "The part object"), vec!["parts"]);
        assert_eq!(group_of(&refs, "PartBase"), vec!["Objects"]);
        // The canonical is untouched — the page keeps its default URL.
        let widget = refs
            .iter()
            .find(|r| r["title"] == "The widget object")
            .unwrap();
        assert_eq!(widget["canonical"], "objects/Widget");
    }

    /// `x-docs.hidden` on an operation and on a schema both drop the ref.
    #[test]
    fn hidden_drops_from_the_generated_navigation() {
        let refs = refs_after_xdocs(&widgets_doc(None));
        let t = titles(&refs);
        assert!(
            !t.iter().any(|x| x == "Health"),
            "hidden operation leaked: {t:?}"
        );
        assert!(
            !t.iter().any(|x| x == "WidgetList"),
            "hidden schema leaked: {t:?}"
        );
    }

    /// An `x-docs.sidebar` is a complete enumeration: a listed page shows even
    /// when flagged hidden, and the listed object page takes the sidebar's
    /// group while keeping its `x-docs.name`.
    #[test]
    fn sidebar_listing_wins_over_hidden() {
        let sidebar = json!([{
            "group": "Reference",
            "pages": [
                { "type": "endpoint", "key": "health" },
                { "type": "object", "key": "WidgetList" },
                { "type": "object", "key": "Widget" }
            ]
        }]);
        let refs = refs_after_xdocs(&widgets_doc(Some(sidebar)));
        assert_eq!(
            titles(&refs),
            vec!["Health", "WidgetList", "The widget object"]
        );
        assert_eq!(group_of(&refs, "The widget object"), vec!["Reference"]);
    }

    /// A nested link to a schema whose page is hidden loses its `canonical`
    /// (there is no page to land on) and keeps its `id`; a link to a page that
    /// survives is untouched.
    #[test]
    fn links_to_a_hidden_schema_lose_their_canonical() {
        let refs = refs_after_xdocs(&widgets_doc(None));

        // objects/Widget → tags: array of Tag (hidden): the row and its items.
        let widget_links = links_of(&refs, "objects/Widget");
        let tag_links: Vec<_> = widget_links
            .iter()
            .filter(|(id, _)| id == "#/components/schemas/Tag")
            .collect();
        assert_eq!(tag_links.len(), 2, "{widget_links:?}");
        assert!(
            tag_links.iter().all(|(_, canonical)| canonical.is_none()),
            "a link to a hidden page kept its canonical: {widget_links:?}"
        );

        // listWidgets → WidgetList (hidden envelope, inlined) → data: array of
        // Widget (a page that survives): the link stays whole.
        let list_links = links_of(&refs, "listWidgets");
        assert!(
            list_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some("objects/Widget".to_string())
            )),
            "{list_links:?}"
        );
    }

    /// In sidebar mode a nested link follows the listed page's canonical (a
    /// page moved under another path included) and a link to an unlisted
    /// schema loses its canonical.
    #[test]
    fn sidebar_mode_links_follow_the_listed_page() {
        let sidebar = json!([{
            "group": "Reference",
            "pages": [
                { "type": "endpoint", "key": "listWidgets" },
                { "type": "object", "key": "Widget", "path": "the-widget" }
            ]
        }]);
        let refs = refs_after_xdocs(&widgets_doc(Some(sidebar)));
        assert_eq!(titles(&refs), vec!["List widgets", "The widget object"]);

        let widget_canonical = refs[1]["canonical"].as_str().unwrap().to_string();
        assert_ne!(widget_canonical, "objects/Widget");

        let list_links = links_of(&refs, "listWidgets");
        assert!(
            list_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some(widget_canonical.clone())
            )),
            "the link did not follow the page: {list_links:?}"
        );

        let widget_links = links_of(&refs, &widget_canonical);
        assert!(
            widget_links
                .iter()
                .filter(|(id, _)| id == "#/components/schemas/Tag")
                .all(|(_, canonical)| canonical.is_none()),
            "a link to an unlisted page kept its canonical: {widget_links:?}"
        );
    }

    /// A listed object page reads its `x-docs` from the same place as the
    /// generated navigation: the schema's own first, then an `allOf` member's.
    /// A composition without any `x-docs` is listed, so it stays, with its
    /// default title.
    #[test]
    fn sidebar_object_page_reads_the_same_xdocs_as_the_generated_navigation() {
        let mut doc = widgets_doc(Some(json!([{
            "group": "Reference",
            "pages": [
                { "type": "object", "key": "Bundle" },
                { "type": "object", "key": "Kit" },
                { "type": "object", "key": "Part" }
            ]
        }])));
        doc["components"]["schemas"]["Bundle"] = json!({
            "x-docs": { "name": "The bundle object" },
            "allOf": [
                { "$ref": "#/components/schemas/PartBase" },
                { "type": "object", "properties": { "count": { "type": "integer" } } }
            ]
        });
        doc["components"]["schemas"]["Kit"] = json!({
            "allOf": [{ "$ref": "#/components/schemas/PartBase" }]
        });
        let refs = refs_after_xdocs(&doc);
        assert_eq!(
            titles(&refs),
            vec!["The bundle object", "Kit", "The part object"]
        );
    }

    /// No `x-docs` anywhere: the pass is a no-op on the ref list.
    #[test]
    fn no_xdocs_is_a_no_op() {
        let mut doc = widgets_doc(None);
        doc["paths"]["/internal/health"]["get"]
            .as_object_mut()
            .unwrap()
            .remove("x-docs");
        for name in ["Widget", "WidgetList", "Tag"] {
            doc["components"]["schemas"][name]
                .as_object_mut()
                .unwrap()
                .remove("x-docs");
        }
        doc["components"]["schemas"]["Part"]["allOf"][1]
            .as_object_mut()
            .unwrap()
            .remove("x-docs");
        let before: Vec<Value> = oap_schema_to_references(&doc, None)
            .iter()
            .map(|r| serde_json::to_value(r).unwrap())
            .collect();
        let after = refs_after_xdocs(&doc);
        assert_eq!(before, after);
    }
}
