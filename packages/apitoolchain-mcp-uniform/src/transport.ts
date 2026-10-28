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
 * The protocol version `initialize` asks for. A server answers the version it
 * speaks; that answer is what the later requests carry in `Mcp-Protocol-Version`.
 */
export const MCP_PROTOCOL_VERSION = "2025-06-18";

/** What a fetcher answers. `json()` is enough for a server that answers plain
 *  JSON; `headers` and `text()` (both present on a `fetch` Response) let the
 *  transport keep a session (`Mcp-Session-Id`) and read an answer streamed as
 *  server-sent events. */
export interface McpFetchResponse {
    ok: boolean;
    status: number;
    json: () => Promise<unknown>;
    text?: () => Promise<string>;
    headers?: { get(name: string): string | null };
}

/**
 * Minimal JSON-RPC fetcher signature. Defaults to global fetch; tests inject a stub.
 * `body` is `""` for the `DELETE` that ends a session.
 */
export type McpFetcher = (
    url: string,
    init: { method: string; headers: Record<string, string>; body: string },
) => Promise<McpFetchResponse>;

export interface McpUrlToReferencesOptions {
    /** Bearer token sent as `Authorization: Bearer <token>`. */
    token?: string;
    /** Extra headers merged on top of the default JSON-RPC headers. */
    headers?: Record<string, string>;
    /** Override the network call (used by tests). */
    fetcher?: McpFetcher;
    /** The version `initialize` asks for (default {@link MCP_PROTOCOL_VERSION}). */
    protocolVersion?: string;
    /** `clientInfo` sent with `initialize`. */
    clientInfo?: { name: string; version: string };
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

        const session = await openSession(source, headers, options);
        try {
            const [toolsResult, resourcesResult] = await Promise.all([
                session.list<McpTool>("tools/list", "tools").catch(() => []),
                session.list<McpResource>("resources/list", "resources").catch(() => []),
            ]);
            tools = toolsResult;
            resources = resourcesResult;
        } finally {
            await session.close();
        }
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

/** Most pages a list call (`tools/list`, `resources/list`) follows through `nextCursor`. */
const MAX_LIST_PAGES = 100;

interface McpSession {
    /** Every item of a paginated list method, following `nextCursor`. */
    list<T>(method: string, key: string): Promise<T[]>;
    /** Ends the session the server opened, if it opened one (best effort). */
    close(): Promise<void>;
}

/**
 * The streamable-HTTP lifecycle: `initialize`, then `notifications/initialized`,
 * then every request with `Mcp-Protocol-Version` (the version the server
 * answered) and `Mcp-Session-Id` (when the server issued one). A server that
 * refuses `initialize` (an HTTP error or a JSON-RPC error) is read the way this
 * transport always read servers: plain requests without the handshake headers.
 */
async function openSession(
    url: string,
    baseHeaders: Record<string, string>,
    options: McpUrlToReferencesOptions,
): Promise<McpSession> {
    const fetcher = options.fetcher || defaultFetcher;
    const headers: Record<string, string> = { ...baseHeaders };
    let nextId = 1;
    let sessionId = "";

    async function send<T>(method: string, params?: unknown): Promise<{ result: T; response: McpFetchResponse }> {
        const id = nextId++;
        const body = JSON.stringify({ jsonrpc: "2.0", id, method, ...(params === undefined ? {} : { params }) });
        const response = await fetcher(url, { method: "POST", headers: { ...headers }, body });
        if (!response.ok) {
            throw new Error(`MCP RPC ${method} failed: ${response.status}`);
        }
        const json = await readResponse<T>(response, id);
        if (json.error) {
            throw new Error(`MCP RPC ${method} error: ${json.error.message}`);
        }
        return { result: (json.result ?? ({} as T)) as T, response };
    }

    const requested = options.protocolVersion || MCP_PROTOCOL_VERSION;
    let initialized = false;
    try {
        const { result, response } = await send<{ protocolVersion?: unknown }>("initialize", {
            protocolVersion: requested,
            capabilities: {},
            clientInfo: options.clientInfo || { name: "xyd-mcp-uniform", version: "0.1.0" },
        });
        const answered = typeof result?.protocolVersion === "string" ? result.protocolVersion : "";
        headers["Mcp-Protocol-Version"] = answered || requested;
        sessionId = response.headers?.get("mcp-session-id") || "";
        if (sessionId) {
            headers["Mcp-Session-Id"] = sessionId;
        }
        initialized = true;
    } catch {
        // Not an initialize-aware server: no handshake headers, as before.
    }
    if (initialized) {
        try {
            await fetcher(url, {
                method: "POST",
                headers: { ...headers },
                body: JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }),
            });
        } catch {
            // A notification has no answer to wait for; a failed one is not fatal.
        }
    }

    return {
        async list<T>(method: string, key: string): Promise<T[]> {
            const items: T[] = [];
            let cursor: string | undefined;
            for (let page = 0; page < MAX_LIST_PAGES; page++) {
                const { result } = await send<Record<string, unknown>>(method, cursor ? { cursor } : undefined);
                const pageItems = result?.[key];
                if (Array.isArray(pageItems)) {
                    items.push(...(pageItems as T[]));
                }
                const next = result?.nextCursor;
                if (typeof next !== "string" || !next || next === cursor) {
                    break;
                }
                cursor = next;
            }
            return items;
        },
        async close() {
            if (!sessionId) {
                return;
            }
            try {
                await fetcher(url, { method: "DELETE", headers: { ...headers }, body: "" });
            } catch {
                // Best effort: the server expires idle sessions on its own.
            }
        },
    };
}

/**
 * The JSON-RPC response with `id`, from a plain JSON answer or from a
 * server-sent-event stream (`Content-Type: text/event-stream`), where it may
 * follow notifications or requests of the server's own.
 */
async function readResponse<T>(response: McpFetchResponse, id: number): Promise<JsonRpcResponse<T>> {
    const contentType = response.headers?.get("content-type") || "";
    if (!response.text) {
        return (await response.json()) as JsonRpcResponse<T>;
    }
    const raw = await response.text();
    const messages: unknown[] = [];
    if (contentType.toLowerCase().includes("text/event-stream")) {
        for (const data of sseData(raw)) {
            try {
                messages.push(JSON.parse(data));
            } catch {
                // A keep-alive or a non-JSON event: not ours.
            }
        }
    } else {
        const parsed = JSON.parse(raw);
        messages.push(...(Array.isArray(parsed) ? parsed : [parsed]));
    }
    const answer = messages.find(
        (m): m is JsonRpcResponse<T> =>
            !!m && typeof m === "object" && (m as { id?: unknown }).id === id && ("result" in m || "error" in m),
    );
    if (!answer) {
        throw new Error(`MCP RPC: no response with id ${id}`);
    }
    return answer;
}

/** The `data` of each event in a server-sent-event stream (multi-line data joined by `\n`). */
function sseData(stream: string): string[] {
    const out: string[] = [];
    for (const event of stream.split(/\r?\n\r?\n/)) {
        const lines: string[] = [];
        for (const line of event.split(/\r?\n/)) {
            if (line.startsWith("data:")) {
                lines.push(line.slice(5).replace(/^ /, ""));
            }
        }
        if (lines.length) {
            out.push(lines.join("\n"));
        }
    }
    return out;
}

const defaultFetcher: McpFetcher = (url, init) =>
    fetch(url, init) as unknown as ReturnType<McpFetcher>;
