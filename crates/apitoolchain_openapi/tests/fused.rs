//! Tier-1 parity for the fused uniform endpoint: {sidebar, pageFrontMatter,
//! pages} against fused.golden.json — goldens generated FROM THE JS IMPLS
//! (XYD_NATIVE=0 bun scripts/build-fused-goldens.ts in packages/openapi).
//! 5.xdocs.sidebar exercises the ported x-docs sidebar plugin end-to-end
//! (canonical rewrites, group paths, endpoint + object pages); 5.xdocs.objects
//! its per-schema half outside sidebar mode (`x-docs.name` / `.group` /
//! `.hidden` on component schemas, `hidden` on an operation) under the
//! per-area objects layout; 11.objects-layouts the default, grouped layout
//! and its ordering rule; 12.page-paths the explicit `x-docs.path` and the
//! title-derived canonicals (`operationPaths: title`), the links and the
//! sidebar following the final paths. A case's converter options are the
//! ones scripts/build-fused-goldens.ts lists for it.

use apitoolchain_openapi::fused::{uniform_oas_pages, FusedInput};
use apitoolchain_uniform::canon;
use serde_json::{json, Value};

fn run_case(name: &str, objects_layout: Option<&str>, operation_paths: Option<&str>) {
    let fixtures =
        apitoolchain_parity::fixtures_dir(env!("CARGO_MANIFEST_DIR"), "apitoolchain-openapi");
    let case = fixtures.join(name);
    let golden_path = case.join("fused.golden.json");
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    let url_prefix = golden.get("urlPrefix").and_then(|u| u.as_str()).unwrap();

    let out = uniform_oas_pages(&FusedInput {
        source: case.join("input.yaml").to_string_lossy().to_string(),
        url_prefix: url_prefix.to_string(),
        objects_layout: objects_layout.map(String::from),
        operation_paths: operation_paths.map(String::from),
        ..Default::default()
    })
    .unwrap_or_else(|e| panic!("{name}: fused failed: {e}"));

    let actual = json!({
        "urlPrefix": out.url_prefix,
        "sidebar": out.sidebar,
        "pageFrontMatter": out.page_front_matter,
        "pages": out.pages,
    });

    if std::env::var("XYD_PARITY_DUMP").as_deref() == Ok("1") {
        std::fs::write(
            case.join("fused.rust.json"),
            serde_json::to_string_pretty(&actual).unwrap(),
        )
        .unwrap();
    }

    let diffs = canon::diff_paths(&actual, &golden, 12);
    assert!(
        diffs.is_empty(),
        "{name}: FUSED PARITY FAILED — first divergences:\n{}",
        diffs
            .iter()
            .map(|(p, a, b)| format!("  at {p}\n    rust:   {a}\n    oracle: {b}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

macro_rules! fused_case {
    ($test:ident, $name:literal) => {
        fused_case!($test, $name, None, None);
    };
    ($test:ident, $name:literal, $objects_layout:expr) => {
        fused_case!($test, $name, $objects_layout, None);
    };
    ($test:ident, $name:literal, $objects_layout:expr, $operation_paths:expr) => {
        #[test]
        fn $test() {
            run_case($name, $objects_layout, $operation_paths);
        }
    };
}

fused_case!(basic, "1.basic");
fused_case!(more, "2.more");
fused_case!(multiple_responses, "3.multiple-responses");
fused_case!(xdocs_sidebar, "5.xdocs.sidebar");
fused_case!(enums, "8.enums");
fused_case!(xdocs_objects, "5.xdocs.objects", Some("per-area"));
fused_case!(objects_layouts, "11.objects-layouts");
fused_case!(page_paths, "12.page-paths", None, Some("title"));

/// The group names of a sidebar level, nested groups as `(name, pages)`.
fn group_names(level: &[Value]) -> Vec<String> {
    level
        .iter()
        .filter_map(|p| p.get("group").and_then(|g| g.as_str()).map(String::from))
        .collect()
}

fn pages_of<'a>(level: &'a [Value], group: &str) -> &'a [Value] {
    level
        .iter()
        .find(|p| p.get("group").and_then(|g| g.as_str()) == Some(group))
        .and_then(|p| p.get("pages"))
        .and_then(|p| p.as_array())
        .map(|p| p.as_slice())
        .unwrap_or_else(|| panic!("no group {group}"))
}

fn xdocs_objects_sidebar(objects_group: Option<&str>, objects_layout: Option<&str>) -> Vec<Value> {
    let fixtures =
        apitoolchain_parity::fixtures_dir(env!("CARGO_MANIFEST_DIR"), "apitoolchain-openapi");
    uniform_oas_pages(&FusedInput {
        source: fixtures
            .join("5.xdocs.objects")
            .join("input.yaml")
            .to_string_lossy()
            .to_string(),
        url_prefix: "docs/api".into(),
        objects_group: objects_group.map(String::from),
        objects_layout: objects_layout.map(String::from),
        ..Default::default()
    })
    .expect("fused ok")
    .sidebar
}

/// The default objects layout (`grouped`, `FusedInput::objects_layout`
/// unset): ONE "Objects" group, the LAST top-level group, holding the page
/// without a resource group first and then one sub-group per resource group
/// in the order the groups appear above it — `widgets` (which has operations)
/// before `parts` (which has none). The sub-groups are nested groups, which a
/// renderer folds by default.
#[test]
fn grouped_layout_is_one_objects_group_last() {
    let sidebar = xdocs_objects_sidebar(None, None);
    assert_eq!(group_names(&sidebar), vec!["widgets", "Objects"]);
    let widgets = pages_of(&sidebar, "widgets");
    assert_eq!(widgets.len(), 3, "{widgets:?}");
    assert!(widgets.iter().all(|p| p.get("group").is_none()));
    let objects = pages_of(&sidebar, "Objects");
    assert_eq!(objects.len(), 3, "{objects:?}");
    assert_eq!(objects[0]["page"], "docs/api/objects/PartBase");
    assert_eq!(objects[1]["group"], "widgets");
    assert_eq!(objects[1]["pages"][0]["page"], "docs/api/objects/Widget");
    assert_eq!(objects[2]["group"], "parts");
    assert_eq!(objects[2]["pages"][0]["page"], "docs/api/objects/Part");
    // The explicit name selects the same layout.
    assert_eq!(xdocs_objects_sidebar(None, Some("grouped")), sidebar);
}

/// `objectsLayout: per-area`: each object page with a resource group is
/// nested under that group's own "Objects" sub-group, AFTER the group's
/// operations; the schema without one keeps the top-level "Objects" group,
/// last.
#[test]
fn per_area_layout_nests_the_object_pages_inside_their_groups() {
    let sidebar = xdocs_objects_sidebar(None, Some("per-area"));
    assert_eq!(group_names(&sidebar), vec!["widgets", "parts", "Objects"]);
    let widgets = pages_of(&sidebar, "widgets");
    assert_eq!(widgets.len(), 4, "{widgets:?}");
    assert!(widgets[..3].iter().all(|p| p.get("group").is_none()));
    assert_eq!(widgets[3]["group"], "Objects");
    assert_eq!(widgets[3]["pages"][0]["page"], "docs/api/objects/Widget");
    assert_eq!(pages_of(&sidebar, "parts")[0]["group"], "Objects");
    assert_eq!(
        pages_of(&sidebar, "Objects")[0]["page"],
        "docs/api/objects/PartBase"
    );
}

/// The `objectsGroup` option (`FusedInput::objects_group`) renames the
/// objects group in both layouts: the top-level group under `grouped`, and
/// under `per-area` the top-level group of the schemas without a resource
/// group AND the sub-group an object page with one is nested under — which
/// comes AFTER the group's operations.
#[test]
fn objects_group_option_renames_the_object_groups() {
    let grouped = xdocs_objects_sidebar(Some("Types"), None);
    assert_eq!(group_names(&grouped), vec!["widgets", "Types"]);
    assert_eq!(pages_of(&grouped, "Types")[1]["group"], "widgets");
    assert!(
        !serde_json::to_string(&grouped).unwrap().contains("Objects"),
        "the default name leaked: {grouped:?}"
    );

    let out = xdocs_objects_sidebar(Some("Types"), Some("per-area"));
    assert_eq!(group_names(&out), vec!["widgets", "parts", "Types"]);
    let widgets = pages_of(&out, "widgets");
    // Three operations first, then the sub-group.
    assert_eq!(widgets.len(), 4, "{widgets:?}");
    assert!(widgets[..3].iter().all(|p| p.get("group").is_none()));
    assert_eq!(widgets[3]["group"], "Types");
    assert_eq!(widgets[3]["pages"][0]["page"], "docs/api/objects/Widget");
    assert_eq!(pages_of(&out, "parts")[0]["group"], "Types");
    assert_eq!(
        pages_of(&out, "Types")[0]["page"],
        "docs/api/objects/PartBase"
    );
    assert!(
        !serde_json::to_string(&out).unwrap().contains("Objects"),
        "the default name leaked: {out:?}"
    );
}

/// The ordering rule on 11.objects-layouts (the fused golden pins the whole
/// shape; this spells the rule out): the "Objects" group is the LAST
/// top-level group, after the declared tags, the undeclared one, an
/// `x-docs.group` and the nameless group of an untagged operation; inside it
/// the pages without a resource group come first, then the sub-groups in the
/// order of the groups above, then a group with no operations.
#[test]
fn grouped_layout_puts_the_objects_group_last_whatever_the_tag_order() {
    let fixtures =
        apitoolchain_parity::fixtures_dir(env!("CARGO_MANIFEST_DIR"), "apitoolchain-openapi");
    let out = uniform_oas_pages(&FusedInput {
        source: fixtures
            .join("11.objects-layouts")
            .join("input.yaml")
            .to_string_lossy()
            .to_string(),
        url_prefix: "docs/api".into(),
        ..Default::default()
    })
    .expect("fused ok");
    assert_eq!(
        group_names(&out.sidebar),
        vec!["parts", "widgets", "audit", "assemblies", "", "Objects"]
    );
    let objects = pages_of(&out.sidebar, "Objects");
    assert_eq!(objects[0]["page"], "docs/api/objects/Status");
    assert_eq!(objects[1]["page"], "docs/api/objects/Address");
    assert_eq!(
        group_names(&objects[2..]),
        vec!["parts", "widgets", "assemblies", "fasteners"]
    );
    assert!(objects[2..]
        .iter()
        .all(|g| g["pages"].as_array().unwrap().len() == 1));
    // The hidden schemas have no page.
    assert!(!out
        .page_front_matter
        .contains_key("docs/api/objects/WidgetList"));
    assert!(!out.page_front_matter.contains_key("docs/api/objects/Error"));
}
