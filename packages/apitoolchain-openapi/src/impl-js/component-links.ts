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
        }
    }
}

/** The component schema a (dereferenced) schema object IS — by identity,
 *  never by containment. */
export function componentSchemaName(schema: unknown): string | undefined {
    if (!schema || typeof schema !== "object") {
        return undefined;
    }
    return componentNames.get(schema);
}

/** `{ id: "#/components/schemas/<Name>", canonical: "objects/<Name>" }` —
 *  `canonical` is the component's own reference page (oas-componentSchemas.ts
 *  emits `objects/<Name>`), `id` the JSON pointer; both plain strings, the
 *  shape the SDK-type pass emits for nested type links as well. */
export function componentSymbolDef(name: string): SymbolDef {
    return {
        id: `#/components/schemas/${name}`,
        canonical: `objects/${name}`,
    };
}

/** The link a property carries when its schema is a component schema. */
export function componentLink(schema: unknown): SymbolDef | undefined {
    const name = componentSchemaName(schema);
    return name ? componentSymbolDef(name) : undefined;
}
