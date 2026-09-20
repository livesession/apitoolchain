// MCP transport: JSON-RPC over HTTP/SSE, bearer auth, and local-manifest IO.
//
// NOT part of the frozen JS fallback, despite having lived in `impl-js` beside
// it. This is permanent infrastructure: the converter crates are deliberately
// HTTP-free and filesystem-free, so fetching an MCP server's surface stays in
// JS in BOTH modes — `src/index.ts` calls `resolveMcpSurface` and then hands the
// result to the native converter.
//
// Split out so that deleting `impl-js` (the actual duplicate of
// crates/apitoolchain_mcp_uniform) becomes an unambiguous delete rather than a
// surgical extraction. The two halves were verified independent before the
// split: nothing here calls a conversion function, and no conversion function
// calls transport. They were INTERLEAVED in the original file, not contiguous —
// `mcpUrlToReferences` sat between `resolveMcpSurface` and its own helpers —
// which is why this is a function-level split, not a line-range one.

import type { Reference, MCPReferenceContext } from "@xyd-js/uniform";

import type {
    JsonRpcResponse,
    JsonSchemaObject,
    McpResource,
    McpTool,
} from "./types";

export type { McpTool, McpResource, JsonSchemaObject } from "./types";

/**
 * Minimal JSON-RPC fetcher signature. Defaults to global fetch; tests inject a stub.
 */
export type McpFetcher = (
    url: string,
    init: { method: string; headers: Record<string, string>; body: string },
) => Promise<{ ok: boolean; status: number; json: () => Promise<unknown> }>;

export interface McpUrlToReferencesOptions {
    /** Bearer token sent as `Authorization: Bearer <token>`. */
    token?: string;
    /** Extra headers merged on top of the default JSON-RPC headers. */
    headers?: Record<string, string>;
    /** Override the network call (used by tests). */
    fetcher?: McpFetcher;
}

/**
 * Convert an MCP source into a Reference[]. The source is either:
 *   - a URL (http/https/sse) — fetched live via JSON-RPC at build time, or
 *   - a local file path to a JSON manifest with the shape
 *     `{ tools: McpTool[], resources?: McpResource[] }` (same shape the
 *     server would return from tools/list + resources/list).
 *
 * One Reference is emitted per tool (MCP_TOOL) and per resource (MCP_RESOURCE),
 * mirroring the one-page-per-endpoint layout used by OpenAPI/GraphQL converters.
 *
 * TODO: prompts/list (MCP_PROMPT)
 * TODO: stdio transport (spawn local process)
 */
/**
 * The fetched MCP surface: what conversion needs after the RPC/manifest step.
 * Extracted as the shim seam (S6+ W3) — the Rust core converts this shape.
 */
export interface McpSurface {
    tools: McpTool[];
    resources: McpResource[];
    serverUrl: string;
    transport: MCPReferenceContext["transport"];
}

export async function resolveMcpSurface(
    source: string,
    options: McpUrlToReferencesOptions = {},
): Promise<McpSurface> {
    const isUrl = /^https?:\/\//i.test(source);

    let tools: McpTool[] = [];
    let resources: McpResource[] = [];
    let transport: MCPReferenceContext["transport"] = "http";
    let serverUrl = source;

    if (isUrl) {
        transport = source.includes("/sse") ? "sse" : "http";

        const headers: Record<string, string> = {
            "Content-Type": "application/json",
            Accept: "application/json, text/event-stream",
            ...(options.headers || {}),
        };
        if (options.token) {
            headers.Authorization = `Bearer ${options.token}`;
        }

        const rpc = makeRpcClient(source, headers, options.fetcher);

        const [toolsResult, resourcesResult] = await Promise.all([
            rpc<{ tools?: McpTool[] }>("tools/list").catch(() => ({ tools: [] })),
            rpc<{ resources?: McpResource[] }>("resources/list").catch(() => ({
                resources: [],
            })),
        ]);
        tools = toolsResult.tools || [];
        resources = resourcesResult.resources || [];
    } else {
        // Local manifest mode — read tools / resources from a JSON file shaped
        // like the combined output of tools/list and resources/list.
        const manifest = await readLocalManifest(source);
        tools = manifest.tools || [];
        resources = manifest.resources || [];
        // For local manifests, drop the file path from the rendered context —
        // it's irrelevant to consumers of the generated docs.
        serverUrl = manifest.serverUrl || "";
    }

    return { tools, resources, serverUrl, transport };
}

interface McpManifest {
    serverUrl?: string;
    tools?: McpTool[];
    resources?: McpResource[];
}

async function readLocalManifest(filePath: string): Promise<McpManifest> {
    const fs = await import("node:fs/promises");
    const raw = await fs.readFile(filePath, "utf8");
    return JSON.parse(raw) as McpManifest;
}

function makeRpcClient(
    url: string,
    headers: Record<string, string>,
    fetcher: McpFetcher = defaultFetcher,
) {
    let nextId = 1;
    return async function call<T>(method: string, params?: unknown): Promise<T> {
        const id = nextId++;
        const body = JSON.stringify({ jsonrpc: "2.0", id, method, params });
        const res = await fetcher(url, { method: "POST", headers, body });
        if (!res.ok) {
            throw new Error(`MCP RPC ${method} failed: ${res.status}`);
        }
        const json = (await res.json()) as JsonRpcResponse<T>;
        if (json.error) {
            throw new Error(`MCP RPC ${method} error: ${json.error.message}`);
        }
        return (json.result ?? ({} as T)) as T;
    };
}

const defaultFetcher: McpFetcher = (url, init) =>
    fetch(url, init) as unknown as ReturnType<McpFetcher>;
