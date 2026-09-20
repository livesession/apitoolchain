// @xyd-js/gql public API — NATIVE-ONLY.
//
// The conversion itself is crates/apitoolchain_gql, reached through
// @xyd-js/native. The frozen TypeScript implementation that used to back a
// fallback (src/impl-js, ~2100 lines) was deleted once @xyd-js/native@0.1.0
// shipped real platform binaries: keeping a fallback means keeping the
// implementation it falls back to, which is the duplication this removes.
//
// A missing addon therefore THROWS rather than silently degrading. That is
// deliberate — a silent degrade is how a native-vs-JS divergence hides, and
// this package had one (see the sidebar "undefined" fix). Same posture as
// xyd's @xyd-js/opensdk-uniform.
//
// What stays in JS, and why it is NOT a fallback:
//   - fetching http(s) schema locations (the Rust core takes files or raw SDL;
//     the converter crates are deliberately network-free)
//   - reattaching the `__UNSAFE_route` thunk, which JSON cannot carry
import type { Reference } from "@xyd-js/uniform";

import { native } from "./native";
import type { GQLSchemaToReferencesOptions } from "./types";

export async function gqlSchemaToReferences(
    schemaLocation: string | string[],
    options?: GQLSchemaToReferencesOptions
): Promise<Reference[]> {
    if (!native?.gqlSchemaToReferences) {
        throw new Error(
            "@xyd-js/gql requires @xyd-js/native, which did not load. " +
                "Install it (it is an optionalDependency, so a failed install is silent) " +
                "or build it locally with `pnpm --filter @xyd-js/native build:native`. " +
                "There is no JavaScript fallback: it was removed in favour of a single implementation."
        );
    }

    const locations = Array.isArray(schemaLocation) ? schemaLocation : [schemaLocation];
    // URLs are fetched HERE (JS) — the native layer reads files / raw SDL only
    // (no network in the Rust core).
    const sources = await Promise.all(
        locations.map(async (location) => {
            if (location.startsWith("http://") || location.startsWith("https://")) {
                const response = await fetch(location);
                if (!response.ok) {
                    throw new Error(`Failed to fetch schema from URL: ${location}`);
                }
                return response.text();
            }
            return location;
        })
    );

    const { references, route } = JSON.parse(
        native.gqlSchemaToReferences(sources, options ? JSON.stringify(options) : undefined)
    );
    if (route) {
        // Non-serializable thunk consumed by plugin-docs' graphql preset —
        // reattached from the native envelope (JSON can't carry functions).
        references.__UNSAFE_route = () => route;
    }
    return references;
}
