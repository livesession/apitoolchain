//! The generated navigation's finishing pass: the top-level objects group's
//! `collapsed` flag (the converter option `objectsCollapsed`). pluginNavigation
//! builds the group tree from `context.group` alone and knows nothing about
//! object pages; this pass runs on its output, so the flag lands on the
//! sidebar entry a renderer draws — not on a reference.

use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::xdocs::has_xdocs_sidebar;
use crate::{ObjectsLayout, Options};
use apitoolchain_uniform::jsrt::node_path_join;

/// The key of the flag on a generated group entry: `{ "group": "Objects",
/// "collapsed": true, "pages": [...] }`. Present (and `true`) on the one
/// entry it applies to, absent everywhere else — never `false`.
pub const COLLAPSED_KEY: &str = "collapsed";

/// Flag the generated navigation's TOP-LEVEL objects group `collapsed: true`,
/// so a renderer draws a toggle on its header and starts it folded (the
/// sub-groups inside it are ordinary nested groups, folded by a renderer on
/// their own). `sidebar` is pluginNavigation's output for `refs` (the final,
/// post-x-docs reference list) under `url_prefix`; `doc` the (preprocessed)
/// document.
///
/// The flag goes on exactly one entry, and only when every one of these holds:
/// - `options.objects_collapsed()` (the `objectsCollapsed` option, default
///   `true`);
/// - the `grouped` objects layout — under `per-area` there is no top-level
///   group of object pages to fold, the layout is unchanged;
/// - no `x-docs.sidebar` — a sidebar is the author's navigation, untouched;
/// - the top-level entry named `options.objects_group()` holds object pages
///   only (directly or in its sub-groups). An operation tagged with the
///   objects group's name shares that group; a group with an operation in it
///   stays open, so no operation is ever hidden behind the fold — an operation
///   group never carries the flag.
pub fn collapse_objects_group(
    sidebar: &mut [Value],
    doc: &Value,
    refs: &[Value],
    url_prefix: &str,
    options: &Options,
) {
    if !options.objects_collapsed()
        || options.objects_layout() != ObjectsLayout::Grouped
        || has_xdocs_sidebar(doc)
    {
        return;
    }
    let object_pages: HashSet<String> = refs
        .iter()
        .filter(|r| is_object_page(r))
        .filter_map(|r| r.get("canonical").and_then(|c| c.as_str()))
        .map(|canonical| node_path_join(&[url_prefix, canonical]))
        .collect();
    if object_pages.is_empty() {
        return;
    }
    let objects_group = options.objects_group();
    // Top-level group names are unique (pluginNavigation merges by name), so
    // at most one entry qualifies.
    if let Some(entry) = sidebar.iter_mut().find(|entry| {
        entry.get("group").and_then(|g| g.as_str()) == Some(objects_group)
            && holds_only(entry, &object_pages)
    }) {
        *entry = with_collapsed(entry);
    }
}

/// An object page: a component schema's reference (`context.componentSchema`).
fn is_object_page(r: &Value) -> bool {
    r.get("context")
        .and_then(|c| c.get("componentSchema"))
        .and_then(|s| s.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

/// Whether every page under a group entry — a direct leaf (`{virtual, page}`,
/// or the page path itself in store mode) or one inside a nested group — is
/// one of `pages`. An entry with nothing under it is not "only object pages".
fn holds_only(entry: &Value, pages: &HashSet<String>) -> bool {
    let Some(children) = entry.get("pages").and_then(|p| p.as_array()) else {
        return false;
    };
    if children.is_empty() {
        return false;
    }
    children.iter().all(|child| match child {
        Value::String(page) => pages.contains(page),
        Value::Object(o) if o.contains_key("group") => holds_only(child, pages),
        Value::Object(o) => o
            .get("page")
            .and_then(|p| p.as_str())
            .map(|p| pages.contains(p))
            .unwrap_or(false),
        _ => false,
    })
}

/// The entry with `collapsed: true` right after `group` (the JS twin's key
/// order; the goldens are compared key-order-insensitively, this is for the
/// reader of a dump).
fn with_collapsed(entry: &Value) -> Value {
    let Some(map) = entry.as_object() else {
        return entry.clone();
    };
    let mut out = Map::with_capacity(map.len() + 1);
    for (key, value) in map {
        if key == COLLAPSED_KEY {
            continue;
        }
        out.insert(key.clone(), value.clone());
        if key == "group" {
            out.insert(COLLAPSED_KEY.to_string(), Value::Bool(true));
        }
    }
    if !out.contains_key(COLLAPSED_KEY) {
        out.insert(COLLAPSED_KEY.to_string(), Value::Bool(true));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::DocCtx;
    use crate::xdocs::apply_xdocs_sidebar_with_options;
    use crate::{oap_schema_to_references, DEFAULT_OBJECTS_GROUP};
    use apitoolchain_uniform::plugins::plugin_navigation;
    use serde_json::json;

    const URL_PREFIX: &str = "docs/api";

    fn operation(id: &str, summary: &str, tags: &[&str]) -> Value {
        json!({
            "operationId": id,
            "summary": summary,
            "tags": tags,
            "responses": { "200": { "description": "ok" } }
        })
    }

    /// Widgets: a `widgets` resource with one operation and its object, a
    /// `parts` object whose group has no operations, and a schema with no
    /// group. `extra_paths` adds operations; `xdocs` a document-level
    /// `x-docs`.
    fn widgets_doc(extra_paths: Value, xdocs: Option<Value>) -> Value {
        let mut paths = json!({
            "/widgets": { "get": operation("listWidgets", "List widgets", &["widgets"]) }
        });
        if let Value::Object(extra) = extra_paths {
            for (k, v) in extra {
                paths[k] = v;
            }
        }
        let mut doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Widgets", "version": "1.0.0" },
            "tags": [{ "name": "widgets" }],
            "paths": paths,
            "components": { "schemas": {
                "Widget": {
                    "x-docs": { "name": "The widget object", "group": "widgets" },
                    "type": "object",
                    "properties": { "id": { "type": "string" } }
                },
                "Part": {
                    "x-docs": { "name": "The part object", "group": "parts" },
                    "type": "object",
                    "properties": { "id": { "type": "string" } }
                },
                "PartBase": {
                    "type": "object",
                    "properties": { "id": { "type": "string" } }
                }
            } }
        });
        if let Some(xdocs) = xdocs {
            doc["x-docs"] = xdocs;
        }
        doc
    }

    /// The fused flow on an in-memory document: convert → x-docs → navigation
    /// → this pass.
    fn sidebar_for(doc: &Value, options: &Options, store: bool) -> Vec<Value> {
        let refs = oap_schema_to_references(doc, Some(options.clone()));
        let mut refs: Vec<Value> = refs
            .iter()
            .map(|r| serde_json::to_value(r).expect("Reference serializes"))
            .collect();
        let (processed, stamps) = DocCtx::preprocess(doc);
        let ctx = DocCtx::with_merged_stamps(&processed, &stamps);
        apply_xdocs_sidebar_with_options(&ctx, &processed, &mut refs, options);
        let settings = json!({ "engine": { "uniform": { "store": store } } });
        let mut sidebar = plugin_navigation(&settings, URL_PREFIX, &refs)
            .expect("navigation")
            .sidebar;
        collapse_objects_group(&mut sidebar, &processed, &refs, URL_PREFIX, options);
        sidebar
    }

    fn entry<'a>(sidebar: &'a [Value], group: &str) -> &'a Value {
        sidebar
            .iter()
            .find(|e| e.get("group").and_then(|g| g.as_str()) == Some(group))
            .unwrap_or_else(|| panic!("no group {group} in {sidebar:?}"))
    }

    fn keys(entry: &Value) -> Vec<&str> {
        entry
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }

    fn flags_in(sidebar: &[Value]) -> usize {
        serde_json::to_string(sidebar)
            .unwrap()
            .matches(&format!("\"{COLLAPSED_KEY}\""))
            .count()
    }

    /// The default: the top-level "Objects" group carries `collapsed: true`
    /// (right after `group`), and nothing else does — not the operation
    /// group above it, not the area sub-groups inside it.
    #[test]
    fn the_objects_group_carries_the_flag_and_nothing_else_does() {
        let sidebar = sidebar_for(&widgets_doc(json!({}), None), &Options::default(), false);
        assert_eq!(
            sidebar
                .iter()
                .map(|e| e["group"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["widgets", DEFAULT_OBJECTS_GROUP]
        );
        let objects = entry(&sidebar, DEFAULT_OBJECTS_GROUP);
        assert_eq!(objects[COLLAPSED_KEY], Value::Bool(true));
        assert_eq!(keys(objects), vec!["group", COLLAPSED_KEY, "pages"]);
        let widgets = entry(&sidebar, "widgets");
        assert!(widgets.get(COLLAPSED_KEY).is_none(), "{widgets:?}");
        // The sub-groups are plain nested groups.
        let sub_groups: Vec<&Value> = objects["pages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p.get("group").is_some())
            .collect();
        assert_eq!(sub_groups.len(), 2, "{objects:?}");
        assert!(sub_groups.iter().all(|g| g.get(COLLAPSED_KEY).is_none()));
        assert_eq!(flags_in(&sidebar), 1);
    }

    /// An operation tagged with the objects group's name shares the group
    /// with the object pages: a group with an operation in it is never
    /// folded — an operation group never carries the flag.
    #[test]
    fn a_group_with_an_operation_in_it_stays_open() {
        let doc = widgets_doc(
            json!({
                "/objects": { "get": operation("listObjects", "List objects", &[DEFAULT_OBJECTS_GROUP]) }
            }),
            None,
        );
        let sidebar = sidebar_for(&doc, &Options::default(), false);
        let objects = entry(&sidebar, DEFAULT_OBJECTS_GROUP);
        assert!(
            objects["pages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["page"] == "docs/api/listObjects"),
            "the operation sits in the group: {objects:?}"
        );
        assert_eq!(flags_in(&sidebar), 0, "{sidebar:?}");
    }

    /// `objectsCollapsed: false` leaves the group open; `true` is the default
    /// spelled out; the renamed group (`objectsGroup`) carries the flag.
    #[test]
    fn the_option_and_the_group_name() {
        let doc = widgets_doc(json!({}), None);
        let open = sidebar_for(
            &doc,
            &Options {
                objects_collapsed: Some(false),
                ..Default::default()
            },
            false,
        );
        assert_eq!(flags_in(&open), 0, "{open:?}");
        let explicit = sidebar_for(
            &doc,
            &Options {
                objects_collapsed: Some(true),
                ..Default::default()
            },
            false,
        );
        assert_eq!(explicit, sidebar_for(&doc, &Options::default(), false));
        let renamed = sidebar_for(
            &doc,
            &Options {
                objects_group: Some("Types".into()),
                ..Default::default()
            },
            false,
        );
        assert_eq!(entry(&renamed, "Types")[COLLAPSED_KEY], Value::Bool(true));
        assert_eq!(flags_in(&renamed), 1);
    }

    /// The per-area layout and sidebar mode are untouched: no entry carries
    /// the flag — not even a sidebar group named like the objects group that
    /// lists object pages only.
    #[test]
    fn per_area_and_sidebar_mode_are_untouched() {
        let per_area = sidebar_for(
            &widgets_doc(json!({}), None),
            &Options {
                objects_layout: Some("per-area".into()),
                ..Default::default()
            },
            false,
        );
        assert!(per_area.iter().any(|e| e["group"] == DEFAULT_OBJECTS_GROUP));
        assert_eq!(flags_in(&per_area), 0, "{per_area:?}");

        let with_sidebar = widgets_doc(
            json!({}),
            Some(json!({
                "sidebar": [
                    { "group": "widgets", "pages": [{ "type": "endpoint", "key": "listWidgets" }] },
                    { "group": DEFAULT_OBJECTS_GROUP, "pages": [
                        { "type": "object", "key": "Widget" },
                        { "type": "object", "key": "Part" }
                    ] }
                ]
            })),
        );
        let sidebar = sidebar_for(&with_sidebar, &Options::default(), false);
        assert!(
            sidebar.iter().any(|e| e["group"] == DEFAULT_OBJECTS_GROUP),
            "{sidebar:?}"
        );
        assert_eq!(flags_in(&sidebar), 0, "{sidebar:?}");
    }

    /// Store mode lists the pages as plain paths; they are recognised too.
    #[test]
    fn store_mode_pages_are_recognised() {
        let sidebar = sidebar_for(&widgets_doc(json!({}), None), &Options::default(), true);
        let objects = entry(&sidebar, DEFAULT_OBJECTS_GROUP);
        assert_eq!(objects["pages"][0], "docs/api/objects/PartBase");
        assert_eq!(objects[COLLAPSED_KEY], Value::Bool(true));
        assert_eq!(flags_in(&sidebar), 1);
    }

    /// A document with no object pages has nothing to flag; the pass is a
    /// no-op on a navigation without the objects group.
    #[test]
    fn nothing_to_flag_without_object_pages() {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Widgets", "version": "1.0.0" },
            "paths": {
                "/widgets": { "get": operation("listWidgets", "List widgets", &["widgets"]) }
            }
        });
        let sidebar = sidebar_for(&doc, &Options::default(), false);
        assert_eq!(sidebar.len(), 1);
        assert_eq!(flags_in(&sidebar), 0);
    }
}
