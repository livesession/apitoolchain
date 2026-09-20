// @xyd-js/mcp-uniform public API — NATIVE-ONLY for the conversion.
//
// Two halves, and only one of them was ever duplicated:
//   - transport (./transport): JSON-RPC over HTTP/SSE, bearer auth, local-manifest
//     IO. Runs in every mode — the converter crates are deliberately HTTP-free and
//     filesystem-free — so it is permanent JS, not a fallback.
//   - conversion (surface -> Reference[]): crates/apitoolchain_mcp_uniform, reached
//     through @xyd-js/native. Its frozen TypeScript twin was deleted once
//     @xyd-js/native@0.1.0 shipped platform binaries.
//
// A missing addon therefore THROWS rather than silently degrading.
import type { Reference } from "@xyd-js/uniform";

import { native } from "./native";
// Transport runs in BOTH modes (the converter crates are deliberately HTTP-free),
// so it lives outside impl-js and survives the reap.
import { resolveMcpSurface, type McpUrlToReferencesOptions } from "./transport";
export type { McpTool, McpResource, JsonSchemaObject } from "./types";
export type { McpFetcher, McpUrlToReferencesOptions } from "./transport";

export async function mcpUrlToReferences(
    source: string,
    options: McpUrlToReferencesOptions = {},
): Promise<Reference[]> {
    if (!native?.mcpToReferences) {
        throw new Error(
            "@xyd-js/mcp-uniform requires @xyd-js/native, which did not load. " +
                "Install it (it is an optionalDependency, so a failed install is silent) " +
                "or build it locally with `pnpm --filter @xyd-js/native build:native`. " +
                "There is no JavaScript fallback: it was removed in favour of a single implementation."
        );
    }
    if (!source) {
        return [];
    }
    const surface = await resolveMcpSurface(source, options);
    return JSON.parse(native.mcpToReferences(JSON.stringify(surface))) as Reference[];
}
