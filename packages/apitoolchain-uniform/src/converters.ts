// The JSON-Schema converters — NATIVE-ONLY.
//
// Both live in crates/apitoolchain_uniform::converters and are reached through
// @xyd-js/native. The frozen TypeScript copy (src/impl-js) was deleted once
// @xyd-js/native@0.1.0 shipped real platform binaries: keeping a fallback means
// keeping the implementation it falls back to, which is the duplication this
// removes.
//
// `uniformPropertiesToJsonSchema` gained a native entry as part of that
// deletion. It had none before — the shim just re-exported the frozen impl —
// and it is re-exported from this package's root, so it is public API. Dropping
// it to avoid binding it would have been an API break taken for the
// implementer's convenience.
//
// IMPORTANT: no static `import ... from "@xyd-js/native"` may appear in this
// package. There is no `browser` export condition and Vite bundles it into the
// client, so a static import fails the browser build at resolve time. The
// `getBuiltinModule` shape in ./native is what keeps that working (and what
// lets the whole module tree-shake out of a client bundle, which it does).
import type { JSONSchema7 } from "json-schema";

import type { Reference, DefinitionProperty } from "./types";
import { native } from "./native";

function requireNative(entry: string): never {
    throw new Error(
        `@xyd-js/uniform.${entry} requires @xyd-js/native, which did not load. ` +
            "Install it (it is an optionalDependency, so a failed install is silent) " +
            "or build it locally with `pnpm --filter @xyd-js/native build:native`. " +
            "There is no JavaScript fallback: it was removed in favour of a single implementation."
    );
}

export function uniformToInputJsonSchema(reference: Reference): JSONSchema7 | null {
    if (!native?.uniformToInputJsonSchema) {
        requireNative("uniformToInputJsonSchema");
    }
    return JSON.parse(native.uniformToInputJsonSchema(JSON.stringify(reference)));
}

export function uniformPropertiesToJsonSchema(
    properties: DefinitionProperty[] | DefinitionProperty,
    id?: string
): JSONSchema7 | null {
    if (!native?.uniformPropertiesToJsonSchema) {
        requireNative("uniformPropertiesToJsonSchema");
    }
    return JSON.parse(
        native.uniformPropertiesToJsonSchema(JSON.stringify(properties), id)
    );
}
