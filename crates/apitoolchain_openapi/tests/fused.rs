//! Tier-1 parity for the fused uniform endpoint: {sidebar, pageFrontMatter,
//! pages} against fused.golden.json — goldens generated FROM THE JS IMPLS
//! (XYD_NATIVE=0 bun scripts/build-fused-goldens.ts in packages/openapi).
//! 5.xdocs.sidebar exercises the ported x-docs sidebar plugin end-to-end
//! (canonical rewrites, group paths, endpoint + object pages); 5.xdocs.objects
//! its per-schema half outside sidebar mode (`x-docs.name` / `.group` /
//! `.hidden` on component schemas, `hidden` on an operation).

use apitoolchain_openapi::fused::{uniform_oas_pages, FusedInput};
use apitoolchain_uniform::canon;
use serde_json::{json, Value};

fn run_case(name: &str) {
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
        #[test]
        fn $test() {
            run_case($name);
        }
    };
}

fused_case!(basic, "1.basic");
fused_case!(more, "2.more");
fused_case!(multiple_responses, "3.multiple-responses");
fused_case!(xdocs_sidebar, "5.xdocs.sidebar");
fused_case!(enums, "8.enums");
fused_case!(xdocs_objects, "5.xdocs.objects");

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

/// The `objectsGroup` option (`FusedInput::objects_group`) renames both the
/// flat top-level group of the schemas without a resource group and the
/// sub-group an object page with one is nested under — which comes AFTER the
/// group's operations.
#[test]
fn objects_group_option_renames_the_object_groups() {
    let fixtures =
        apitoolchain_parity::fixtures_dir(env!("CARGO_MANIFEST_DIR"), "apitoolchain-openapi");
    let out = uniform_oas_pages(&FusedInput {
        source: fixtures
            .join("5.xdocs.objects")
            .join("input.yaml")
            .to_string_lossy()
            .to_string(),
        url_prefix: "docs/api".into(),
        objects_group: Some("Types".into()),
        ..Default::default()
    })
    .expect("fused ok");

    assert_eq!(group_names(&out.sidebar), vec!["widgets", "parts", "Types"]);
    let widgets = pages_of(&out.sidebar, "widgets");
    // Three operations first, then the sub-group.
    assert_eq!(widgets.len(), 4, "{widgets:?}");
    assert!(widgets[..3].iter().all(|p| p.get("group").is_none()));
    assert_eq!(widgets[3]["group"], "Types");
    assert_eq!(widgets[3]["pages"][0]["page"], "docs/api/objects/Widget");
    assert_eq!(pages_of(&out.sidebar, "parts")[0]["group"], "Types");
    assert_eq!(
        pages_of(&out.sidebar, "Types")[0]["page"],
        "docs/api/objects/PartBase"
    );
    assert!(
        !serde_json::to_string(&out.sidebar)
            .unwrap()
            .contains("Objects"),
        "the default name leaked: {:?}",
        out.sidebar
    );
}
