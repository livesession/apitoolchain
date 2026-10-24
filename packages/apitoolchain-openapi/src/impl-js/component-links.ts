import {OpenAPIV3} from "openapi-types";

import type {SymbolDef} from "@xyd-js/uniform";

// Schema links — which schema objects ARE component schemas, and the
// `symbolDef` a property pointing at one carries.
//
// Identity, not stamps: the deref writes `__UNSAFE_refPath` onto the TARGET of
// a `$ref` and onto its PARENT container, so a stamp alone cannot tell "this
// schema is `Widget`" from "this schema contains a `$ref` to `Widget`". The
// dereferenced document shares ONE object per component between
// `components.schemas.<Name>` and every `$ref` to it, so object identity is
// the exact answer. The Rust twin (crates/apitoolchain_openapi/src/links.rs)
// answers by node address with the same first-registration-wins rule, so a
// pure-`$ref` alias (`B: {$ref: A}`) keeps the name it points at on both sides.
const componentNames = new WeakMap<object, string>();
// The link label of each component schema (see `objectTypeConstant`), keyed
// like the names.
const componentLabels = new WeakMap<object, string>();

/** Register a dereferenced document's component schemas by identity. Called at
 *  the start of `oapSchemaToReferences`, before any property is converted. */
export function registerComponentSchemas(doc: OpenAPIV3.Document | undefined) {
    const schemas = doc?.components?.schemas;
    if (!schemas) {
        return;
    }
    for (const [name, schema] of Object.entries(schemas)) {
        if (schema && typeof schema === "object" && !componentNames.has(schema)) {
            componentNames.set(schema, name);
            componentLabels.set(schema, objectTypeConstant(schema as OpenAPIV3.SchemaObject) ?? name);
        }
    }
}

/** The "object type" convention: a schema whose `object` property is a string
 *  constant — a single-value `enum` or a `const` — names its instances by it
 *  (`{ "object": "widget", ... }`). Read on the schema's own `properties`, else
 *  on the first `allOf` member that declares one (a composed model keeps the
 *  tag on a member). Anything else (no `object` property, a multi-value enum,
 *  a non-string literal) → `undefined`, and the link falls back to the schema
 *  name. */
export function objectTypeConstant(schema: OpenAPIV3.SchemaObject | undefined): string | undefined {
    if (!schema || typeof schema !== "object") {
        return undefined;
    }
    const own = ownObjectConstant(schema);
    if (own !== undefined) {
        return own;
    }
    for (const member of schema.allOf || []) {
        const found = ownObjectConstant(member as OpenAPIV3.SchemaObject);
        if (found !== undefined) {
            return found;
        }
    }
    return undefined;
}

function ownObjectConstant(schema: OpenAPIV3.SchemaObject | undefined): string | undefined {
    const object = schema?.properties?.object as (OpenAPIV3.SchemaObject & { const?: unknown }) | undefined;
    if (!object || typeof object !== "object") {
        return undefined;
    }
    if (Array.isArray(object.enum)) {
        return object.enum.length === 1 && typeof object.enum[0] === "string" ? object.enum[0] : undefined;
    }
    return typeof object.const === "string" ? object.const : undefined;
}

/** The component schema a (dereferenced) schema object IS — by identity,
 *  never by containment. */
export function componentSchemaName(schema: unknown): string | undefined {
    if (!schema || typeof schema !== "object") {
        return undefined;
    }
    return componentNames.get(schema);
}

/** `{ id: "#/components/schemas/<Name>", canonical: "objects/<Name>", label }`
 *  — `canonical` is the component's own reference page (oas-componentSchemas.ts
 *  emits `objects/<Name>`), `id` the JSON pointer; both plain strings, the
 *  shape the SDK-type pass emits for nested type links as well. `label` is
 *  what a renderer prints where it would otherwise print a bare `object`. */
export function componentSymbolDef(name: string, label: string = name): SymbolDef {
    return {
        id: `#/components/schemas/${name}`,
        canonical: `objects/${name}`,
        label,
    };
}

/** The link a property carries when its schema is a component schema. */
export function componentLink(schema: unknown): SymbolDef | undefined {
    const name = componentSchemaName(schema);
    if (!name) {
        return undefined;
    }
    const label = componentLabels.get(schema as object) ?? name;
    return componentSymbolDef(name, label);
}
