//! Schema links — which nodes ARE component schemas, and the `symbolDef` a
//! property pointing at one carries.
//!
//! Identity, not stamps: the deref layer writes `__UNSAFE_refPath` onto the
//! TARGET of a `$ref` and onto its PARENT container, so a stamp alone cannot
//! tell "this schema is `Widget`" from "this schema contains a `$ref` to
//! `Widget`". Both twins therefore answer by object identity — the JS side
//! through a `WeakMap` over the dereferenced document (`component-links.ts`),
//! this side through the resolved node's address — which agrees by
//! construction and never links a container to one of its members.

use std::collections::HashMap;

use apitoolchain_uniform::{StrOrList, SymbolDef};
use serde_json::Value;

use crate::doc::DocCtx;

/// The conversion context: the deref layer plus the component-schema identity
/// map. Derefs to [`DocCtx`], so `ctx.resolve(..)` / `ctx.ref_path(..)` read
/// as before.
pub struct ConvCtx<'a> {
    pub doc: DocCtx<'a>,
    /// Resolved node address → component schema name. First registration
    /// wins, so a pure-`$ref` alias (`B: {$ref: A}`) keeps the name it points
    /// at — the JS twin registers in the same order with the same rule.
    components: HashMap<usize, String>,
    /// Component schema name → its link label (see [`object_type_constant`]).
    labels: HashMap<String, String>,
}

impl<'a> std::ops::Deref for ConvCtx<'a> {
    type Target = DocCtx<'a>;

    fn deref(&self) -> &DocCtx<'a> {
        &self.doc
    }
}

fn addr(v: &Value) -> usize {
    v as *const Value as usize
}

impl<'a> ConvCtx<'a> {
    pub fn new(doc: DocCtx<'a>) -> Self {
        let mut components: HashMap<usize, String> = HashMap::new();
        let mut labels: HashMap<String, String> = HashMap::new();
        if let Some(schemas) = doc
            .raw
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.as_object())
        {
            for (name, schema) in schemas {
                let node = doc.resolve(schema);
                components.entry(addr(node)).or_insert_with(|| name.clone());
                let label = object_type_constant(&doc, node).unwrap_or_else(|| name.clone());
                labels.insert(name.clone(), label);
            }
        }
        ConvCtx {
            doc,
            components,
            labels,
        }
    }

    /// The component schema a (resolved) node IS — by identity, never by
    /// containment.
    pub fn component_name(&self, node: &Value) -> Option<&str> {
        self.components.get(&addr(node)).map(|s| s.as_str())
    }

    /// The label of a component schema's link: its constant `object` value,
    /// else its name.
    pub fn component_label<'b>(&'b self, name: &'b str) -> &'b str {
        self.labels.get(name).map(|s| s.as_str()).unwrap_or(name)
    }

    /// The link a property carries when its schema is a component schema.
    pub fn component_link(&self, node: &Value) -> Option<SymbolDef> {
        self.component_name(node)
            .map(|name| component_symbol_def(name, self.component_label(name)))
    }
}

/// `{ id: "#/components/schemas/<Name>", canonical: "objects/<Name>", label }`
/// — `canonical` is the component's own reference page (components.rs emits
/// `objects/<Name>`), `id` the JSON pointer; both plain strings, which is the
/// shape the SDK-type pass emits for nested type links as well. `label` is
/// what a renderer prints where it would otherwise print a bare `object`.
pub fn component_symbol_def(name: &str, label: &str) -> SymbolDef {
    SymbolDef {
        id: Some(StrOrList::One(format!("#/components/schemas/{name}"))),
        canonical: Some(StrOrList::One(format!("objects/{name}"))),
        label: Some(label.to_string()),
    }
}

/// The "object type" convention: a schema whose `object` property is a string
/// constant — a single-value `enum` or a `const` — names its instances by it
/// (`{ "object": "widget", ... }`). Read on the schema's own `properties`,
/// else on the first `allOf` member that declares one (a composed model keeps
/// the tag on a member). Anything else (no `object` property, a multi-value
/// enum, a non-string literal) → `None`, and the link falls back to the
/// schema name.
pub fn object_type_constant(doc: &DocCtx, schema: &Value) -> Option<String> {
    let schema = doc.resolve(schema);
    if let Some(found) = own_object_constant(doc, schema) {
        return Some(found);
    }
    for member in schema.get("allOf")?.as_array()? {
        if let Some(found) = own_object_constant(doc, doc.resolve(member)) {
            return Some(found);
        }
    }
    None
}

fn own_object_constant(doc: &DocCtx, schema: &Value) -> Option<String> {
    let object = doc.resolve(schema.get("properties")?.get("object")?);
    if let Some(values) = object.get("enum").and_then(|e| e.as_array()) {
        if values.len() == 1 {
            return values[0].as_str().map(String::from);
        }
        return None;
    }
    object.get("const")?.as_str().map(String::from)
}
