//! OpenAPI 3.x → Uniform `Reference[]` — the Rust port of
//! `packages/openapi` (S6+ W2). Endpoint code-sample generation
//! (@readme/oas-to-snippet) intentionally stays a JS post-pass in the shim;
//! everything else — deref semantics, schema conversion, parameters/request/
//! response definitions, component schemas incl. their JSON examples, tag
//! sorting — is here. The frozen fixtures are the spec.

mod components;
pub mod core;
pub mod fused;
mod links;
mod paths;
mod util;
pub mod xdocs;

use apitoolchain_uniform::{Reference, ReferenceType};
use serde::Deserialize;
use serde_json::Value;

// `oas_doc` is the extracted leaf (DocCtx + spec loading) shared with the
// opensdk repo. Aliasing it as `doc` keeps every internal `crate::doc::DocCtx`
// resolving unchanged, and the re-export keeps `apitoolchain_openapi::DocCtx` — the
// path five call sites outside this crate already use — working verbatim.
pub use links::ConvCtx;
use oas_doc as doc;
pub use oas_doc::{parse_spec, read_spec, DocCtx, OasError};

const SUPPORTED_HTTP_METHODS: &[&str] = &["get", "put", "patch", "post", "delete"];

/// The default name of the group the component-schema pages sit in: the
/// top-level objects group, and under the per-area layout the sub-group
/// nested under a resource group (`x-docs.group`) for the schemas that have
/// one. Renamed with [`Options::objects_group`].
pub const DEFAULT_OBJECTS_GROUP: &str = "Objects";

/// Where the object pages (the component schemas) sit in the generated
/// navigation — [`Options::objects_layout`] (JSON `objectsLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectsLayout {
    /// `"grouped"` (the default): ONE top-level objects group, the last group
    /// of the navigation — the pages without a resource group first, then one
    /// sub-group per resource group (`x-docs.group`) in the order the groups
    /// appear above it (`context.group = [<objects group>, <group>]`; a
    /// renderer that starts nested groups collapsed shows them folded).
    #[default]
    Grouped,
    /// `"per-area"`: an object page with a resource group is nested under that
    /// group's own objects sub-group, after its operations
    /// (`context.group = [<group>, <objects group>]`); the pages without one
    /// keep the top-level objects group.
    PerArea,
}

impl ObjectsLayout {
    /// The JSON name of the layout.
    pub fn as_str(self) -> &'static str {
        match self {
            ObjectsLayout::Grouped => "grouped",
            ObjectsLayout::PerArea => "per-area",
        }
    }

    /// The layout a JSON name selects — anything but `"per-area"` is the
    /// default, as in the JS twin.
    pub fn parse(name: Option<&str>) -> Self {
        match name {
            Some("per-area") => ObjectsLayout::PerArea,
            _ => ObjectsLayout::Grouped,
        }
    }
}

/// The default [`ObjectsLayout`].
pub const DEFAULT_OBJECTS_LAYOUT: ObjectsLayout = ObjectsLayout::Grouped;

/// Port of `uniformOasOptions`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    /// Format: `"METHOD /path"` (e.g. `"GET /users"`) or
    /// `"/components/schemas/Name"`.
    pub regions: Option<Vec<String>>,
    /// The name of the component-schema pages' group (JSON: `objectsGroup`) —
    /// the top-level objects group and, under the per-area layout, the
    /// sub-group under a resource group. `None` / empty →
    /// [`DEFAULT_OBJECTS_GROUP`].
    pub objects_group: Option<String>,
    /// Where an object page with a resource group goes (JSON: `objectsLayout`,
    /// `"grouped"` or `"per-area"`); anything else → [`DEFAULT_OBJECTS_LAYOUT`].
    /// See [`ObjectsLayout`].
    pub objects_layout: Option<String>,
}

impl Options {
    /// The effective name of the component-schema pages' group.
    pub fn objects_group(&self) -> &str {
        match self.objects_group.as_deref() {
            Some(name) if !name.is_empty() => name,
            _ => DEFAULT_OBJECTS_GROUP,
        }
    }

    /// The effective objects layout.
    pub fn objects_layout(&self) -> ObjectsLayout {
        ObjectsLayout::parse(self.objects_layout.as_deref())
    }
}

/// Port of `oapSchemaToReferences` (endpoint `examples.groups` left EMPTY —
/// the shim's JS post-pass fills them; component-schema examples are emitted).
pub fn oap_schema_to_references(raw: &Value, options: Option<Options>) -> Vec<Reference> {
    if raw.is_null() {
        return vec![];
    }
    let options = options.unwrap_or_default();
    // Materialize $ref-with-siblings merges first (v12 sibling semantics), then
    // build the lazy-resolution context over the processed doc.
    let (processed, merged_stamps) = DocCtx::preprocess(raw);
    let raw = &processed;
    let ctx = ConvCtx::new(DocCtx::with_merged_stamps(raw, &merged_stamps));
    let mut references: Vec<Reference> = Vec::new();

    // Mutable servers accumulation quirk: starts from doc servers; path-level
    // servers get appended (dupe-guarded) as paths are processed, and the
    // ctx.servers fallback for every endpoint reads the CURRENT list.
    let mut servers: Vec<String> = raw
        .get("servers")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.get("url").and_then(|u| u.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let first_server = servers.first().cloned().unwrap_or_default();

    let regions = options.regions.clone().unwrap_or_default();

    let paths_obj = raw.get("paths").and_then(|p| p.as_object());
    for (endpoint_path, oap_path) in paths_obj.into_iter().flatten() {
        let oap_path = ctx.resolve(oap_path);
        for method in SUPPORTED_HTTP_METHODS {
            // The JS pushes path servers inside the METHOD loop (dupe-guarded).
            if let Some(path_servers) = oap_path.get("servers").and_then(|s| s.as_array()) {
                for ps in path_servers {
                    if let Some(url) = ps.get("url").and_then(|u| u.as_str()) {
                        if !servers.iter().any(|s| s == url) {
                            servers.push(url.to_string());
                        }
                    }
                }
            }

            if !regions.is_empty() {
                let region_key = format!("{} {endpoint_path}", method.to_uppercase());
                if !regions.contains(&region_key) {
                    continue;
                }
            }

            let Some(mut reference) =
                paths::oap_path_to_reference(&ctx, method, endpoint_path, oap_path)
            else {
                continue;
            };

            // Orchestrator context mutations (oas-schema.ts):
            if let Some(Value::Object(c)) = reference.context.as_mut() {
                // ctx.path OVERWRITTEN with the RAW path (undoing the
                // encodeURIComponent from oapPathToReference).
                c.insert("path".into(), Value::String(endpoint_path.clone()));

                if !c.contains_key("fullPath") && !first_server.is_empty() {
                    if let Some(full) = util::join_url_pathname(&first_server, endpoint_path) {
                        c.insert("fullPath".into(), Value::String(full));
                    }
                }

                let has_servers = c
                    .get("servers")
                    .and_then(|s| s.as_array())
                    .map(|a| !a.is_empty())
                    .unwrap_or(false);
                if !has_servers && !servers.is_empty() {
                    c.insert(
                        "servers".into(),
                        Value::Array(servers.iter().cloned().map(Value::String).collect()),
                    );
                }

                // scopes: global oauth2 security + per-method security.
                let mut scopes: Vec<String> = Vec::new();
                if let Some(sec) = raw.get("security").and_then(|s| s.as_array()) {
                    for security in sec {
                        if let Some(obj) = security.as_object() {
                            for (key, val) in obj {
                                if key == "oauth2" || key == "OAuth2" {
                                    if let Some(list) = val.as_array() {
                                        scopes.extend(
                                            list.iter()
                                                .filter_map(|v| v.as_str().map(String::from)),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                let oap_method = ctx.resolve(oap_path.get(*method).unwrap_or(&Value::Null));
                if let Some(method_sec) = oap_method.get("security") {
                    if let Some(list) = method_sec.as_array() {
                        if list.is_empty() {
                            scopes.clear();
                        }
                        for security in list {
                            if let Some(obj) = security.as_object() {
                                for (key, val) in obj {
                                    let scheme = raw
                                        .get("components")
                                        .and_then(|c| c.get("securitySchemes"))
                                        .and_then(|s| s.get(key));
                                    let is_oauth2 =
                                        scheme.and_then(|s| s.get("type")).and_then(|t| t.as_str())
                                            == Some("oauth2");
                                    if is_oauth2 {
                                        if let Some(vals) = val.as_array() {
                                            scopes.extend(
                                                vals.iter()
                                                    .filter_map(|v| v.as_str().map(String::from)),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                c.insert(
                    "scopes".into(),
                    Value::Array(scopes.into_iter().map(Value::String).collect()),
                );
            }

            references.push(reference);
        }
    }

    references.extend(components::schema_components_to_references(&ctx, &options));

    let tags = get_tags(raw);
    sort_references_by_tags(&mut references, &tags);

    references
}

/// Convenience: file path → references.
pub fn oap_schema_to_references_from_file(
    path: &str,
    options: Option<Options>,
) -> Result<Vec<Reference>, OasError> {
    let raw = read_spec(path)?;
    Ok(oap_schema_to_references(&raw, options))
}

/// Empirically pinned `oas.getTags()`: unique operation tags; those declared in
/// doc.tags come first (in doc.tags order), the undeclared follow in operation
/// encounter order.
fn get_tags(raw: &Value) -> Vec<String> {
    let mut encountered: Vec<String> = Vec::new();
    if let Some(paths) = raw.get("paths").and_then(|p| p.as_object()) {
        for (_p, item) in paths {
            if let Some(obj) = item.as_object() {
                for (_m, op) in obj {
                    if let Some(tags) = op.get("tags").and_then(|t| t.as_array()) {
                        for t in tags {
                            if let Some(name) = t.as_str() {
                                if !encountered.iter().any(|e| e == name) {
                                    encountered.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let declared: Vec<String> = raw
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut out: Vec<String> = declared
        .into_iter()
        .filter(|d| encountered.iter().any(|e| e == d))
        .collect();
    for e in encountered {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// `sortReferencesByTags` — stable, mirroring the comparator exactly. The
/// object pages (the component schemas) come after every operation whatever
/// the tags — so their group is the last one of the generated navigation,
/// even when an operation is tagged with the objects group's name — and keep
/// the schema order among themselves.
fn sort_references_by_tags(references: &mut [Reference], tags: &[String]) {
    references.sort_by(|prev, next| {
        let prev_object = prev.reference_type == Some(ReferenceType::RestComponentSchema);
        let next_object = next.reference_type == Some(ReferenceType::RestComponentSchema);
        if prev_object != next_object {
            return if prev_object {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }

        let a_tags = group_of(prev);
        let b_tags = group_of(next);

        for tag in tags {
            let a_index = a_tags.iter().position(|t| t == tag);
            let b_index = b_tags.iter().position(|t| t == tag);
            match (a_index, b_index) {
                (Some(a), Some(b)) => return a.cmp(&b),
                (Some(_), None) => return std::cmp::Ordering::Less,
                (None, Some(_)) => return std::cmp::Ordering::Greater,
                (None, None) => {}
            }
        }
        let a0 = a_tags.first().cloned().unwrap_or_default();
        let b0 = b_tags.first().cloned().unwrap_or_default();
        a0.cmp(&b0)
    });
}

fn group_of(r: &Reference) -> Vec<String> {
    r.context
        .as_ref()
        .and_then(|c| c.get("group"))
        .and_then(|g| g.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// An operation tagged with the objects group's name does not pull the
    /// object pages up among the operations: they sort last whatever the tags.
    #[test]
    fn object_pages_sort_after_every_operation() {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Widgets", "version": "1.0.0" },
            "tags": [{ "name": "Objects" }, { "name": "widgets" }],
            "paths": {
                "/objects": { "get": {
                    "operationId": "listObjects", "summary": "List objects",
                    "tags": ["Objects"], "responses": { "200": { "description": "ok" } }
                } },
                "/widgets": { "get": {
                    "operationId": "listWidgets", "summary": "List widgets",
                    "tags": ["widgets"], "responses": { "200": { "description": "ok" } }
                } }
            },
            "components": { "schemas": {
                "Widget": { "type": "object", "properties": { "id": { "type": "string" } } }
            } }
        });
        let refs = oap_schema_to_references(&doc, None);
        let canonicals: Vec<&str> = refs.iter().map(|r| r.canonical.as_str()).collect();
        assert_eq!(
            canonicals,
            vec!["listObjects", "listWidgets", "objects/Widget"]
        );
    }

    /// `objectsLayout` parses leniently, like the JS twin: `"per-area"` or
    /// the default.
    #[test]
    fn objects_layout_option_parses_leniently() {
        assert_eq!(Options::default().objects_layout(), ObjectsLayout::Grouped);
        let per_area = Options {
            objects_layout: Some("per-area".into()),
            ..Default::default()
        };
        assert_eq!(per_area.objects_layout(), ObjectsLayout::PerArea);
        let unknown = Options {
            objects_layout: Some("sideways".into()),
            ..Default::default()
        };
        assert_eq!(unknown.objects_layout(), DEFAULT_OBJECTS_LAYOUT);
        let json: Options =
            serde_json::from_str(r#"{"objectsLayout":"per-area","objectsGroup":"Types"}"#).unwrap();
        assert_eq!(json.objects_layout(), ObjectsLayout::PerArea);
        assert_eq!(json.objects_group(), "Types");
        assert_eq!(ObjectsLayout::PerArea.as_str(), "per-area");
        assert_eq!(ObjectsLayout::parse(Some("grouped")).as_str(), "grouped");
    }
}
