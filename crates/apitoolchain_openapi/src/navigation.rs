//! The generated navigation's finishing passes, in the order they run:
//!
//! 1. [`collapse_objects_group`] — the top-level objects group's `collapsed`
//!    flag (the converter option `objectsCollapsed`);
//! 2. [`group_by_tag_groups`] — the document's tag groups (`x-tagGroups`) as
//!    sections over the top-level groups, and the tags' display names
//!    (`x-displayName`) as the groups' labels. The document drives it; there
//!    is no option.
//!
//! pluginNavigation builds the group tree from `context.group` alone and
//! knows nothing about object pages, sections or labels; these passes run on
//! its output, so what they change lands on the sidebar entries a renderer
//! draws — never on a reference (canonicals, links and page paths stay as
//! they are).

use std::collections::{HashMap, HashSet};

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

/// The document key of the tag groups: `x-tagGroups: [{ name, tags: [...] }]`,
/// the sections of the generated navigation (the extension Redoc reads).
pub const TAG_GROUPS_KEY: &str = "x-tagGroups";

/// The key of a tag's display name: `tags: [{ name, x-displayName }]`, the
/// label of the tag's groups in the generated navigation (the extension Redoc
/// reads).
pub const DISPLAY_NAME_KEY: &str = "x-displayName";

/// One well-formed `x-tagGroups` entry: the section's name and the group
/// names it lists, in order.
struct TagGroup {
    name: String,
    tags: Vec<String>,
}

/// Group the generated navigation's top-level groups into sections (the
/// document's `x-tagGroups`) and label every generated group with its tag's
/// display name (the tags' `x-displayName`). `sidebar` is the navigation
/// after [`collapse_objects_group`] — which finds the objects group by its
/// raw name, so it runs first; `doc` the (preprocessed) document.
///
/// A no-op with an `x-docs.sidebar` (the author's navigation, untouched) or
/// when the document has neither a well-formed tag group nor a tag with a
/// non-empty string display name. Otherwise:
///
/// - **Sections.** A tag group is `{ name: <a non-empty string>, tags: [<the
///   group names it lists>] }`; malformed entries (and non-string or empty
///   names in `tags`) are skipped, and a tag group that repeats an earlier
///   one's name is skipped. The top-level entries are walked in order: an
///   entry whose (raw) group name a tag group lists moves into that tag
///   group's section — the first tag group listing it wins. The section,
///   `{ "group": <name>, "pages": [...] }`, takes the place of its first
///   member, and its members follow the tag group's `tags` order. Every
///   other entry keeps its place — no page is ever hidden; the objects group
///   is not a tag and stays where it is. A listed name without an entry adds
///   nothing, and a section without members is not emitted. A tag group named
///   like a top-level group that keeps its place (its name or its display
///   name) is skipped, so no two top-level groups share a name. A section is
///   a plain group entry: no flag is added — the groups moved into it become
///   nested groups, which a renderer folds on its own.
/// - **Labels.** Every generated group entry, at any depth, whose name is a
///   document tag's `name` takes that tag's `x-displayName` (the first tag of
///   a name wins) — the groups inside a section and the sub-groups of the
///   objects group alike. A section's own name is never relabelled; page
///   entries and the other keys (`collapsed`) are untouched, and the key
///   order is kept (`group`, then `collapsed` if present, then `pages`).
pub fn group_by_tag_groups(sidebar: &mut Vec<Value>, doc: &Value) {
    if has_xdocs_sidebar(doc) {
        return;
    }
    let tag_groups = tag_groups_of(doc);
    let display_names = display_names_of(doc);
    if tag_groups.is_empty() && display_names.is_empty() {
        return;
    }
    let owners = section_owners(sidebar, &tag_groups, &display_names);

    // Each section's slot in `out` (taken by its first member) and its
    // members with their rank in the tag group's `tags`.
    let mut sections: HashMap<usize, (usize, Vec<(usize, Value)>)> = HashMap::new();
    let mut out: Vec<Value> = Vec::with_capacity(sidebar.len());
    for (mut entry, owner) in std::mem::take(sidebar).into_iter().zip(owners) {
        relabel(&mut entry, &display_names);
        match owner {
            Some((section, rank)) => {
                let (_, members) = sections.entry(section).or_insert_with(|| {
                    out.push(Value::Null);
                    (out.len() - 1, Vec::new())
                });
                members.push((rank, entry));
            }
            None => out.push(entry),
        }
    }
    for (section, (slot, mut members)) in sections {
        // Stable: entries of one rank (one name) keep their order.
        members.sort_by_key(|(rank, _)| *rank);
        let mut entry = Map::with_capacity(2);
        entry.insert(
            "group".to_string(),
            Value::String(tag_groups[section].name.clone()),
        );
        entry.insert(
            "pages".to_string(),
            Value::Array(members.into_iter().map(|(_, member)| member).collect()),
        );
        out[slot] = Value::Object(entry);
    }
    *sidebar = out;
}

/// The document's well-formed tag groups, in order (see
/// [`group_by_tag_groups`]).
fn tag_groups_of(doc: &Value) -> Vec<TagGroup> {
    let Some(groups) = doc.get(TAG_GROUPS_KEY).and_then(|g| g.as_array()) else {
        return Vec::new();
    };
    let mut out: Vec<TagGroup> = Vec::new();
    for group in groups {
        let Some(name) = group
            .get("name")
            .and_then(|n| n.as_str())
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        let Some(tags) = group.get("tags").and_then(|t| t.as_array()) else {
            continue;
        };
        if out.iter().any(|earlier| earlier.name == name) {
            continue;
        }
        out.push(TagGroup {
            name: name.to_string(),
            tags: tags
                .iter()
                .filter_map(|t| t.as_str())
                .filter(|t| !t.is_empty())
                .map(String::from)
                .collect(),
        });
    }
    out
}

/// Tag name → display name: the document tags with a non-empty string
/// `x-displayName` (the first tag of a name wins).
fn display_names_of(doc: &Value) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let Some(tags) = doc.get("tags").and_then(|t| t.as_array()) else {
        return out;
    };
    let mut seen: HashSet<&str> = HashSet::new();
    for tag in tags {
        let Some(name) = tag
            .get("name")
            .and_then(|n| n.as_str())
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        if !seen.insert(name) {
            continue;
        }
        if let Some(label) = tag
            .get(DISPLAY_NAME_KEY)
            .and_then(|l| l.as_str())
            .filter(|l| !l.is_empty())
        {
            out.insert(name.to_string(), label.to_string());
        }
    }
    out
}

/// Per top-level entry: the tag group it moves into and its rank in that tag
/// group's `tags`, or `None` when it keeps its place. A tag group named like
/// an entry that keeps its place (its name or its display name — the label it
/// takes afterwards) is dropped — which can leave more entries in place (the
/// ones it held), so the assignment repeats until nothing more is dropped (at
/// most once per tag group).
fn section_owners(
    sidebar: &[Value],
    tag_groups: &[TagGroup],
    display_names: &HashMap<String, String>,
) -> Vec<Option<(usize, usize)>> {
    let names: Vec<Option<&str>> = sidebar
        .iter()
        .map(|entry| entry.get("group").and_then(|g| g.as_str()))
        .collect();
    let mut active = vec![true; tag_groups.len()];
    loop {
        let owners: Vec<Option<(usize, usize)>> = names
            .iter()
            .map(|name| {
                let name = (*name)?;
                tag_groups
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| active[*i])
                    .find_map(|(i, group)| {
                        group.tags.iter().position(|t| t == name).map(|r| (i, r))
                    })
            })
            .collect();
        let mut dropped = false;
        for (i, group) in tag_groups.iter().enumerate() {
            let collides = names.iter().zip(&owners).any(|(name, owner)| {
                owner.is_none()
                    && name.is_some_and(|n| {
                        n == group.name || display_names.get(n).is_some_and(|l| *l == group.name)
                    })
            });
            if active[i] && collides {
                active[i] = false;
                dropped = true;
            }
        }
        if !dropped {
            return owners;
        }
    }
}

/// Label a group entry and every group under it with the display names; a
/// page entry (a `{virtual, page}` leaf, or the page path in store mode) is
/// left as it is.
fn relabel(entry: &mut Value, display_names: &HashMap<String, String>) {
    let Value::Object(map) = entry else {
        return;
    };
    if !map.contains_key("group") {
        return;
    }
    if let Some(Value::String(name)) = map.get_mut("group") {
        if let Some(label) = display_names.get(name.as_str()) {
            *name = label.clone();
        }
    }
    if let Some(Value::Array(pages)) = map.get_mut("pages") {
        for page in pages {
            relabel(page, display_names);
        }
    }
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
        apply_xdocs_sidebar_with_options(&ctx, &processed, &mut refs, options).expect("x-docs");
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

    // ---- group_by_tag_groups ------------------------------------------------

    /// A store: one operation per tag — `pets`, `store`, `users`, `audit`,
    /// the declared order — a `Money` object page without an area, a `Pet`
    /// one in the `pets` area and a `Report` one in `reports` (a declared tag
    /// with no operations). `display_names` gives tags an `x-displayName`;
    /// `root` merges document keys (`x-tagGroups`, `x-docs`).
    fn petstore_doc(display_names: &[(&str, &str)], root: Value) -> Value {
        let tags: Vec<Value> = ["pets", "store", "users", "audit", "reports"]
            .iter()
            .map(|name| {
                let mut tag = json!({ "name": name });
                if let Some((_, label)) = display_names.iter().find(|(n, _)| n == name) {
                    tag[DISPLAY_NAME_KEY] = json!(label);
                }
                tag
            })
            .collect();
        let object = |name: &str, group: Option<&str>| {
            let mut schema = json!({
                "x-docs": { "name": format!("The {name} object") },
                "type": "object",
                "properties": { "id": { "type": "string" } }
            });
            if let Some(group) = group {
                schema["x-docs"]["group"] = json!(group);
            }
            schema
        };
        let mut doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Petstore", "version": "1.0.0" },
            "tags": tags,
            "paths": {
                "/pets": { "get": operation("listPets", "List pets", &["pets"]) },
                "/store/inventory": { "get": operation("getInventory", "Retrieve inventory", &["store"]) },
                "/users": { "get": operation("listUsers", "List users", &["users"]) },
                "/audit": { "get": operation("listAuditEvents", "List audit events", &["audit"]) }
            },
            "components": { "schemas": {
                "Money": object("money", None),
                "Pet": object("pet", Some("pets")),
                "Report": object("report", Some("reports"))
            } }
        });
        if let Value::Object(root) = root {
            for (k, v) in root {
                doc[k] = v;
            }
        }
        doc
    }

    /// The fused flow with both finishing passes: [`sidebar_for`], then this
    /// pass over the preprocessed document.
    fn grouped_for(doc: &Value, options: &Options, store: bool) -> Vec<Value> {
        let mut sidebar = sidebar_for(doc, options, store);
        let (processed, _) = DocCtx::preprocess(doc);
        group_by_tag_groups(&mut sidebar, &processed);
        sidebar
    }

    fn with_tag_groups(groups: Value) -> Value {
        petstore_doc(&[], json!({ TAG_GROUPS_KEY: groups }))
    }

    /// The group names of a level's group entries, in order.
    fn names(level: &[Value]) -> Vec<&str> {
        level
            .iter()
            .filter_map(|e| e.get("group").and_then(|g| g.as_str()))
            .collect()
    }

    fn pages(entry: &Value) -> &[Value] {
        entry["pages"].as_array().expect("a group's pages")
    }

    /// The baseline every case is read against: four area groups in the tag
    /// order, then the collapsed objects group with its leaf and its area
    /// sub-groups.
    #[test]
    fn the_petstore_baseline() {
        let sidebar = sidebar_for(&petstore_doc(&[], json!({})), &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["pets", "store", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        let objects = entry(&sidebar, DEFAULT_OBJECTS_GROUP);
        assert_eq!(objects["pages"][0]["page"], "docs/api/objects/Money");
        assert_eq!(names(pages(objects)), vec!["pets", "reports"]);
    }

    /// Two sections: each takes the place of its first member, its members
    /// follow the tag group's order — not the navigation's — and move whole;
    /// the objects group stays last, still the only collapsed entry; a
    /// section is a plain `{group, pages}` entry.
    #[test]
    fn two_sections_take_the_place_of_their_first_member() {
        let doc = with_tag_groups(json!([
            { "name": "Core", "tags": ["store", "pets"] },
            { "name": "Admin", "tags": ["audit", "users"] }
        ]));
        let base = sidebar_for(&doc, &Options::default(), false);
        let sidebar = grouped_for(&doc, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "Admin", DEFAULT_OBJECTS_GROUP]
        );
        let core = entry(&sidebar, "Core");
        assert_eq!(keys(core), vec!["group", "pages"]);
        assert_eq!(names(pages(core)), vec!["store", "pets"]);
        assert_eq!(pages(core)[0], *entry(&base, "store"));
        assert_eq!(pages(core)[1], *entry(&base, "pets"));
        assert_eq!(
            names(pages(entry(&sidebar, "Admin"))),
            vec!["audit", "users"]
        );
        assert_eq!(
            *entry(&sidebar, DEFAULT_OBJECTS_GROUP),
            *entry(&base, DEFAULT_OBJECTS_GROUP)
        );
        assert_eq!(flags_in(&sidebar), 1);
    }

    /// An entry no tag group lists keeps its place — before a section and
    /// after one alike; nothing is hidden.
    #[test]
    fn an_ungrouped_entry_keeps_its_place() {
        let doc = with_tag_groups(json!([{ "name": "Core", "tags": ["store", "users"] }]));
        let sidebar = grouped_for(&doc, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["pets", "Core", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(
            names(pages(entry(&sidebar, "Core"))),
            vec!["store", "users"]
        );
    }

    /// A listed name without a top-level entry adds nothing — `reports` has
    /// object pages only, a sub-group of the objects group, which stays
    /// there — and a tag group without members emits no section.
    #[test]
    fn a_listed_name_without_an_entry_adds_nothing() {
        let doc = with_tag_groups(json!([
            { "name": "Core", "tags": ["pets", "reports", "billing"] },
            { "name": "Admin", "tags": ["billing"] }
        ]));
        let sidebar = grouped_for(&doc, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "store", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(names(pages(entry(&sidebar, "Core"))), vec!["pets"]);
        assert_eq!(
            names(pages(entry(&sidebar, DEFAULT_OBJECTS_GROUP))),
            vec!["pets", "reports"]
        );
    }

    /// A name two tag groups list goes to the first; a tag group repeating an
    /// earlier one's name is skipped, so what only it lists keeps its place.
    #[test]
    fn the_first_tag_group_listing_a_name_wins() {
        let doc = with_tag_groups(json!([
            { "name": "Core", "tags": ["pets", "store"] },
            { "name": "Admin", "tags": ["store", "users"] },
            { "name": "Core", "tags": ["audit"] }
        ]));
        let sidebar = grouped_for(&doc, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "Admin", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(names(pages(entry(&sidebar, "Core"))), vec!["pets", "store"]);
        assert_eq!(names(pages(entry(&sidebar, "Admin"))), vec!["users"]);
    }

    /// No two top-level groups share a name: a tag group named like an entry
    /// that keeps its place (its name or its display name) is skipped — and so,
    /// in turn, is one named like an entry that the skipped one would have
    /// moved. A tag group named like an entry that moves (into it, or into
    /// another section) is kept.
    #[test]
    fn a_section_never_shares_a_name_with_an_entry_that_stays() {
        let base = sidebar_for(&petstore_doc(&[], json!({})), &Options::default(), false);

        let collides = with_tag_groups(json!([{ "name": "audit", "tags": ["pets", "store"] }]));
        assert_eq!(grouped_for(&collides, &Options::default(), false), base);

        let chained = with_tag_groups(json!([
            { "name": "pets", "tags": ["store"] },
            { "name": "store", "tags": ["users"] }
        ]));
        assert_eq!(grouped_for(&chained, &Options::default(), false), base);

        let into_itself = with_tag_groups(json!([{ "name": "pets", "tags": ["pets", "store"] }]));
        let sidebar = grouped_for(&into_itself, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["pets", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(names(pages(&sidebar[0])), vec!["pets", "store"]);

        let elsewhere = with_tag_groups(json!([
            { "name": "Core", "tags": ["pets"] },
            { "name": "pets", "tags": ["store"] }
        ]));
        let sidebar = grouped_for(&elsewhere, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "pets", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(names(pages(&sidebar[0])), vec!["pets"]);
        assert_eq!(names(pages(&sidebar[1])), vec!["store"]);

        // Named like the display name of an entry that keeps its place: the
        // tag group is skipped, so the navigation is the relabelled baseline.
        let labelled_like = petstore_doc(
            &[("pets", "Core")],
            json!({ TAG_GROUPS_KEY: [{ "name": "Core", "tags": ["store"] }] }),
        );
        let sidebar = grouped_for(&labelled_like, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "store", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(pages(&sidebar[0]), pages(&base[0]));
        assert_eq!(sidebar[1], base[1]);
        let labels_only = petstore_doc(&[("pets", "Core")], json!({}));
        assert_eq!(
            sidebar,
            grouped_for(&labels_only, &Options::default(), false)
        );
    }

    /// Display names label every generated group at any depth — the areas in
    /// a section, an area that keeps its place, the objects group's area
    /// sub-groups (depth 2, `reports` included) — while a tag without one
    /// keeps its name, a section's own name is never relabelled, the page
    /// entries and the objects group's key order are untouched.
    #[test]
    fn display_names_label_the_groups_at_any_depth() {
        let display_names = [("pets", "Pets"), ("store", "Store"), ("reports", "Reports")];
        let doc = petstore_doc(
            &display_names,
            json!({ TAG_GROUPS_KEY: [{ "name": "Core", "tags": ["pets", "store"] }] }),
        );
        let base = sidebar_for(&doc, &Options::default(), false);
        let sidebar = grouped_for(&doc, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        let core = entry(&sidebar, "Core");
        assert_eq!(names(pages(core)), vec!["Pets", "Store"]);
        assert_eq!(pages(&pages(core)[0]), pages(entry(&base, "pets")));
        let objects = entry(&sidebar, DEFAULT_OBJECTS_GROUP);
        assert_eq!(keys(objects), vec!["group", COLLAPSED_KEY, "pages"]);
        assert_eq!(names(pages(objects)), vec!["Pets", "Reports"]);
        assert_eq!(
            pages(objects)[0],
            pages(entry(&base, DEFAULT_OBJECTS_GROUP))[0]
        );

        // Display names alone: the same navigation, relabelled.
        let labels_only = petstore_doc(&display_names, json!({}));
        let sidebar = grouped_for(&labels_only, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Pets", "Store", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );

        // A section named like a display-named tag keeps its own name.
        let named_like_a_tag = petstore_doc(
            &display_names,
            json!({ TAG_GROUPS_KEY: [{ "name": "pets", "tags": ["pets"] }] }),
        );
        let sidebar = grouped_for(&named_like_a_tag, &Options::default(), false);
        assert_eq!(sidebar[0]["group"], "pets");
        assert_eq!(names(pages(&sidebar[0])), vec!["Pets"]);

        // The first tag of a name decides, with or without a display name.
        let mut repeated = petstore_doc(&[("pets", "Pets")], json!({}));
        let tags = repeated["tags"].as_array_mut().unwrap();
        tags.push(json!({ "name": "pets", DISPLAY_NAME_KEY: "Animals" }));
        tags.push(json!({ "name": "store", DISPLAY_NAME_KEY: "Shop" }));
        let sidebar = grouped_for(&repeated, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Pets", "store", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
    }

    /// Under the per-area layout an area carries its own objects sub-group
    /// (`[area, Objects]`): it moves into the section whole, and an area with
    /// object pages only (`reports`) moves too — it is a top-level entry here.
    #[test]
    fn the_per_area_layout_moves_an_area_whole() {
        let options = Options {
            objects_layout: Some("per-area".into()),
            ..Default::default()
        };
        let doc = with_tag_groups(json!([{ "name": "Core", "tags": ["pets", "reports"] }]));
        let base = sidebar_for(&doc, &options, false);
        assert_eq!(
            names(pages(entry(&base, "pets"))),
            vec![DEFAULT_OBJECTS_GROUP]
        );
        let sidebar = grouped_for(&doc, &options, false);
        let expected: Vec<&str> = names(&base)
            .into_iter()
            .filter(|n| *n != "reports")
            .map(|n| if n == "pets" { "Core" } else { n })
            .collect();
        assert_eq!(names(&sidebar), expected);
        let core = entry(&sidebar, "Core");
        assert_eq!(
            pages(core),
            &[
                entry(&base, "pets").clone(),
                entry(&base, "reports").clone()
            ]
        );
        assert_eq!(flags_in(&sidebar), 0);
    }

    /// Store mode: the page paths inside the moved, relabelled groups are
    /// untouched.
    #[test]
    fn store_mode_page_paths_are_untouched() {
        let doc = petstore_doc(
            &[("pets", "Pets")],
            json!({ TAG_GROUPS_KEY: [{ "name": "Core", "tags": ["pets"] }] }),
        );
        let sidebar = grouped_for(&doc, &Options::default(), true);
        let core = entry(&sidebar, "Core");
        assert_eq!(names(pages(core)), vec!["Pets"]);
        assert_eq!(pages(&pages(core)[0]), &[json!("docs/api/listPets")]);
    }

    /// Malformed tag groups — not an object, no string name, an empty name, no
    /// `tags` array — are skipped, and so are the non-string and empty names
    /// in `tags`; a well-formed one beside them still applies.
    #[test]
    fn malformed_tag_groups_are_skipped() {
        let doc = with_tag_groups(json!([
            42,
            { "name": "Admin" },
            { "tags": ["users"] },
            { "name": "", "tags": ["users"] },
            { "name": 7, "tags": ["users"] },
            { "name": "Ops", "tags": "audit" },
            { "name": "Core", "tags": ["store", 7, "", null, "pets"] }
        ]));
        let sidebar = grouped_for(&doc, &Options::default(), false);
        assert_eq!(
            names(&sidebar),
            vec!["Core", "users", "audit", DEFAULT_OBJECTS_GROUP]
        );
        assert_eq!(names(pages(entry(&sidebar, "Core"))), vec!["store", "pets"]);
    }

    /// Without a well-formed tag group or a non-empty string display name the
    /// pass changes nothing.
    #[test]
    fn no_extension_is_a_no_op() {
        let base = sidebar_for(&petstore_doc(&[], json!({})), &Options::default(), false);
        assert_eq!(
            grouped_for(&petstore_doc(&[], json!({})), &Options::default(), false),
            base
        );
        let mut malformed = petstore_doc(
            &[("pets", "")],
            json!({ TAG_GROUPS_KEY: [42, { "name": "Core" }, { "tags": ["pets"] }] }),
        );
        malformed["tags"][1][DISPLAY_NAME_KEY] = json!(7);
        assert_eq!(grouped_for(&malformed, &Options::default(), false), base);
        let not_a_list = with_tag_groups(json!({ "name": "Core", "tags": ["pets"] }));
        assert_eq!(grouped_for(&not_a_list, &Options::default(), false), base);
    }

    /// Sidebar mode is the author's navigation: tag groups and display names
    /// change nothing there.
    #[test]
    fn sidebar_mode_is_untouched() {
        let doc = petstore_doc(
            &[("pets", "Pets")],
            json!({
                TAG_GROUPS_KEY: [{ "name": "Core", "tags": ["pets", "store"] }],
                "x-docs": { "sidebar": [
                    { "group": "pets", "pages": [{ "type": "endpoint", "key": "listPets" }] },
                    { "group": "store", "pages": [{ "type": "endpoint", "key": "getInventory" }] }
                ] }
            }),
        );
        let base = sidebar_for(&doc, &Options::default(), false);
        assert_eq!(names(&base), vec!["pets", "store"]);
        assert_eq!(grouped_for(&doc, &Options::default(), false), base);
    }
}
