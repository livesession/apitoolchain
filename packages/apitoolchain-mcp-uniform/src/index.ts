// @xyd-js/mcp-uniform public API (S6+ W3 rider shim): the JSON-RPC transport,
// auth headers and local-manifest IO stay JS (impl-js resolveMcpSurface); the
// surface → Reference[] conversion dispatches to the Rust core
// (crates/apitoolchain_mcp_uniform via @xyd-js/native) when present.
import type { Reference } from "@xyd-js/uniform";

import { native } from "./native";
// Transport runs in BOTH modes (the converter crates are deliberately HTTP-free),
// so it lives outside impl-js and survives the reap.
import { resolveMcpSurface, type McpUrlToReferencesOptions } from "./transport";
// The frozen JS conversion — used only when the native addon is absent.
import { mcpUrlToReferences as jsMcpUrlToReferences } from "./impl-js/index";

export type { McpTool, McpResource, JsonSchemaObject } from "./types";
export type { McpFetcher, McpUrlToReferencesOptions } from "./transport";

export async function mcpUrlToReferences(
    source: string,
    options: McpUrlToReferencesOptions = {},
): Promise<Reference[]> {
    if (!native?.mcpToReferences) {
        return jsMcpUrlToReferences(source, options);
    }
    if (!source) {
        return [];
    }
    const surface = await resolveMcpSurface(source, options);
    return JSON.parse(native.mcpToReferences(JSON.stringify(surface))) as Reference[];
}
