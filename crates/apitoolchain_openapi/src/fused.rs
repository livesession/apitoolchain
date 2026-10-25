//! The fused uniform endpoint (S6+ W3 tail): ONE call takes an OpenAPI spec
//! source and returns everything plugin-docs' `uniformResolver` needs to emit
//! virtual pages + sidebar — conversion, the x-docs sidebar plugin, and
//! pluginNavigation all run natively; references never materialize in JS and
//! the endpoint code-sample post-pass (irrelevant to pages) never runs at
//! boot. The JS side keeps: the urlPrefix sidebar-route match (input),
//! compose-file merging, gray-matter frontmatter stringify + fs writes
//! (byte-owned by JS), and sidebar wiring.

use serde_json::{json, Map, Value};

use crate::doc::DocCtx;
use crate::xdocs::{apply_xdocs_sidebar_with_options, get_xdocs_route};
use crate::{oap_schema_to_references, read_spec, OasError, Options};
use apitoolchain_uniform::jsrt::node_path_join;
use apitoolchain_uniform::plugins::plugin_navigation;

#[derive(Debug, Clone, Default)]
pub struct FusedInput {
    pub source: String,
    /// The sidebar-route-matched urlPrefix (JS computed; "" when none).
    pub url_prefix: String,
    pub match_route: String,
    pub options_url_prefix: String,
    /// settings.engine.uniform.store
    pub store: bool,
    /// The component-schema pages' group name (the converter option
    /// `objectsGroup`): the top-level objects group and, under the per-area
    /// layout, the sub-group under a resource group. `None` →
    /// [`crate::DEFAULT_OBJECTS_GROUP`].
    pub objects_group: Option<String>,
    /// The objects layout (the converter option `objectsLayout`: `"grouped"`
    /// or `"per-area"`). `None` → [`crate::DEFAULT_OBJECTS_LAYOUT`].
    pub objects_layout: Option<String>,
    /// How a page without an `x-docs.path` gets its canonical (the converter
    /// option `operationPaths`: `"operation-id"` or `"title"`). `None` →
    /// [`crate::DEFAULT_OPERATION_PATHS`].
    pub operation_paths: Option<String>,
}

pub struct FusedOutput {
    pub url_prefix: String,
    /// The effective matchRoute after the x-docs.route fileRouting step.
    pub match_route: String,
    /// JS must `sidebar.push({route: match_route, pages: []})` when true.
    pub new_route_pushed: bool,
    pub sidebar: Vec<Value>,
    pub page_front_matter: Map<String, Value>,
    /// One entry per (post-x-docs) reference, in emit order:
    /// `{pagePath, region}` — JS builds frontmatter + writes the .md.
    pub pages: Vec<Value>,
}

/// Mirrors uniformResolver's flow for `uniformType === "openapi"`:
/// resolve refs → fileRouting/urlPrefix decision → x-docs plugin →
/// pluginNavigation → per-ref page entries.
pub fn uniform_oas_pages(input: &FusedInput) -> Result<FusedOutput, OasError> {
    let raw = read_spec(&input.source)?;
    let options = Options {
        regions: None,
        objects_group: input.objects_group.clone(),
        objects_layout: input.objects_layout.clone(),
        operation_paths: input.operation_paths.clone(),
    };

    // Full conversion (endpoint examples are a JS post-pass the page flow
    // never needs; component-schema examples are part of conversion).
    let refs = oap_schema_to_references(&raw, Some(options.clone()));
    let mut refs: Vec<Value> = refs
        .iter()
        .map(|r| serde_json::to_value(r).expect("Reference serializes"))
        .collect();

    // x-docs machinery resolves against the (preprocessed) doc.
    let (doc, stamps) = DocCtx::preprocess(&raw);
    let ctx = DocCtx::with_merged_stamps(&doc, &stamps);

    // urlPrefix decision — the exact uniformResolver order:
    //   matched sidebar route (input) → fileRouting (x-docs.route) →
    //   matchRoute (push new route) → options.urlPrefix.
    let mut url_prefix = input.url_prefix.clone();
    let mut match_route = input.match_route.clone();
    let mut new_route_pushed = false;
    if url_prefix.is_empty() {
        if let Some(route) = get_xdocs_route(&raw) {
            if !route.is_empty() {
                match_route = route;
            }
        }
    }
    if url_prefix.is_empty() && !match_route.is_empty() {
        new_route_pushed = true;
        url_prefix = match_route.clone();
    }
    if url_prefix.is_empty() && !input.options_url_prefix.is_empty() {
        url_prefix = input.options_url_prefix.clone();
    }

    // Plugin order matches `uniform(refs, {plugins: [xdocs, navigation]})`:
    // the x-docs defer may REPLACE the ref list before navigation sees it.
    // Two pages at one path (`x-docs.path`, or two summaries that derive the
    // same canonical) are an error here, naming both.
    apply_xdocs_sidebar_with_options(&ctx, &doc, &mut refs, &options)?;

    let settings_stub = json!({"engine": {"uniform": {"store": input.store}}});
    let nav = plugin_navigation(&settings_stub, &url_prefix, &refs)
        .map_err(|e| OasError(format!("pluginNavigation: {e}")))?;

    // Per-ref page entries — pagePath mirrors `path.join(urlPrefix,
    // ref.canonical)`; region mirrors the resolver's openapi switch.
    let pages: Vec<Value> = refs
        .iter()
        .map(|r| {
            let canonical = r.get("canonical").and_then(|c| c.as_str()).unwrap_or("");
            let page_path = node_path_join(&[&url_prefix, canonical]);

            let ctx_v = r.get("context");
            let method = ctx_v
                .and_then(|c| c.get("method"))
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let path = ctx_v
                .and_then(|c| c.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let component_schema = ctx_v
                .and_then(|c| c.get("componentSchema"))
                .and_then(|s| s.as_str())
                .unwrap_or("");

            let region = if !method.is_empty() && !path.is_empty() {
                format!("{} {}", method.to_uppercase(), path)
            } else if !component_schema.is_empty() {
                format!("/components/schemas/{component_schema}")
            } else {
                String::new()
            };

            json!({ "pagePath": page_path, "region": region })
        })
        .collect();

    Ok(FusedOutput {
        url_prefix,
        match_route,
        new_route_pushed,
        sidebar: nav.sidebar,
        page_front_matter: nav.page_front_matter,
        pages,
    })
}
