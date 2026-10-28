//! Port of `oas-core.ts` — `schemaObjectToUniformDefinitionProperty`(/-ies/
//! -Meta). Faithful to the letter, including the quirks the fixtures encode:
//! presence-not-truthiness meta checks (`"nullable" in` fires on `false`),
//! the required-override spread, the anyOf branch emitting NO meta, the
//! spread-merge in allOf, visitedRefs clone-at-set snapshots, and the arrayOf
//! store that is immediately overwritten (kept as the final store only).

use apitoolchain_uniform::{property_type, DefinitionProperty, Meta, SymbolDef};
use serde_json::Value;

use crate::links::ConvCtx;

/// TS `visitedRefs: Map<string, DefinitionProperty>` — insertion-ordered isn't
/// needed (lookup only), but values are SNAPSHOTS taken at set time.
pub type VisitedRefs = std::collections::HashMap<String, DefinitionProperty>;

const BUILT_IN_PROPERTIES: &[&str] = &["__internal_getRefPath"];

fn is_built_in_prop(name: &str) -> bool {
    BUILT_IN_PROPERTIES.contains(&name)
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(String::from)
}

fn type_of(v: &Value) -> Option<String> {
    s(v, "type")
}

fn description_of(v: &Value) -> String {
    s(v, "description").unwrap_or_default()
}

fn required_includes(schema: &Value, name: &str) -> bool {
    schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().any(|x| x.as_str() == Some(name)))
        .unwrap_or(false)
}

fn is_merge_type(t: &str) -> bool {
    t == property_type::XOR || t == property_type::UNION
}

fn is_of_type(t: &str) -> bool {
    t == property_type::XOR || t == property_type::UNION || t == property_type::ARRAY
}

/// Port of `schemaObjectToUniformDefinitionProperties`.
/// Returns Ok(array) or Err(single root property) mirroring the TS
/// `DefinitionProperty[] | DefinitionProperty | null` union
/// (None → the TS `null`).
pub enum PropsResult {
    List(Vec<DefinitionProperty>),
    Single(Box<DefinitionProperty>),
    None,
}

/// Every top-level row gets its OWN visited map: the TS passes `visitedRefs`
/// through undefined, so each `schemaObjectToUniformDefinitionProperty` call
/// from here allocates a fresh `Map` and a snapshot never crosses from one row
/// to the next. One map shared by all rows handed the second row that reached
/// a stamped schema the first row's snapshot (`parts` under a union branch
/// came back as the `parts` built under an `allOf` wrapper — children instead
/// of an `ofProperty`).
pub fn schema_object_to_properties(
    ctx: &ConvCtx,
    schema_object: &Value,
    root_property: bool,
) -> PropsResult {
    let schema_object = ctx.resolve(schema_object);
    let mut properties: Vec<DefinitionProperty> = Vec::new();

    let has = |k: &str| schema_object.get(k).map(|v| !v.is_null()).unwrap_or(false);

    if has("anyOf") || has("oneOf") {
        if let Some(property) = schema_object_to_property(
            ctx,
            "",
            schema_object,
            false,
            false,
            &mut VisitedRefs::new(),
            None,
        ) {
            if root_property {
                return PropsResult::Single(Box::new(property));
            }
            properties.push(property);
        }
    } else if has("allOf") {
        let mut component_paths: Vec<String> = Vec::new();
        let all_of = schema_object.get("allOf").and_then(|v| v.as_array());
        for schema in all_of.into_iter().flatten() {
            let schema = ctx.resolve(schema);
            if let Some(rp) = ctx.internal_ref_path(schema) {
                component_paths.extend(rp);
            }
            if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props {
                    if is_built_in_prop(prop_name) {
                        continue;
                    }
                    let prop_schema = ctx.resolve(prop_schema);
                    let required = required_includes(schema, prop_name);
                    let array_of = type_of(prop_schema).as_deref() == Some("array");
                    if let Some(property) = schema_object_to_property(
                        ctx,
                        prop_name,
                        prop_schema,
                        required,
                        array_of,
                        &mut VisitedRefs::new(),
                        None,
                    ) {
                        merge_or_push(&mut properties, prop_name, property);
                    }
                }
            }
        }
        ctx.set_internal_ref_path(schema_object, component_paths);
    } else if let Some(props) = schema_object.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in props {
            if is_built_in_prop(prop_name) {
                continue;
            }
            let prop_schema = ctx.resolve(prop_schema);
            let required = required_includes(schema_object, prop_name);
            let array_of = type_of(prop_schema).as_deref() == Some("array");
            if let Some(property) = schema_object_to_property(
                ctx,
                prop_name,
                prop_schema,
                required,
                array_of,
                &mut VisitedRefs::new(),
                None,
            ) {
                properties.push(property);
            }
        }
    }

    PropsResult::List(properties)
}

/// TS allOf-properties merge: `{...existing, ...property}` + the description/
/// meta rules.
fn merge_or_push(
    properties: &mut Vec<DefinitionProperty>,
    name: &str,
    property: DefinitionProperty,
) {
    if let Some(idx) = properties
        .iter()
        .position(|p| p.name.as_deref() == Some(name))
    {
        let existing = properties[idx].clone();
        properties[idx] = spread_merge(&existing, &property);
    } else {
        properties.push(property);
    }
}

fn spread_merge(
    existing: &DefinitionProperty,
    property: &DefinitionProperty,
) -> DefinitionProperty {
    // {...existing, ...property}: property's present fields win; then the
    // explicit description/meta overrides.
    let mut out = existing.clone();
    out.name = property.name.clone().or(out.name);
    out.property_type = property.property_type.clone();
    if property.description.is_some() {
        out.description = property.description.clone();
    }
    if property.examples.is_some() {
        out.examples = property.examples.clone();
    }
    if property.symbol_def.is_some() {
        out.symbol_def = property.symbol_def.clone();
    }
    if property.context.is_some() {
        out.context = property.context.clone();
    }
    if property.properties.is_some() {
        out.properties = property.properties.clone();
    }
    if property.of_property.is_some() {
        out.of_property = property.of_property.clone();
    }
    // description: property.description || existing.description || ""
    let desc = property
        .description
        .clone()
        .filter(|d| !d.is_empty())
        .or_else(|| existing.description.clone().filter(|d| !d.is_empty()))
        .unwrap_or_default();
    out.description = Some(desc);
    // meta: [...existing.meta, ...property.meta]
    let mut meta = existing.meta.clone().unwrap_or_default();
    meta.extend(property.meta.clone().unwrap_or_default());
    out.meta = Some(meta);
    out
}

/// Port of `schemaObjectToUniformDefinitionProperty`.
#[allow(clippy::too_many_arguments)]
pub fn schema_object_to_property(
    ctx: &ConvCtx,
    name: &str,
    schema: &Value,
    required: bool,
    array_of: bool,
    visited: &mut VisitedRefs,
    parent_property: Option<DefinitionProperty>,
) -> Option<DefinitionProperty> {
    if name == "__UNSAFE_refPath" {
        return None;
    }
    if schema.is_null() {
        return None;
    }
    let schema = ctx.resolve(schema);

    // Circular guard: a stamped schema whose refPath was already visited
    // returns a deep copy of the SNAPSHOT with the current name.
    let ref_path = js_ref_path(ctx, schema);
    if !ref_path.is_empty() {
        if let Some(def_prop) = visited.get(&ref_path) {
            let mut v = def_prop.clone();
            v.name = Some(name.to_string());
            return Some(v);
        }
    }

    if let Some(p) = parent_property {
        visited.insert(ref_path.clone(), p);
    }

    // A property whose schema IS a component schema links to that schema's
    // page (`objects/<Name>`) — the same link the SDK-type pass emits for
    // nested types. Identity-based (links.rs), so a schema that merely
    // contains a `$ref` never links to its member.
    let link: Option<SymbolDef> = ctx.component_link(schema);

    let has = |k: &str| schema.get(k).map(|v| !v.is_null()).unwrap_or(false);

    // anyOf → $$union (NOTE: no meta on the union property — TS omits it).
    if has("anyOf") {
        let mut component_paths: Vec<String> = Vec::new();
        let mut properties: Vec<DefinitionProperty> = Vec::new();
        for variant in schema
            .get("anyOf")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let variant = ctx.resolve(variant);
            if let Some(rp) = ctx.internal_ref_path(variant) {
                component_paths.extend(rp);
            }
            if let Some(property) =
                schema_object_to_property(ctx, name, variant, required, false, visited, None)
            {
                if is_merge_type(&property.property_type) {
                    properties.extend(property.properties.unwrap_or_default());
                } else {
                    let mut p = property;
                    let title = s(variant, "title");
                    p.name = Some(
                        title
                            .or_else(|| p.name.clone().filter(|n| !n.is_empty()))
                            .unwrap_or_default(),
                    );
                    properties.push(p);
                }
            }
        }
        ctx.set_internal_ref_path(schema, component_paths);

        let prop = DefinitionProperty {
            name: Some(name.to_string()),
            property_type: property_type::UNION.to_string(),
            description: Some(description_of(schema)),
            properties: Some(properties),
            symbol_def: link,
            ..Default::default()
        };
        if !ref_path.is_empty() {
            visited.insert(ref_path, prop.clone());
        }
        return Some(prop);
    }

    let mut meta = property_meta(schema, name, required);
    // Last, as the TS meta builder appends it.
    if let Some(value) = additional_properties_meta(ctx, schema) {
        meta.push(Meta {
            name: "additionalProperties".into(),
            value: Some(value),
        });
    }

    // oneOf → $$xor.
    if has("oneOf") {
        let mut component_paths: Vec<String> = Vec::new();
        let mut properties: Vec<DefinitionProperty> = Vec::new();
        for variant in schema
            .get("oneOf")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let variant = ctx.resolve(variant);
            if let Some(rp) = ctx.internal_ref_path(variant) {
                component_paths.extend(rp);
            }
            if let Some(property) =
                schema_object_to_property(ctx, name, variant, required, false, visited, None)
            {
                let mut p = property;
                let title = s(variant, "title");
                p.name = Some(
                    title
                        .or_else(|| p.name.clone().filter(|n| !n.is_empty()))
                        .unwrap_or_default(),
                );
                properties.push(p);
            }
        }
        ctx.set_internal_ref_path(schema, component_paths);

        let prop = DefinitionProperty {
            name: Some(name.to_string()),
            property_type: property_type::XOR.to_string(),
            description: Some(description_of(schema)),
            properties: Some(properties),
            meta: Some(meta),
            symbol_def: link,
            ..Default::default()
        };
        if !ref_path.is_empty() {
            visited.insert(ref_path, prop.clone());
        }
        return Some(prop);
    }

    // allOf → merged property.
    if has("allOf") {
        let mut component_paths: Vec<String> = Vec::new();
        // Whether the wrapper declares a `type` of its own; when it does not,
        // a folded scalar member decides the row's type (see `fold_member`).
        let wrapper_typed = type_of(schema).is_some();
        // The members that are component schemas — ONE such member links the
        // row to that schema's page (`id: allOf: [$ref WidgetId]`); two or
        // more are a composition with no single page to point at.
        let mut member_links: Vec<SymbolDef> = Vec::new();
        let mut merged = DefinitionProperty {
            name: Some(name.to_string()),
            property_type: type_of(schema).unwrap_or_default(),
            description: Some(description_of(schema)),
            properties: Some(vec![]),
            meta: Some(meta),
            symbol_def: link,
            ..Default::default()
        };

        for variant in schema
            .get("allOf")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let variant = ctx.resolve(variant);
            if let Some(rp) = ctx.internal_ref_path(variant) {
                component_paths.extend(rp);
            }
            if let Some(member_link) = ctx.component_link(variant) {
                member_links.push(member_link);
            }
            if merged.property_type.is_empty() {
                if let Some(t) = type_of(variant) {
                    merged.property_type = t;
                }
            }

            if let Some(props) = variant.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_schema) in props {
                    if is_built_in_prop(prop_name) {
                        continue;
                    }
                    let prop_schema = ctx.resolve(prop_schema);
                    let req = required_includes(variant, prop_name);
                    if let Some(property) = schema_object_to_property(
                        ctx,
                        prop_name,
                        prop_schema,
                        req,
                        false,
                        visited,
                        None,
                    ) {
                        let list = merged.properties.get_or_insert_with(Vec::new);
                        merge_or_push(list, prop_name, property);
                    }
                }
            } else if let Some(property) =
                schema_object_to_property(ctx, "", variant, false, false, visited, None)
            {
                if is_of_type(&property.property_type) {
                    merged.of_property = Some(Box::new(property));
                } else if merged.of_property.is_some() {
                    let d = property
                        .description
                        .clone()
                        .filter(|d| !d.is_empty())
                        .or_else(|| {
                            merged
                                .of_property
                                .as_ref()
                                .and_then(|o| o.description.clone())
                                .filter(|d| !d.is_empty())
                        })
                        .unwrap_or_default();
                    merged.description = Some(d);
                } else {
                    fold_member(&mut merged, property, wrapper_typed);
                }
            }
        }

        if merged.symbol_def.is_none() && member_links.len() == 1 {
            merged.symbol_def = member_links.pop();
        }

        ctx.set_internal_ref_path(schema, component_paths);
        if !ref_path.is_empty() {
            visited.insert(ref_path, merged.clone());
        }
        return Some(merged);
    }

    let mut property = DefinitionProperty {
        name: Some(name.to_string()),
        property_type: type_of(schema).unwrap_or_else(|| "object".to_string()),
        description: Some(description_of(schema)),
        meta: Some(meta.clone()),
        symbol_def: link.clone(),
        ..Default::default()
    };

    // enum → $$enum with value-named child properties. A `const` (OpenAPI 3.1)
    // is a one-value enum: same children, same `enum-type`, plus the `const`
    // meta a single-value enum carries too.
    let enum_values: Option<Vec<Value>> = schema
        .get("enum")
        .and_then(|e| e.as_array())
        .cloned()
        .or_else(|| schema.get("const").map(|c| vec![c.clone()]));
    if let Some(enum_values) = enum_values {
        // A `null` member is not a value row: it makes the enum nullable (one
        // real null for consumers), so it becomes the `nullable` meta instead
        // of a child named "null".
        let member_values: Vec<&Value> = enum_values.iter().filter(|v| !v.is_null()).collect();
        let mut meta = meta;
        if member_values.len() != enum_values.len() && !meta.iter().any(|m| m.name == "nullable") {
            meta.push(Meta::new("nullable", "true"));
        }
        // TS builds a synthetic {properties: {<String(v)>: {type}}} object and
        // runs the plain-properties path over it (JS object keys: last-wins,
        // string-coerced).
        // JS builds an OBJECT keyed by String(value) → integer-like keys sort
        // ascending first (Object key ordering), duplicates last-write-wins at
        // the first position.
        let mut m = serde_json::Map::new();
        for v in &member_values {
            m.insert(json_key(v), serde_json::Value::Null);
        }
        let keys: Vec<String> = crate::util::js_object_keys(&m)
            .into_iter()
            .cloned()
            .collect();
        let enum_properties: Vec<DefinitionProperty> = keys
            .iter()
            .map(|key| DefinitionProperty {
                name: Some(key.clone()),
                property_type: type_of(schema).unwrap_or_else(|| "object".to_string()),
                description: Some(String::new()),
                meta: Some(vec![]),
                ..Default::default()
            })
            .collect();

        meta.push(Meta {
            name: "enum-type".into(),
            value: schema.get("type").cloned().map(Some).unwrap_or(None),
        });
        // The constant's value: `{ name: "const", value: <literal> }` for a
        // single-value enum or a `const`, beside the `$$enum` children a
        // renderer already knows. Scalars stay what they are; an object or
        // array literal is JSON-stringified, like the `example` meta.
        if enum_values.len() == 1 {
            meta.push(Meta {
                name: "const".into(),
                value: Some(literal_meta_value(&enum_values[0])),
            });
        }

        let enum_property = DefinitionProperty {
            name: Some(name.to_string()),
            property_type: property_type::ENUM.to_string(),
            description: Some(description_of(schema)),
            meta: Some(meta),
            properties: Some(enum_properties),
            of_property: Some(Box::new(DefinitionProperty {
                name: Some(String::new()),
                description: Some(String::new()),
                property_type: type_of(schema).unwrap_or_default(),
                ..Default::default()
            })),
            symbol_def: link,
            ..Default::default()
        };

        if !ref_path.is_empty() {
            visited.insert(ref_path, enum_property.clone());
        }
        return Some(enum_property);
    }

    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        let mut list = Vec::new();
        for (prop_name, prop_schema) in props {
            if is_built_in_prop(prop_name) {
                continue;
            }
            let prop_schema = ctx.resolve(prop_schema);
            let req = required_includes(schema, prop_name);
            let array_of = type_of(prop_schema).as_deref() == Some("array");
            if let Some(nested) =
                schema_object_to_property(ctx, prop_name, prop_schema, req, array_of, visited, None)
            {
                list.push(nested);
            }
        }
        property.properties = Some(list);
    } else if type_of(schema).as_deref() == Some("array") {
        if let Some(items) = schema.get("items") {
            let items_resolved = ctx.resolve(items);
            // Only when items isn't a bare unresolvable $ref (post-deref it never is).
            let mut array_property = DefinitionProperty {
                name: Some(name.to_string()),
                property_type: property_type::ARRAY.to_string(),
                description: Some(description_of(schema)),
                meta: Some(meta.clone()),
                properties: Some(vec![]),
                symbol_def: link,
                ..Default::default()
            };

            let items_property = schema_object_to_property(
                ctx,
                "",
                items_resolved,
                required,
                true,
                visited,
                Some(array_property.clone()),
            );

            if let Some(items_property) = items_property {
                // An array of a component links to that component — the rule
                // the request/response roots already follow (an array's link
                // is its items').
                let items_link = items_property.symbol_def.clone();
                let items_has_of = items_property
                    .of_property
                    .as_ref()
                    .map(|o| !o.property_type.is_empty())
                    .unwrap_or(false);
                if array_of || is_of_type(&items_property.property_type) || items_has_of {
                    array_property.of_property = Some(Box::new(DefinitionProperty {
                        name: Some(String::new()),
                        property_type: items_property.property_type.clone(),
                        properties: Some(items_property.properties.clone().unwrap_or_default()),
                        description: Some(items_property.description.clone().unwrap_or_default()),
                        meta: Some(items_property.meta.clone().unwrap_or_default()),
                        of_property: items_property.of_property.clone(),
                        symbol_def: items_link.clone(),
                        ..Default::default()
                    }));
                } else {
                    array_property.properties = Some(vec![items_property]);
                }
                if array_property.symbol_def.is_none() {
                    array_property.symbol_def = items_link;
                }
            }
            if !ref_path.is_empty() {
                visited.insert(ref_path, array_property.clone());
            }
            return Some(array_property);
        }
    }

    if !ref_path.is_empty() {
        visited.insert(ref_path, property.clone());
    }
    Some(property)
}

/// The `__UNSAFE_refPath` of a schema as the JS deref leaves it.
///
/// The shared crawl (oas_doc) differs from `$RefParser.dereference` in two
/// ways that the visited-snapshot guard turns into wrong rows: it stamps a
/// `$ref`'s TARGET before descending into it (JS: `onDereference` runs after
/// the descent, so a component's own path is written LAST), and it treats the
/// enclosing object as the parent of a `$ref` inside an array (JS: the array
/// itself). So a component holding `items: $ref A` ends up stamped `A`, and
/// an `allOf: [$ref A]` wrapper ends up stamped `A` too — where the JS has
/// the component's own path and no stamp on the wrapper. Without this, the
/// second `allOf: [$ref A]` row under one root came back as a renamed copy
/// of the first (or of an earlier `array of A` row).
fn js_ref_path(ctx: &ConvCtx, schema: &Value) -> String {
    if let Some(name) = ctx.component_name(schema) {
        return format!("#/components/schemas/{name}");
    }
    let Some(stamp) = ctx.ref_path(schema) else {
        return String::new();
    };
    for key in ["allOf", "oneOf", "anyOf"] {
        let members = schema.get(key).and_then(|m| m.as_array());
        for member in members.into_iter().flatten() {
            if member.get("$ref").and_then(|r| r.as_str()) == Some(stamp) {
                return String::new();
            }
        }
    }
    stamp.to_string()
}

/// An `allOf` member that is not an of-type (a scalar, an enum, an object or
/// a composition without a `properties` key of its own) is folded onto the
/// merged row instead of being pushed as a NAMELESS CHILD (before this, a
/// described `id: allOf: [$ref Uuid]` rendered as a parent with one
/// empty-named `string` row):
/// - type: a `$$enum` member decides the row's type whatever the wrapper
///   declares (its `type: string` is the enum's base type, kept in
///   `enum-type` and `ofProperty`); otherwise the wrapper's declared type
///   wins, else the member's;
/// - description: the wrapper's, else the member's;
/// - meta: merged by name, the wrapper's winning;
/// - rows: an enum's value rows or an object's property rows, merged by name
///   into the rows the wrapper (or an earlier member) already has — a
///   composed member keeps every row whatever the member order;
/// - ofProperty: the first one seen.
fn fold_member(merged: &mut DefinitionProperty, member: DefinitionProperty, wrapper_typed: bool) {
    if member.property_type == property_type::ENUM || !wrapper_typed {
        merged.property_type = member.property_type;
    }
    if merged.description.as_deref().unwrap_or("").is_empty() {
        merged.description = Some(member.description.unwrap_or_default());
    }
    let meta = merged.meta.get_or_insert_with(Vec::new);
    for m in member.meta.unwrap_or_default() {
        if !meta.iter().any(|existing| existing.name == m.name) {
            meta.push(m);
        }
    }
    if let Some(rows) = member.properties.filter(|p| !p.is_empty()) {
        let has_rows = merged
            .properties
            .as_ref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        if !has_rows {
            merged.properties = Some(rows);
        } else {
            let list = merged.properties.get_or_insert_with(Vec::new);
            for row in rows {
                let name = row.name.clone().unwrap_or_default();
                merge_or_push(list, &name, row);
            }
        }
    }
    if merged.of_property.is_none() {
        merged.of_property = member.of_property;
    }
}

/// The `const` meta value: scalars as they are, object/array/null literals
/// JSON-stringified (TS: `typeof v === "object" ? JSON.stringify(v) : v`).
fn literal_meta_value(v: &Value) -> Value {
    if v.is_object() || v.is_array() || v.is_null() {
        Value::String(serde_json::to_string(v).unwrap_or_default())
    } else {
        v.clone()
    }
}

/// JS `String(v)` for object-key coercion of enum values.
fn json_key(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// The `additionalProperties` meta value of an object schema that allows keys
/// beyond its `properties` (port of `additionalPropertiesMetaValue`): `true`
/// for `additionalProperties: true` or a schema without a `type`, else
/// `{ type }` — the schema's type (with `"null"` added for `nullable: true`),
/// a looser form of it that never rejects what the schema accepts; nested
/// shapes are not copied. `None` when the schema does not say (or says
/// `false`).
pub fn additional_properties_meta(ctx: &ConvCtx, schema: &Value) -> Option<Value> {
    let additional = schema.get("additionalProperties")?;
    if additional == &Value::Bool(true) {
        return Some(Value::Bool(true));
    }
    let additional = ctx.resolve(additional);
    let object = additional.as_object()?;
    match object.get("type") {
        Some(Value::String(ty)) => {
            let nullable = object.get("nullable") == Some(&Value::Bool(true));
            let ty = if nullable && ty != "null" {
                serde_json::json!([ty, "null"])
            } else {
                Value::String(ty.clone())
            };
            Some(serde_json::json!({ "type": ty }))
        }
        Some(Value::Array(types)) if !types.is_empty() && types.iter().all(|t| t.is_string()) => {
            Some(serde_json::json!({ "type": types }))
        }
        _ => Some(Value::Bool(true)),
    }
}

/// Port of `schemaObjectToUniformDefinitionPropertyMeta` with the required
/// OVERRIDE applied first (TS: `{...schema, required: required?[name]:undefined}`).
pub fn property_meta(schema: &Value, name: &str, required: bool) -> Vec<Meta> {
    let mut meta = Vec::new();
    if schema.is_null() {
        return meta;
    }

    // The override discards the schema's own `required` entirely.
    if required {
        // required: [name] → the array branch matches `name`.
        let _ = name;
        meta.push(Meta::new("required", "true"));
    }

    if schema.get("deprecated").map(is_truthy).unwrap_or(false) {
        meta.push(Meta::new("deprecated", "true"));
    }
    if let Some(default) = schema.get("default") {
        meta.push(Meta {
            name: "defaults".into(),
            value: Some(default.clone()),
        });
    }
    // Presence, not truthiness — `nullable: false` ALSO yields "true" (TS quirk).
    if schema.get("nullable").is_some() {
        meta.push(Meta::new("nullable", "true"));
    }
    if let Some(example) = schema.get("example") {
        // TS: `typeof example === "object" ? JSON.stringify(example) : example`
        // — JS typeof covers objects, arrays AND null ("object" for all three).
        let value = if example.is_object() || example.is_array() || example.is_null() {
            Value::String(serde_json::to_string(example).unwrap_or_default())
        } else {
            example.clone()
        };
        meta.push(Meta {
            name: "example".into(),
            value: Some(value),
        });
    }
    if let Some(examples) = schema.get("examples") {
        meta.push(Meta {
            name: "examples".into(),
            value: Some(examples.clone()),
        });
    }
    if let Some(maximum) = schema.get("maximum") {
        meta.push(Meta {
            name: "maximum".into(),
            value: Some(maximum.clone()),
        });
    }
    if let Some(minimum) = schema.get("minimum") {
        meta.push(Meta {
            name: "minimum".into(),
            value: Some(minimum.clone()),
        });
    }

    meta
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::oap_schema_to_references;
    use apitoolchain_uniform::{DefinitionProperty, StrOrList};
    use serde_json::{json, Value};

    /// The `Widget` component page's property rows for a schema set.
    fn widget_rows(schemas: Value) -> Vec<DefinitionProperty> {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Widgets", "version": "1.0.0" },
            "paths": {},
            "components": { "schemas": schemas }
        });
        let refs = oap_schema_to_references(&doc, None);
        let widget = refs
            .into_iter()
            .find(|r| r.canonical == "objects/Widget")
            .expect("the Widget page");
        widget.definitions.into_iter().next().unwrap().properties
    }

    fn row<'a>(rows: &'a [DefinitionProperty], name: &str) -> &'a DefinitionProperty {
        rows.iter()
            .find(|p| p.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no row {name}"))
    }

    fn meta_value<'a>(p: &'a DefinitionProperty, name: &str) -> Option<&'a Value> {
        p.meta
            .as_ref()?
            .iter()
            .find(|m| m.name == name)
            .and_then(|m| m.value.as_ref())
    }

    fn canonical_of(p: &DefinitionProperty) -> Option<&str> {
        match p.symbol_def.as_ref()?.canonical.as_ref()? {
            StrOrList::One(s) => Some(s.as_str()),
            StrOrList::Many(_) => None,
        }
    }

    fn label_of(p: &DefinitionProperty) -> Option<&str> {
        p.symbol_def.as_ref()?.label.as_deref()
    }

    /// Every link to a component schema carries a `label`: the schema's
    /// constant `object` value (a single-value enum or a `const`, on the
    /// schema or on an `allOf` member), else the schema name — on nested
    /// rows, an array's items, a nullable wrapper, a union's variants, the
    /// request/response roots and the component pages themselves.
    #[test]
    fn schema_links_carry_the_schema_label() {
        let doc = json!({
            "openapi": "3.0.3",
            "info": { "title": "Widgets", "version": "1.0.0" },
            "paths": {
                "/widgets/{id}": {
                    "get": {
                        "operationId": "getWidget",
                        "responses": { "200": { "description": "ok", "content": { "application/json": {
                            "schema": { "$ref": "#/components/schemas/Widget" }
                        } } } }
                    }
                },
                "/materials": {
                    "get": {
                        "operationId": "listMaterials",
                        "responses": { "200": { "description": "ok", "content": { "application/json": {
                            "schema": { "type": "array", "items": { "$ref": "#/components/schemas/Material" } }
                        } } } }
                    },
                    "post": {
                        "operationId": "createMaterial",
                        "requestBody": { "content": { "application/json": {
                            "schema": { "$ref": "#/components/schemas/Material" }
                        } } },
                        "responses": { "200": { "description": "ok", "content": { "application/json": {
                            "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } }
                        } } } }
                    }
                }
            },
            "components": { "schemas": {
                "Widget": {
                    "type": "object",
                    "properties": {
                        "object": { "type": "string", "enum": ["widget"] },
                        "material": { "$ref": "#/components/schemas/Material" },
                        "parts": { "type": "array", "items": { "$ref": "#/components/schemas/Part" } },
                        "assembly": { "$ref": "#/components/schemas/Assembly" },
                        "shape": { "$ref": "#/components/schemas/Shape" },
                        "rank": { "$ref": "#/components/schemas/Rank" },
                        "spare": { "allOf": [{ "$ref": "#/components/schemas/Part" }], "nullable": true },
                        "backup": { "anyOf": [{ "$ref": "#/components/schemas/Part" }, { "type": "string", "nullable": true }] }
                    }
                },
                "Material": { "type": "object", "properties": { "name": { "type": "string" } } },
                "Part": { "type": "object", "properties": { "object": { "type": "string", "const": "part" } } },
                "AssemblyBase": { "type": "object", "properties": { "id": { "type": "string" } } },
                "Assembly": { "allOf": [
                    { "$ref": "#/components/schemas/AssemblyBase" },
                    { "type": "object", "properties": { "object": { "type": "string", "enum": ["assembly"] } } }
                ] },
                // A multi-value enum is not a constant; neither is a number.
                "Shape": { "type": "object", "properties": { "object": { "type": "string", "enum": ["round", "square"] } } },
                "Rank": { "type": "object", "properties": { "object": { "type": "integer", "enum": [1] } } }
            } }
        });
        let refs = oap_schema_to_references(&doc, None);
        let page = |canonical: &str| {
            refs.iter()
                .find(|r| r.canonical == canonical)
                .unwrap_or_else(|| panic!("no page {canonical}"))
        };

        let widget = page("objects/Widget");
        let rows = &widget.definitions[0].properties;
        assert_eq!(label_of(row(rows, "material")), Some("Material"));
        assert_eq!(label_of(row(rows, "assembly")), Some("assembly"));
        assert_eq!(label_of(row(rows, "shape")), Some("Shape"));
        assert_eq!(label_of(row(rows, "rank")), Some("Rank"));
        let parts = row(rows, "parts");
        assert_eq!(label_of(parts), Some("part"));
        assert_eq!(label_of(parts.of_property.as_ref().unwrap()), Some("part"));
        let spare = row(rows, "spare");
        assert_eq!(spare.property_type, "object");
        assert_eq!(meta_value(spare, "nullable"), Some(&json!("true")));
        assert_eq!(label_of(spare), Some("part"));
        let backup = row(rows, "backup");
        assert_eq!(backup.property_type, "$$union");
        assert!(backup.symbol_def.is_none());
        assert_eq!(
            label_of(&backup.properties.as_ref().unwrap()[0]),
            Some("part")
        );

        // The component pages' own links.
        let own = |canonical: &str| {
            page(canonical).definitions[0]
                .symbol_def
                .clone()
                .unwrap_or_else(|| panic!("no symbolDef on {canonical}"))
        };
        assert_eq!(own("objects/Widget").label.as_deref(), Some("widget"));
        assert_eq!(
            own("objects/Widget").canonical,
            Some(StrOrList::One("objects/Widget".into()))
        );
        assert_eq!(own("objects/Material").label.as_deref(), Some("Material"));
        assert_eq!(own("objects/Assembly").label.as_deref(), Some("assembly"));

        // The request/response roots: a `$ref` root, an array root (its
        // items'), and an inline root (no component → no link).
        let root = |canonical: &str, title: &str| {
            page(canonical)
                .definitions
                .iter()
                .find(|d| d.title == title)
                .unwrap()
                .variants
                .as_ref()
                .unwrap()[0]
                .symbol_def
                .clone()
        };
        let get = root("getWidget", "Response").expect("a link");
        assert_eq!(get.label.as_deref(), Some("widget"));
        assert_eq!(get.canonical, Some(StrOrList::One("objects/Widget".into())));
        assert_eq!(
            root("listMaterials", "Response").unwrap().label.as_deref(),
            Some("Material")
        );
        assert_eq!(
            root("createMaterial", "Request body")
                .unwrap()
                .label
                .as_deref(),
            Some("Material")
        );
        assert!(root("createMaterial", "Response").is_none());
    }

    /// A single-value enum and a `const` both carry `{ name: "const", value }`
    /// beside the `$$enum` children; a multi-value enum does not.
    #[test]
    fn const_meta_on_single_value_enums_and_const() {
        let rows = widget_rows(json!({
            "Widget": {
                "type": "object",
                "properties": {
                    "object": { "type": "string", "enum": ["widget"] },
                    "kind": { "type": "string", "const": "standard" },
                    "version": { "type": "integer", "const": 2 },
                    "status": { "type": "string", "enum": ["active", "archived"] }
                }
            }
        }));

        let object = row(&rows, "object");
        assert_eq!(object.property_type, "$$enum");
        assert_eq!(meta_value(object, "const"), Some(&json!("widget")));
        assert_eq!(meta_value(object, "enum-type"), Some(&json!("string")));
        assert_eq!(object.properties.as_ref().unwrap().len(), 1);

        let kind = row(&rows, "kind");
        assert_eq!(kind.property_type, "$$enum");
        assert_eq!(meta_value(kind, "const"), Some(&json!("standard")));
        assert_eq!(
            kind.properties.as_ref().unwrap()[0].name.as_deref(),
            Some("standard")
        );

        assert_eq!(meta_value(row(&rows, "version"), "const"), Some(&json!(2)));

        let status = row(&rows, "status");
        assert_eq!(status.property_type, "$$enum");
        assert!(meta_value(status, "const").is_none());
        assert_eq!(status.properties.as_ref().unwrap().len(), 2);
    }

    /// `allOf: [$ref <scalar>]` + description is a scalar row with the
    /// member's type and meta — never a parent with one nameless child. An
    /// enum member folds its values in; the wrapper's own meta wins by name.
    #[test]
    fn allof_scalar_member_folds_onto_the_row() {
        let rows = widget_rows(json!({
            "WidgetId": { "type": "string", "format": "uuid", "example": "wid_1", "description": "A widget id." },
            "Color": { "type": "string", "enum": ["red", "green"] },
            "Widget": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "allOf": [{ "$ref": "#/components/schemas/WidgetId" }], "description": "The widget's id." },
                    "color": { "allOf": [{ "$ref": "#/components/schemas/Color" }], "example": "green" },
                    "other": { "allOf": [{ "$ref": "#/components/schemas/WidgetId" }] },
                    "shade": { "type": "string", "allOf": [{ "$ref": "#/components/schemas/Color" }] }
                }
            }
        }));

        let id = row(&rows, "id");
        assert_eq!(id.property_type, "string");
        assert_eq!(id.description.as_deref(), Some("The widget's id."));
        assert_eq!(id.properties.as_ref().map(|p| p.len()), Some(0));
        assert_eq!(meta_value(id, "required"), Some(&json!("true")));
        assert_eq!(meta_value(id, "example"), Some(&json!("wid_1")));
        assert_eq!(canonical_of(id), Some("objects/WidgetId"));

        let color = row(&rows, "color");
        assert_eq!(color.property_type, "$$enum");
        assert_eq!(color.properties.as_ref().unwrap().len(), 2);
        assert_eq!(meta_value(color, "enum-type"), Some(&json!("string")));
        // The wrapper's `example` wins over the member's (none here) by name.
        assert_eq!(meta_value(color, "example"), Some(&json!("green")));
        assert_eq!(color.of_property.as_ref().unwrap().property_type, "string");

        // No wrapper description → the member's.
        assert_eq!(
            row(&rows, "other").description.as_deref(),
            Some("A widget id.")
        );

        // A typed wrapper (`type: string`) around an enum component is still
        // an enum row: the marker wins over the declared base type.
        let shade = row(&rows, "shade");
        assert_eq!(shade.property_type, "$$enum");
        assert_eq!(shade.properties.as_ref().unwrap().len(), 2);
        assert_eq!(meta_value(shade, "enum-type"), Some(&json!("string")));
        assert_eq!(shade.of_property.as_ref().unwrap().property_type, "string");
        assert_eq!(canonical_of(shade), Some("objects/Color"));
    }

    /// A composed member (a component that is itself an `allOf`, so it has no
    /// `properties` key of its own) contributes its rows whatever its position
    /// among the members: merged by name into the rows already there, never
    /// dropped and never a nameless child group.
    #[test]
    fn allof_composed_member_rows_merge_whatever_the_order() {
        let rows = widget_rows(json!({
            "Base": { "type": "object", "properties": { "id": { "type": "string" } } },
            "Composed": {
                "allOf": [
                    { "$ref": "#/components/schemas/Base" },
                    { "type": "object", "properties": { "sku": { "type": "string" } } }
                ]
            },
            "Widget": {
                "type": "object",
                "properties": {
                    "inline_first": {
                        "allOf": [
                            { "type": "object", "properties": { "name": { "type": "string" } } },
                            { "$ref": "#/components/schemas/Composed" }
                        ]
                    },
                    "composed_first": {
                        "allOf": [
                            { "$ref": "#/components/schemas/Composed" },
                            { "type": "object", "properties": { "name": { "type": "string", "description": "overrides" } } }
                        ]
                    },
                    "overlap": {
                        "allOf": [
                            { "type": "object", "properties": { "id": { "type": "string", "description": "the wrapper's id" } } },
                            { "$ref": "#/components/schemas/Composed" }
                        ]
                    }
                }
            }
        }));

        let names = |p: &DefinitionProperty| -> Vec<String> {
            p.properties
                .as_ref()
                .unwrap()
                .iter()
                .map(|c| c.name.clone().unwrap_or_default())
                .collect()
        };

        let inline_first = row(&rows, "inline_first");
        assert_eq!(inline_first.property_type, "object");
        assert_eq!(names(inline_first), vec!["name", "id", "sku"]);

        let composed_first = row(&rows, "composed_first");
        assert_eq!(names(composed_first), vec!["id", "sku", "name"]);
        assert_eq!(
            row(composed_first.properties.as_ref().unwrap(), "name")
                .description
                .as_deref(),
            Some("overrides")
        );

        // A row both sides declare is merged by name: one `id`, the later
        // member's shape over the earlier one's, the description kept.
        let overlap = row(&rows, "overlap");
        assert_eq!(names(overlap), vec!["id", "sku"]);
        assert_eq!(
            row(overlap.properties.as_ref().unwrap(), "id")
                .description
                .as_deref(),
            Some("the wrapper's id")
        );
    }

    /// Nested properties that ARE a component schema link to its page —
    /// directly, through `allOf: [$ref]`, and as an array's items; inline
    /// schemas and multi-member compositions do not.
    #[test]
    fn nested_properties_link_to_their_component_schema() {
        let rows = widget_rows(json!({
            "WidgetDefinition": { "type": "object", "properties": { "mode": { "type": "string" } } },
            "Part": { "type": "object", "properties": { "sku": { "type": "string" } } },
            "Base": { "type": "object", "properties": { "id": { "type": "string" } } },
            "Widget": {
                "type": "object",
                "properties": {
                    "definition": { "$ref": "#/components/schemas/WidgetDefinition" },
                    "described": { "allOf": [{ "$ref": "#/components/schemas/WidgetDefinition" }], "description": "d" },
                    "parts": { "type": "array", "items": { "$ref": "#/components/schemas/Part" } },
                    "inline": { "type": "object", "properties": { "a": { "type": "string" } } },
                    "composed": { "allOf": [{ "$ref": "#/components/schemas/Base" }, { "$ref": "#/components/schemas/Part" }] }
                }
            }
        }));

        let definition = row(&rows, "definition");
        assert_eq!(canonical_of(definition), Some("objects/WidgetDefinition"));
        assert_eq!(
            definition.symbol_def.as_ref().unwrap().id,
            Some(StrOrList::One(
                "#/components/schemas/WidgetDefinition".into()
            ))
        );
        assert_eq!(definition.properties.as_ref().unwrap().len(), 1);

        assert_eq!(
            canonical_of(row(&rows, "described")),
            Some("objects/WidgetDefinition")
        );

        let parts = row(&rows, "parts");
        assert_eq!(parts.property_type, "$$array");
        assert_eq!(canonical_of(parts), Some("objects/Part"));
        assert_eq!(
            canonical_of(parts.of_property.as_ref().unwrap()),
            Some("objects/Part")
        );

        assert!(row(&rows, "inline").symbol_def.is_none());
        assert!(row(&rows, "composed").symbol_def.is_none());
    }
}
