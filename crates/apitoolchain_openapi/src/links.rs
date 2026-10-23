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
        if let Some(schemas) = doc
            .raw
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.as_object())
        {
            for (name, schema) in schemas {
                let node = doc.resolve(schema);
                components.entry(addr(node)).or_insert_with(|| name.clone());
            }
        }
        ConvCtx { doc, components }
    }

    /// The component schema a (resolved) node IS — by identity, never by
    /// containment.
    pub fn component_name(&self, node: &Value) -> Option<&str> {
        self.components.get(&addr(node)).map(|s| s.as_str())
    }

    /// The link a property carries when its schema is a component schema.
    pub fn component_link(&self, node: &Value) -> Option<SymbolDef> {
        self.component_name(node).map(component_symbol_def)
    }
}

/// `{ id: "#/components/schemas/<Name>", canonical: "objects/<Name>" }` —
/// `canonical` is the component's own reference page (components.rs emits
/// `objects/<Name>`), `id` the JSON pointer; both plain strings, which is the
/// shape the SDK-type pass emits for nested type links as well.
pub fn component_symbol_def(name: &str) -> SymbolDef {
    SymbolDef {
        id: Some(StrOrList::One(format!("#/components/schemas/{name}"))),
        canonical: Some(StrOrList::One(format!("objects/{name}"))),
    }
}
