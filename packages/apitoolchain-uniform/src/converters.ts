// The JSON-Schema converter — NATIVE-ONLY.
//
// It lives in crates/apitoolchain_uniform::converters and is reached through
// @xyd-js/native. The frozen TypeScript copy (src/impl-js) was deleted once
// @xyd-js/native@0.1.0 shipped real platform binaries: keeping a fallback means
// keeping the implementation it falls back to, which is the duplication this
// removes.
//
// `uniformPropertiesToJsonSchema` went away with it rather than gaining a
// native binding. It LOOKED public — src/index.ts exported it — but the
// package's real entry is the root index.ts, which never re-exported it, so it
// never reached dist and no consumer could import it. Binding a symbol nobody
// can reach would have meant an napi export, a wider crate surface and a hard
// floor on the addon version, all to preserve dead code. The Rust that backs it
// stays: the crate still uses it internally.
//
// IMPORTANT: this package must never statically import the native addon. There
// is no `browser` export condition and Vite bundles it into the client, so a
// static import fails the browser build at resolve time. The `getBuiltinModule`
// shape in ./native is what keeps that working (and what lets the whole module
// tree-shake out of a client bundle, which it does).
//
// Deliberately phrased without the literal import syntax: scripts/check-
// standalone.sh greps for it by text, so spelling the banned form out — even
// inside a comment explaining the ban — fails CI.
import type { JSONSchema7 } from "json-schema";

import type { Reference } from "./types";
import { native } from "./native";

export function uniformToInputJsonSchema(reference: Reference): JSONSchema7 | null {
    if (!native?.uniformToInputJsonSchema) {
        throw new Error(
            "@xyd-js/uniform.uniformToInputJsonSchema requires @xyd-js/native, which did not load. " +
                "Install it (it is an optionalDependency, so a failed install is silent) " +
                "or build it locally with `pnpm --filter @xyd-js/native build:native`. " +
                "There is no JavaScript fallback: it was removed in favour of a single implementation."
        );
    }
    return JSON.parse(native.uniformToInputJsonSchema(JSON.stringify(reference)));
}
