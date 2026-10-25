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
use crate::links::object_type_constant;
use crate::page_paths::{component_page_path, explicit_page_path, kebab_case, operation_page_path};
use crate::{OasError, ObjectsLayout, OperationPaths, Options};
use apitoolchain_uniform::jsrt::truthy;

const COMPONENT_SCHEMA_PREFIX: &str = "#/components/schemas/";

/// `getXDocs(doc)?.route` (string) — the fileRouting side-channel.
pub fn get_xdocs_route(doc: &Value) -> Option<String> {
    doc.get("x-docs")?
        .get("route")?
        .as_str()
        .map(|s| s.to_string())
}

/// [`apply_xdocs_sidebar_with_options`] with the default [`Options`] (the
/// object pages' sub-group named [`crate::DEFAULT_OBJECTS_GROUP`]).
pub fn apply_xdocs_sidebar(
    ctx: &DocCtx,
    doc: &Value,
    refs: &mut Vec<Value>,
) -> Result<(), OasError> {
    apply_xdocs_sidebar_with_options(ctx, doc, refs, &Options::default())
}

/// Apply the plugin to `refs` in place: the per-ref x-docs meta pass, then
/// (when `x-docs.sidebar` exists) the sidebar-driven ref-list rebuild —
/// otherwise the generated navigation minus the `x-docs.hidden` refs.
/// `options.objects_group()` names the object pages' group and
/// `options.objects_layout()` says where an object page with a resource group
/// (`x-docs.group`) goes (see [`ObjectsLayout`]); `options.operation_paths()`
/// how a page without an `x-docs.path` gets its canonical (see
/// [`OperationPaths`]).
///
/// Errors when two pages of the generated navigation share one path — two
/// equal `x-docs.path`s, or two summaries that derive the same canonical —
/// naming both pages. (Under an `x-docs.sidebar` the entries place the pages
/// themselves and are not checked.)
pub fn apply_xdocs_sidebar_with_options(
    ctx: &DocCtx,
    doc: &Value,
    refs: &mut Vec<Value>,
    options: &Options,
) -> Result<(), OasError> {
    let objects_group = options.objects_group();
    let objects_layout = options.objects_layout();
    let derive_paths = options.operation_paths() == OperationPaths::Title;
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
            let meta = component_xdocs(ctx, doc, cs);
            if let Some(meta) = meta {
                if apply_docs_meta(r, meta, Some((objects_group, objects_layout))) {
                    hidden.push(i);
                }
            }
            // The page's path: its `x-docs.path`, else (title mode) the one
            // derived from its group and label. The links are relinked from
            // the final list below, so they follow.
            if let Some(canonical) = component_page_canonical(ctx, doc, cs, meta, derive_paths) {
                r["canonical"] = Value::String(canonical);
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

        let meta = op.get("x-docs");
        if let Some(meta) = meta {
            if apply_docs_meta(r, meta, None) {
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
        // The page's path: its `x-docs.path`, else (title mode) the one
        // derived from its group (set above) and its verb-first summary.
        if let Some(canonical) = operation_page_canonical(r, op, meta, derive_paths) {
            r["canonical"] = Value::String(canonical);
        }
    }

    // ---- defer pass: the sidebar-driven rebuild -------------------------
    let xdocs = doc.get("x-docs");
    let Some(sidebar) = xdocs_sidebar(doc) else {
        // No sidebar: the generated navigation, minus the hidden refs — and,
        // under the grouped objects layout, the objects group's own order.
        if !hidden.is_empty() {
            let mut i = 0;
            refs.retain(|_| {
                let keep = !hidden.contains(&i);
                i += 1;
                keep
            });
        }
        if objects_layout == ObjectsLayout::Grouped {
            order_objects_group(refs);
        }
        check_page_paths(refs)?;
        relink_from_final_list(refs);
        return Ok(());
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
    relink_from_final_list(refs);
    Ok(())
}

/// [`apply_xdocs_links_with_options`] with the default [`Options`].
pub fn apply_xdocs_links(ctx: &DocCtx, doc: &Value, refs: &mut [Value]) {
    apply_xdocs_links_with_options(ctx, doc, refs, &Options::default());
}

/// The regions-mode counterpart of [`apply_xdocs_sidebar_with_options`]: a
/// page's own references (one region), whose nested links still have to land
/// on pages that exist — read from the document, since a one-page reference
/// list cannot say which pages the navigation keeps. The JS plugin runs this
/// pass on every generated page; the twin keeps the Rust side able to do the
/// same. `options.operation_paths()` decides the derived page paths the links
/// follow, as in the whole-document pass.
pub fn apply_xdocs_links_with_options(
    ctx: &DocCtx,
    doc: &Value,
    refs: &mut [Value],
    options: &Options,
) {
    let links = schema_page_links(ctx, doc, options);
    relink_schema_links(refs, &|name| {
        links.get(name).cloned().unwrap_or(PageLink::Keep)
    });
}

/// A component page's canonical when something other than the converter
/// decides it: its `x-docs.path`, else — under `operationPaths: title` — the
/// derived `<group>/<label>` (`objects/<label>` without a group), the label
/// its constant `object` value verbatim, else its name in kebab-case. `None`
/// leaves the converter's `objects/<Name>`.
fn component_page_canonical(
    ctx: &DocCtx,
    doc: &Value,
    name: &str,
    meta: Option<&Value>,
    derive: bool,
) -> Option<String> {
    if let Some(path) = meta.and_then(explicit_path_of) {
        return Some(path);
    }
    if !derive {
        return None;
    }
    let schema = doc.get("components")?.get("schemas")?.get(name)?;
    let label = object_type_constant(ctx, schema).unwrap_or_else(|| kebab_case(name));
    let group = meta
        .and_then(|m| m.get("group"))
        .and_then(|g| g.as_str())
        .filter(|g| !g.trim().is_empty());
    Some(component_page_path(group, &label))
}

/// An operation page's canonical when something other than the converter
/// decides it: its `x-docs.path`, else — under `operationPaths: title` — the
/// `<group>/[<sub-resource>/]<verb>` derived from its (final) group and a
/// verb-first summary. `None` (no path, the default mode, a summary that is
/// not verb-first, no group) leaves the converter's canonical.
fn operation_page_canonical(
    r: &Value,
    op: &Value,
    meta: Option<&Value>,
    derive: bool,
) -> Option<String> {
    if let Some(path) = meta.and_then(explicit_path_of) {
        return Some(path);
    }
    if !derive {
        return None;
    }
    let summary = op.get("summary")?.as_str()?;
    let group = r
        .get("context")?
        .get("group")?
        .as_array()?
        .first()?
        .as_str()?;
    operation_page_path(summary, group)
}

/// `x-docs.path`, normalised (see [`explicit_page_path`]).
fn explicit_path_of(meta: &Value) -> Option<String> {
    explicit_page_path(meta.get("path")?.as_str()?)
}

/// Two pages of the generated navigation at one path are an error naming
/// both — one would overwrite the other's page.
fn check_page_paths(refs: &[Value]) -> Result<(), OasError> {
    let mut seen: HashMap<&str, &Value> = HashMap::new();
    for r in refs {
        let Some(canonical) = r.get("canonical").and_then(|c| c.as_str()) else {
            continue;
        };
        if let Some(first) = seen.insert(canonical, r) {
            return Err(OasError(format!(
                "two pages share the path \"{canonical}\": {} and {} — give one of them an `x-docs.path` of its own",
                describe_page(first),
                describe_page(r)
            )));
        }
    }
    Ok(())
}

/// `POST /widgets ("Create widget")` / `/components/schemas/Widget ("The
/// widget object")` — how an error names a page.
fn describe_page(r: &Value) -> String {
    let ctx = r.get("context");
    let get = |k: &str| ctx.and_then(|c| c.get(k)).and_then(|v| v.as_str());
    let region = match (get("method"), get("path"), get("componentSchema")) {
        (Some(method), Some(path), _) => format!("{} {path}", method.to_uppercase()),
        (_, _, Some(schema)) => format!("/components/schemas/{schema}"),
        _ => "a page".to_string(),
    };
    let title = r.get("title").and_then(|t| t.as_str()).unwrap_or("");
    format!("{region} (\"{title}\")")
}

/// What a nested link to a component schema becomes: the canonical of the
/// schema's page, no `canonical` at all when the schema has no page, or the
/// converter's link left as it is.
#[derive(Debug, Clone, PartialEq)]
enum PageLink {
    Set(String),
    Drop,
    Keep,
}

/// The page links once the reference list is FINAL (whole-document mode): a
/// schema's surviving reference gives its canonical — so a sidebar that moves
/// an object page under another path is followed — and a schema without one
/// has no page any more (`x-docs.hidden`, or not listed by an
/// `x-docs.sidebar`).
fn relink_from_final_list(refs: &mut [Value]) {
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
    relink_schema_links(refs, &|name| match canonical_by_schema.get(name) {
        Some(canonical) => PageLink::Set(canonical.clone()),
        None => PageLink::Drop,
    });
}

/// The page links read from the DOCUMENT (regions mode, where the reference
/// list is one page long and cannot say which pages exist): without an
/// `x-docs.sidebar` a schema has a page unless it is `x-docs.hidden`, at its
/// `x-docs.path` or its title-derived path when one applies; with a sidebar,
/// only the schemas it lists have a page, where the entry puts it (an entry
/// without a `path` leaves the page at its `x-docs.path` / derived path /
/// the converter's). The same rules the whole-document pass applies through
/// the surviving references.
fn schema_page_links(ctx: &DocCtx, doc: &Value, options: &Options) -> HashMap<String, PageLink> {
    let mut links: HashMap<String, PageLink> = HashMap::new();
    let Some(schemas) = doc
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(|s| s.as_object())
    else {
        return links;
    };
    let derive = options.operation_paths() == OperationPaths::Title;
    // The page's canonical when the document decides it (`x-docs.path`, the
    // title derivation), else the converter's.
    let own_link = |name: &str| -> PageLink {
        let meta = component_xdocs(ctx, doc, name);
        match component_page_canonical(ctx, doc, name, meta, derive) {
            Some(canonical) => PageLink::Set(canonical),
            None => PageLink::Keep,
        }
    };
    let xdocs = doc.get("x-docs");
    let sidebar = xdocs
        .and_then(|x| x.get("sidebar"))
        .and_then(|s| s.as_array());
    let Some(sidebar) = sidebar else {
        for name in schemas.keys() {
            let hidden = component_xdocs(ctx, doc, name)
                .map(|meta| truthy(meta.get("hidden")))
                .unwrap_or(false);
            links.insert(
                name.clone(),
                if hidden {
                    PageLink::Drop
                } else {
                    own_link(name)
                },
            );
        }
        return links;
    };

    for name in schemas.keys() {
        links.insert(name.clone(), PageLink::Drop);
    }
    let inherit = xdocs
        .and_then(|x| x.get("sidebarPathStrategy"))
        .and_then(|v| v.as_str())
        == Some("inherit");

    fn walk(
        pages: &[Value],
        parent_path: Option<&str>,
        inherit: bool,
        schemas: &Map<String, Value>,
        own_link: &dyn Fn(&str) -> PageLink,
        links: &mut HashMap<String, PageLink>,
    ) {
        for page in pages {
            if let Some(nested) = page.get("pages").and_then(|p| p.as_array()) {
                let pp = page.get("path").and_then(|p| p.as_str());
                walk(nested, pp, inherit, schemas, own_link, links);
            } else if page.get("type").and_then(|t| t.as_str()) == Some("object") {
                let Some(key) = page.get("key").and_then(|k| k.as_str()) else {
                    continue;
                };
                if !schemas.contains_key(key) {
                    continue;
                }
                // A component-schema reference has no `context.path` of its
                // own (that is an operation's), so `inherit` reads the group's.
                let link = match sidebar_page_canonical(inherit, page, parent_path, "") {
                    Some(canonical) => PageLink::Set(canonical),
                    None => own_link(key),
                };
                links.insert(key.to_string(), link);
            }
        }
    }
    for group in sidebar {
        if let Some(pages) = group.get("pages").and_then(|p| p.as_array()) {
            walk(pages, None, inherit, schemas, &own_link, &mut links);
        }
    }
    links
}

/// Schema links. A `symbolDef` whose `id` is a component schema
/// (`#/components/schemas/<Name>`) — a property's, a request/response root's
/// (the variant's) or a component page's own — takes that schema's page link:
/// the page's canonical, or no `canonical` at all when the schema has no page
/// — a link must never land on a page that does not exist. The `id` and the
/// `label` stay either way.
fn relink_schema_links(refs: &mut [Value], page_link: &dyn Fn(&str) -> PageLink) {
    for r in refs.iter_mut() {
        let Some(definitions) = r.get_mut("definitions").and_then(|d| d.as_array_mut()) else {
            continue;
        };
        for definition in definitions {
            relink_symbol_def(definition.get_mut("symbolDef"), page_link);
            relink_rows(definition.get_mut("properties"), page_link);
            relink_property(definition.get_mut("rootProperty"), page_link);
            if let Some(variants) = definition
                .get_mut("variants")
                .and_then(|v| v.as_array_mut())
            {
                for variant in variants {
                    relink_symbol_def(variant.get_mut("symbolDef"), page_link);
                    relink_rows(variant.get_mut("properties"), page_link);
                    relink_property(variant.get_mut("rootProperty"), page_link);
                }
            }
        }
    }
}

fn relink_symbol_def(symbol_def: Option<&mut Value>, page_link: &dyn Fn(&str) -> PageLink) {
    let Some(symbol_def) = symbol_def.and_then(|s| s.as_object_mut()) else {
        return;
    };
    let schema = symbol_def
        .get("id")
        .and_then(|i| i.as_str())
        .and_then(|i| i.strip_prefix(COMPONENT_SCHEMA_PREFIX))
        .map(|s| s.to_string());
    if let Some(schema) = schema {
        match page_link(&schema) {
            PageLink::Set(target) => {
                symbol_def.insert("canonical".to_string(), Value::String(target));
            }
            PageLink::Drop => {
                symbol_def.remove("canonical");
            }
            PageLink::Keep => {}
        }
    }
}

fn relink_rows(rows: Option<&mut Value>, page_link: &dyn Fn(&str) -> PageLink) {
    if let Some(rows) = rows.and_then(|r| r.as_array_mut()) {
        for row in rows {
            relink_property(Some(row), page_link);
        }
    }
}

fn relink_property(property: Option<&mut Value>, page_link: &dyn Fn(&str) -> PageLink) {
    let Some(property) = property.filter(|p| p.is_object()) else {
        return;
    };
    relink_symbol_def(property.get_mut("symbolDef"), page_link);
    relink_rows(property.get_mut("properties"), page_link);
    relink_property(property.get_mut("ofProperty"), page_link);
}

/// The document's `x-docs.sidebar` — sidebar mode — when it has one.
fn xdocs_sidebar(doc: &Value) -> Option<&Vec<Value>> {
    doc.get("x-docs")?.get("sidebar")?.as_array()
}

/// Whether the document is in sidebar mode (an `x-docs.sidebar`): the
/// navigation is the author's then, not the generated one — the condition the
/// defer pass above switches on, for the passes that run after it.
pub fn has_xdocs_sidebar(doc: &Value) -> bool {
    xdocs_sidebar(doc).is_some()
}

/// The common `x-docs` keys of an operation or a component schema: `name` →
/// the reference title, `group` → its sidebar group. A component schema's
/// `group` is a RESOURCE group (the one its endpoints sit in), and
/// `object_page` — its objects group name and the layout — says where its
/// page goes: under the grouped layout into that group's sub-group of the
/// top-level objects group — `["Objects", group]` — under per-area into the
/// objects sub-group of the resource group — `[group, "Objects"]` — after the
/// operations, which the navigation lists before any sub-group. An operation
/// (`object_page: None`) goes straight into `[group]`. Returns whether the
/// ref is `hidden`.
fn apply_docs_meta(
    r: &mut Value,
    meta: &Value,
    object_page: Option<(&str, ObjectsLayout)>,
) -> bool {
    if let Some(name) = meta.get("name").filter(|n| truthy(Some(n))) {
        r["title"] = name.clone();
    }
    if let Some(group) = meta.get("group").filter(|g| truthy(Some(g))) {
        // JS: `if (ref.context) ref.context.group = [meta.group]`
        if r.get("context").map(|c| truthy(Some(c))).unwrap_or(false) {
            r["context"]["group"] = match object_page {
                Some((objects_group, ObjectsLayout::PerArea)) => json!([group, objects_group]),
                Some((objects_group, ObjectsLayout::Grouped)) => json!([objects_group, group]),
                None => json!([group]),
            };
        }
    }
    truthy(meta.get("hidden"))
}

/// The grouped layout's order inside the objects group (no sidebar): the
/// pages without a resource group first, then the pages of each resource
/// group in the order the groups appear in the navigation above them (the
/// operations' top-level groups, first seen first), the groups with no
/// operations after those in schema order. The navigation lists a group's
/// direct pages before its sub-groups and the groups by first appearance, so
/// this order is the sidebar's. Every object page already sits after every
/// operation (the converter sorts them last); the sort is stable.
fn order_objects_group(refs: &mut [Value]) {
    fn is_object_page(r: &Value) -> bool {
        r.get("context")
            .and_then(|c| c.get("componentSchema"))
            .is_some()
    }
    fn group_at(r: &Value, i: usize) -> Option<&str> {
        r.get("context")?.get("group")?.as_array()?.get(i)?.as_str()
    }
    let mut area_rank: Vec<String> = Vec::new();
    for r in refs.iter() {
        if is_object_page(r) {
            continue;
        }
        if let Some(top) = group_at(r, 0) {
            if !area_rank.iter().any(|a| a == top) {
                area_rank.push(top.to_string());
            }
        }
    }
    refs.sort_by_key(|r| {
        if !is_object_page(r) {
            return 0;
        }
        match group_at(r, 1) {
            None => 1,
            Some(area) => {
                2 + area_rank
                    .iter()
                    .position(|a| a == area)
                    .unwrap_or(area_rank.len())
            }
        }
    });
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
    let own_path = uniform_ref
        .get("context")
        .and_then(|c| c.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(canonical) = sidebar_page_canonical(inherit, page, parent_path, &own_path) {
        uniform_ref["canonical"] = Value::String(canonical);
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

/// The canonical an `x-docs.sidebar` entry gives its page — the entry's
/// `path` under the enclosing group's `path`; under
/// `sidebarPathStrategy: inherit` the page's own path (an operation's) comes
/// first — or `None` when the entry leaves the page where the converter put
/// it.
fn sidebar_page_canonical(
    inherit: bool,
    page: &Value,
    parent_path: Option<&str>,
    own_path: &str,
) -> Option<String> {
    let page_path = page.get("path").and_then(|p| p.as_str());
    if inherit {
        let first_part = parent_path.filter(|p| !p.is_empty()).unwrap_or(own_path);
        let canonical = join_paths(&[Some(first_part), page_path]);
        return (!canonical.is_empty()).then_some(canonical);
    }
    if page_path.map(|p| !p.is_empty()).unwrap_or(false) {
        return Some(join_paths(&[parent_path, page_path]));
    }
    parent_path.filter(|p| !p.is_empty()).map(|p| p.to_string())
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
        apply_xdocs_sidebar(&ctx, &processed, &mut refs).expect("no page-path collision");
        refs
    }

    /// Like [`refs_after_xdocs`], the collision error surfaced.
    fn try_refs_after_xdocs(doc: &Value, options: &crate::Options) -> Result<Vec<Value>, OasError> {
        let refs = oap_schema_to_references(doc, Some(options.clone()));
        let mut refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        let (processed, stamps) = DocCtx::preprocess(doc);
        let ctx = DocCtx::with_merged_stamps(&processed, &stamps);
        apply_xdocs_sidebar_with_options(&ctx, &processed, &mut refs, options)?;
        Ok(refs)
    }

    /// Convert ONE region the way a generated page does, then the
    /// regions-mode pass (the links only).
    fn region_after_xdocs(doc: &Value, region: &str) -> Vec<Value> {
        let options = crate::Options {
            regions: Some(vec![region.to_string()]),
            ..Default::default()
        };
        let refs = oap_schema_to_references(doc, Some(options));
        let mut refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        let (processed, stamps) = DocCtx::preprocess(doc);
        let ctx = DocCtx::with_merged_stamps(&processed, &stamps);
        apply_xdocs_links(&ctx, &processed, &mut refs);
        refs
    }

    /// [`region_after_xdocs`] with options (the title-derived paths).
    fn region_after_xdocs_with(doc: &Value, region: &str, options: &crate::Options) -> Vec<Value> {
        let options = crate::Options {
            regions: Some(vec![region.to_string()]),
            ..options.clone()
        };
        let refs = oap_schema_to_references(doc, Some(options.clone()));
        let mut refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        let (processed, stamps) = DocCtx::preprocess(doc);
        let ctx = DocCtx::with_merged_stamps(&processed, &stamps);
        apply_xdocs_links_with_options(&ctx, &processed, &mut refs, &options);
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

    /// Convert with `options`, then the x-docs pass with the same options.
    fn refs_after_xdocs_with(doc: &Value, options: &crate::Options) -> Vec<Value> {
        let refs = oap_schema_to_references(doc, Some(options.clone()));
        let mut refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        let (processed, stamps) = DocCtx::preprocess(doc);
        let ctx = DocCtx::with_merged_stamps(&processed, &stamps);
        apply_xdocs_sidebar_with_options(&ctx, &processed, &mut refs, options)
            .expect("no page-path collision");
        refs
    }

    fn canonicals(refs: &[Value]) -> Vec<String> {
        refs.iter()
            .map(|r| r["canonical"].as_str().unwrap_or("").to_string())
            .collect()
    }

    /// Outside sidebar mode a component schema's `x-docs.name` / `.group`
    /// retitle and regroup its page: under the default layout (`grouped`)
    /// every object page sits in ONE top-level "Objects" group after the
    /// operations — the pages without a resource group first, then one
    /// sub-group per resource group in the order the groups appear above
    /// (`widgets` has operations, `parts` has none, so it follows); `hidden`
    /// drops it.
    #[test]
    fn component_xdocs_outside_sidebar_mode() {
        let refs = refs_after_xdocs(&widgets_doc(None));
        assert_eq!(
            titles(&refs),
            vec![
                "List widgets",
                "PartBase",
                "The widget object",
                "The part object"
            ]
        );
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["Objects", "widgets"]
        );
        assert_eq!(group_of(&refs, "The part object"), vec!["Objects", "parts"]);
        assert_eq!(group_of(&refs, "PartBase"), vec!["Objects"]);
        // An operation's `x-docs.group` is never nested.
        assert_eq!(group_of(&refs, "List widgets"), vec!["widgets"]);
        // The canonical is untouched — the page keeps its default URL.
        let widget = refs
            .iter()
            .find(|r| r["title"] == "The widget object")
            .unwrap();
        assert_eq!(widget["canonical"], "objects/Widget");
    }

    /// The `objectsGroup` option renames both the top-level objects group and
    /// the per-area sub-group; an empty name means the default.
    #[test]
    fn objects_group_option_renames_the_flat_group_and_the_sub_group() {
        let doc = widgets_doc(None);
        let options = crate::Options {
            objects_group: Some("Types".into()),
            ..Default::default()
        };
        let refs = oap_schema_to_references(&doc, Some(options.clone()));
        let refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        assert_eq!(group_of(&refs, "Widget"), vec!["Types"]);
        let refs = refs_after_xdocs_with(&doc, &options);
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["Types", "widgets"]
        );
        assert_eq!(group_of(&refs, "The part object"), vec!["Types", "parts"]);
        assert_eq!(group_of(&refs, "PartBase"), vec!["Types"]);

        let per_area = crate::Options {
            objects_group: Some("Types".into()),
            objects_layout: Some("per-area".into()),
            ..Default::default()
        };
        let refs = refs_after_xdocs_with(&doc, &per_area);
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["widgets", "Types"]
        );
        assert_eq!(group_of(&refs, "PartBase"), vec!["Types"]);

        let empty = crate::Options {
            objects_group: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(empty.objects_group(), crate::DEFAULT_OBJECTS_GROUP);
    }

    /// `objectsLayout: per-area` nests an object page under its resource
    /// group's "Objects" sub-group (after the group's operations, which the
    /// navigation lists before any sub-group); the object pages keep the
    /// schema order. An unknown layout name means the default.
    #[test]
    fn per_area_layout_nests_the_object_pages_inside_their_groups() {
        let doc = widgets_doc(None);
        let per_area = crate::Options {
            objects_layout: Some("per-area".into()),
            ..Default::default()
        };
        let refs = refs_after_xdocs_with(&doc, &per_area);
        assert_eq!(
            titles(&refs),
            vec![
                "List widgets",
                "The widget object",
                "The part object",
                "PartBase"
            ]
        );
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["widgets", "Objects"]
        );
        assert_eq!(group_of(&refs, "The part object"), vec!["parts", "Objects"]);
        assert_eq!(group_of(&refs, "PartBase"), vec!["Objects"]);
        assert_eq!(group_of(&refs, "List widgets"), vec!["widgets"]);

        let unknown = crate::Options {
            objects_layout: Some("sideways".into()),
            ..Default::default()
        };
        let refs = refs_after_xdocs_with(&doc, &unknown);
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["Objects", "widgets"]
        );
    }

    /// The grouped layout's ordering rule on the 11.objects-layouts fixture:
    /// the operations in tag order (declared `parts`, `widgets`; the
    /// undeclared `assemblies`; the untagged `getStatus`), then EVERY object
    /// page — the two without a resource group first (schema order), then the
    /// groups in the order they appear above (`parts`, `widgets`,
    /// `assemblies`), then `fasteners`, a group with no operations.
    #[test]
    fn grouped_layout_orders_the_objects_group_by_the_groups_above() {
        let input =
            apitoolchain_parity::fixtures_dir(env!("CARGO_MANIFEST_DIR"), "apitoolchain-openapi")
                .join("11.objects-layouts")
                .join("input.yaml");
        let doc = crate::read_spec(input.to_str().unwrap()).expect("fixture parses");
        let refs = refs_after_xdocs(&doc);
        assert_eq!(
            canonicals(&refs),
            vec![
                "listParts",
                "listWidgets",
                "listAudit",
                "createAssembly",
                "getStatus",
                "objects/Status",
                "objects/Address",
                "objects/Part",
                "objects/Widget",
                "objects/Assembly",
                "objects/Fastener",
            ]
        );
        let groups: Vec<Vec<String>> = refs
            .iter()
            .map(|r| {
                r["context"]["group"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect()
            })
            .collect();
        assert_eq!(
            groups,
            vec![
                vec!["parts"],
                vec!["widgets"],
                vec!["audit"],
                vec!["assemblies"],
                vec![""],
                vec!["Objects"],
                vec!["Objects"],
                vec!["Objects", "parts"],
                vec!["Objects", "widgets"],
                vec!["Objects", "assemblies"],
                vec!["Objects", "fasteners"],
            ]
        );
    }

    /// A sidebar entry's group path is the sidebar's: no sub-group is added
    /// to a listed object page.
    #[test]
    fn sidebar_mode_does_not_nest_listed_object_pages() {
        let sidebar = json!([{
            "group": "Reference",
            "pages": [
                { "type": "endpoint", "key": "listWidgets" },
                { "type": "object", "key": "Widget" }
            ]
        }]);
        let refs = refs_after_xdocs(&widgets_doc(Some(sidebar)));
        assert_eq!(group_of(&refs, "The widget object"), vec!["Reference"]);
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
        // The response ROOT links to the hidden envelope: the variant's own
        // symbolDef loses its canonical too (and keeps its id and label).
        assert!(
            list_links.contains(&("#/components/schemas/WidgetList".to_string(), None)),
            "{list_links:?}"
        );
        let list = refs
            .iter()
            .find(|r| r["canonical"] == "listWidgets")
            .unwrap();
        let root = &list["definitions"][0]["variants"][0]["symbolDef"];
        assert_eq!(root["label"], "WidgetList");
        assert!(root.get("canonical").is_none(), "{root}");
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

    /// Regions mode (one page at a time): the links follow the document, not
    /// the one-page reference list — a link to a hidden schema loses its
    /// canonical, a link to a page that exists keeps it.
    #[test]
    fn regions_mode_links_follow_the_document() {
        let doc = widgets_doc(None);

        let refs = region_after_xdocs(&doc, "/components/schemas/Widget");
        assert_eq!(refs.len(), 1, "{:?}", titles(&refs));
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

        let refs = region_after_xdocs(&doc, "GET /widgets");
        assert_eq!(refs.len(), 1, "{:?}", titles(&refs));
        let list_links = links_of(&refs, "listWidgets");
        assert!(
            list_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some("objects/Widget".to_string())
            )),
            "{list_links:?}"
        );
    }

    /// Regions mode under an `x-docs.sidebar`: a link follows the listed
    /// page's canonical exactly as the whole-document pass computes it, and a
    /// link to an unlisted schema loses its canonical.
    #[test]
    fn regions_mode_links_follow_the_sidebar() {
        let sidebar = json!([{
            "group": "Reference",
            "pages": [
                { "type": "endpoint", "key": "listWidgets" },
                { "type": "object", "key": "Widget", "path": "the-widget" }
            ]
        }]);
        let doc = widgets_doc(Some(sidebar));
        let whole = refs_after_xdocs(&doc);
        let widget_canonical = whole[1]["canonical"].as_str().unwrap().to_string();
        assert_ne!(widget_canonical, "objects/Widget");

        let refs = region_after_xdocs(&doc, "GET /widgets");
        let list_links = links_of(&refs, "listWidgets");
        assert!(
            list_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some(widget_canonical.clone())
            )),
            "the link did not follow the page: {list_links:?}"
        );

        // The page itself keeps the converter's canonical in regions mode (the
        // sidebar pass renames pages, this one only relinks); its rows follow
        // the document.
        let refs = region_after_xdocs(&doc, "/components/schemas/Widget");
        let widget_links = links_of(&refs, "objects/Widget");
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

    fn title_mode() -> crate::Options {
        crate::Options {
            operation_paths: Some("title".into()),
            ..Default::default()
        }
    }

    /// `x-docs.path` names a page's path outright — on an operation and on a
    /// component schema — under the default mode: the canonical is the path
    /// (normalised: no outer slashes), the sidebar grouping is untouched, and
    /// every link to the schema follows the final canonical.
    #[test]
    fn explicit_paths_set_the_canonical_and_the_links_follow() {
        let mut doc = widgets_doc(None);
        doc["paths"]["/widgets"]["get"]["x-docs"] = json!({ "path": "/widgets/list/" });
        doc["components"]["schemas"]["Widget"]["x-docs"]["path"] = json!("widgets/object");
        let refs = refs_after_xdocs(&doc);
        assert_eq!(
            canonicals(&refs),
            vec![
                "widgets/list",
                "objects/PartBase",
                "widgets/object",
                "objects/Part"
            ]
        );
        assert_eq!(group_of(&refs, "List widgets"), vec!["widgets"]);
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["Objects", "widgets"]
        );
        let list_links = links_of(&refs, "widgets/list");
        assert!(
            list_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some("widgets/object".to_string())
            )),
            "the link did not follow the explicit path: {list_links:?}"
        );
        // The page's own link too.
        let widget = refs
            .iter()
            .find(|r| r["canonical"] == "widgets/object")
            .unwrap();
        assert_eq!(
            widget["definitions"][0]["symbolDef"]["canonical"],
            "widgets/object"
        );
    }

    /// Two pages at one path are an error naming both — two equal
    /// `x-docs.path`s, or (title mode) two summaries that derive the same
    /// canonical.
    #[test]
    fn colliding_page_paths_are_an_error_naming_both() {
        let mut doc = widgets_doc(None);
        doc["paths"]["/widgets/all"] = json!({
            "get": {
                "operationId": "listAllWidgets",
                "summary": "List all widgets",
                "tags": ["widgets"],
                "x-docs": { "path": "widgets/list" },
                "responses": { "200": { "description": "ok" } }
            }
        });
        doc["paths"]["/widgets"]["get"]["x-docs"] = json!({ "path": "widgets/list" });
        let err = try_refs_after_xdocs(&doc, &crate::Options::default())
            .expect_err("a collision is an error");
        let message = err.to_string();
        for needle in [
            "\"widgets/list\"",
            "GET /widgets (\"List widgets\")",
            "GET /widgets/all (\"List all widgets\")",
        ] {
            assert!(message.contains(needle), "{message}");
        }

        // Derived: both summaries are "List … widgets" in `widgets`.
        doc["paths"]["/widgets"]["get"]
            .as_object_mut()
            .unwrap()
            .remove("x-docs");
        doc["paths"]["/widgets/all"]["get"]
            .as_object_mut()
            .unwrap()
            .remove("x-docs");
        let message = try_refs_after_xdocs(&doc, &title_mode())
            .expect_err("a derived collision is an error")
            .to_string();
        assert!(message.contains("\"widgets/list\""), "{message}");
        assert!(message.contains("GET /widgets/all"), "{message}");
        // An `x-docs.path` on one of them resolves it.
        doc["paths"]["/widgets/all"]["get"]["x-docs"] = json!({ "path": "widgets/list-all" });
        let refs = try_refs_after_xdocs(&doc, &title_mode()).expect("resolved");
        assert!(canonicals(&refs).contains(&"widgets/list-all".to_string()));
        // Under the default mode the operationIds differ: no collision.
        try_refs_after_xdocs(&doc, &crate::Options::default()).expect("distinct operationIds");
    }

    /// `operationPaths: title`: an operation derives `<group>/[<sub>/]<verb>`
    /// from its group and verb-first summary; a component page `<group>/<label>`
    /// (the label its constant `object` value, else the kebab-cased name) or
    /// `objects/<label>` without a group; an `x-docs.path` wins over both; a
    /// summary that is not verb-first keeps the operationId. The links follow.
    #[test]
    fn title_mode_derives_the_canonicals() {
        let mut doc = widgets_doc(None);
        doc["paths"]["/widgets/{widgetId}/parts"] = json!({
            "get": {
                "operationId": "listWidgetParts",
                "summary": "List widget parts",
                "tags": ["widgets"],
                "responses": { "200": { "description": "ok" } }
            }
        });
        doc["paths"]["/widgets/{widgetId}/archive"] = json!({
            "post": {
                "operationId": "archiveWidget",
                "summary": "Widget archival",
                "tags": ["widgets"],
                "responses": { "200": { "description": "ok" } }
            }
        });
        doc["paths"]["/status"] = json!({
            "get": {
                "operationId": "getStatus",
                "summary": "Retrieve status",
                "responses": { "200": { "description": "ok" } }
            }
        });
        doc["components"]["schemas"]["Widget"]["properties"]["object"] =
            json!({ "type": "string", "enum": ["widget"] });
        doc["components"]["schemas"]["Part"]["allOf"][1]["x-docs"]["path"] = json!("parts/object");
        let refs = refs_after_xdocs_with(&doc, &title_mode());
        assert_eq!(
            canonicals(&refs),
            vec![
                "widgets/list",
                "widgets/parts/list",
                "archiveWidget",
                "getStatus",
                "objects/part-base",
                "widgets/widget",
                "parts/object",
            ]
        );
        // The grouping is the layout's, not the path's.
        assert_eq!(
            group_of(&refs, "The widget object"),
            vec!["Objects", "widgets"]
        );
        let list_links = links_of(&refs, "widgets/list");
        assert!(
            list_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some("widgets/widget".to_string())
            )),
            "the link did not follow the derived path: {list_links:?}"
        );
        // Regions mode (the links only): the same targets, read from the
        // document.
        let page = region_after_xdocs_with(&doc, "GET /widgets", &title_mode());
        let page_links = links_of(&page, "listWidgets");
        assert!(
            page_links.contains(&(
                "#/components/schemas/Widget".to_string(),
                Some("widgets/widget".to_string())
            )),
            "{page_links:?}"
        );
        let widget_page =
            region_after_xdocs_with(&doc, "/components/schemas/Widget", &title_mode());
        let widget_links = links_of(&widget_page, "objects/Widget");
        assert!(
            widget_links
                .iter()
                .filter(|(id, _)| id == "#/components/schemas/Tag")
                .all(|(_, canonical)| canonical.is_none()),
            "{widget_links:?}"
        );
        // An explicit path is followed in regions mode too.
        doc["components"]["schemas"]["Widget"]["x-docs"]["path"] = json!("widgets/object");
        let page = region_after_xdocs_with(&doc, "GET /widgets", &crate::Options::default());
        assert!(links_of(&page, "listWidgets").contains(&(
            "#/components/schemas/Widget".to_string(),
            Some("widgets/object".to_string())
        )));
    }

    /// Under an `x-docs.sidebar` the entries place the pages: an entry with a
    /// `path` wins over the schema's `x-docs.path`; one without leaves the
    /// page at its `x-docs.path`. Collisions are not checked there.
    #[test]
    fn sidebar_entries_win_over_explicit_paths() {
        let sidebar = json!([{
            "group": "Reference",
            "pages": [
                { "type": "endpoint", "key": "listWidgets" },
                { "type": "object", "key": "Widget", "path": "the-widget" }
            ]
        }]);
        let mut doc = widgets_doc(Some(sidebar));
        doc["paths"]["/widgets"]["get"]["x-docs"] = json!({ "path": "widgets/list" });
        doc["components"]["schemas"]["Widget"]["x-docs"]["path"] = json!("widgets/object");
        let refs = refs_after_xdocs(&doc);
        assert_eq!(refs[0]["canonical"], "widgets/list");
        let widget_canonical = refs[1]["canonical"].as_str().unwrap();
        assert!(
            widget_canonical.ends_with("the-widget"),
            "{widget_canonical}"
        );
        let list_links = links_of(&refs, "widgets/list");
        assert!(list_links.contains(&(
            "#/components/schemas/Widget".to_string(),
            Some(widget_canonical.to_string())
        )));
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
