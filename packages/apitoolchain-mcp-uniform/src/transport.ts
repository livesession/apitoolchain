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

/** How long one request may take, its answer included (milliseconds). */
export const MCP_TIMEOUT_MS = 30_000;

/** What a fetcher answers. `json()` is enough for a server that answers plain
 *  JSON; `headers` and `text()` (both present on a `fetch` Response) let the
 *  transport keep a session (`Mcp-Session-Id`) and read an answer streamed as
 *  server-sent events, and `body` (a stream) lets it stop reading such a stream
 *  at the event that carries the answer. */
export interface McpFetchResponse {
    ok: boolean;
    status: number;
    json: () => Promise<unknown>;
    text?: () => Promise<string>;
    headers?: { get(name: string): string | null };
    body?: ReadableStream<Uint8Array> | null;
}

/**
 * Minimal JSON-RPC fetcher signature. Defaults to global fetch; tests inject a stub.
 * `body` is `""` for the `DELETE` that ends a session; `signal` aborts the
 * request when it runs out of time.
 */
export type McpFetcher = (
    url: string,
    init: { method: string; headers: Record<string, string>; body: string; signal?: AbortSignal },
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
    /** How long one request may take, its answer included (default {@link MCP_TIMEOUT_MS}). */
    timeoutMs?: number;
}

/**
 * A JSON-RPC request that failed: `status` is the HTTP status (0 for a network
 * failure or a timeout), `code` the JSON-RPC error code when the server sent one.
 */
export class McpRpcError extends Error {
    constructor(
        message: string,
        readonly method: string,
        readonly status: number,
        readonly code?: number,
    ) {
        super(message);
        this.name = "McpRpcError";
    }
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

/**
 * The tools and resources of an MCP server (a URL) or of a local manifest.
 *
 * A server that does not offer a list (no such capability, or it answers the
 * method as unknown) contributes none; any other failure — the server down, the
 * token refused, a request the server rejects, a timeout — is thrown, so a
 * docs build does not silently come out empty.
 */
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
            [tools, resources] = await Promise.all([
                session.list<McpTool>("tools/list", "tools", "tools"),
                session.list<McpResource>("resources/list", "resources", "resources"),
            ]);
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

/** Answers to `initialize` that mean "this server has no initialize": read it without the handshake. */
const NO_INITIALIZE_STATUS = new Set([404, 405, 501]);
/** JSON-RPC codes that mean the method is unknown or the request unsupported. */
const UNSUPPORTED_CODES = new Set([-32601, -32600]);

function unsupported(error: unknown): boolean {
    return (
        error instanceof McpRpcError &&
        (NO_INITIALIZE_STATUS.has(error.status) || (error.code !== undefined && UNSUPPORTED_CODES.has(error.code)))
    );
}

/** A failure worth one more try: no answer at all, a timeout or a 5xx (not 501). */
function transient(error: unknown): boolean {
    return (
        error instanceof McpRpcError &&
        error.code === undefined &&
        (error.status === 0 || error.status === 429 || (error.status >= 500 && error.status !== 501))
    );
}

interface McpSession {
    /**
     * Every item of a paginated list method, following `nextCursor`. A list the
     * server does not offer is empty; a failure on a later page is thrown with
     * the cursor it stopped at.
     */
    list<T>(method: string, key: string, capability: string): Promise<T[]>;
    /** Ends the session the server opened, if it opened one (best effort). */
    close(): Promise<void>;
}

/**
 * The streamable-HTTP lifecycle: `initialize`, then `notifications/initialized`,
 * then every request with `Mcp-Protocol-Version` (the version the server
 * answered) and `Mcp-Session-Id` (when the server issued one).
 *
 * A server whose `initialize` is unknown to it (HTTP 404, 405 or 501, or the
 * JSON-RPC errors -32601 / -32600) is read the way this transport always read
 * servers: plain requests without the handshake headers. A network failure, a
 * timeout or a 5xx is tried once more; anything else (401, 403, another 4xx,
 * another JSON-RPC error) is thrown.
 */
async function openSession(
    url: string,
    baseHeaders: Record<string, string>,
    options: McpUrlToReferencesOptions,
): Promise<McpSession> {
    const fetcher = options.fetcher || defaultFetcher;
    const timeoutMs =
        typeof options.timeoutMs === "number" && options.timeoutMs > 0 ? options.timeoutMs : MCP_TIMEOUT_MS;
    const headers: Record<string, string> = { ...baseHeaders };
    let nextId = 1;
    let sessionId = "";

    /** One fetch plus `use` of its answer, bounded by the timeout — even for a fetcher that ignores the signal. */
    async function exchange<R>(
        method: string,
        init: { method: string; body: string },
        use: (response: McpFetchResponse) => Promise<R>,
    ): Promise<R> {
        const signal = AbortSignal.timeout(timeoutMs);
        let timer: ReturnType<typeof setTimeout> | undefined;
        const expired = new Promise<never>((_, reject) => {
            timer = setTimeout(
                () => reject(new McpRpcError(`MCP RPC ${method} failed: no answer within ${timeoutMs} ms`, method, 0)),
                timeoutMs + 50,
            );
        });
        try {
            return await Promise.race([
                (async () => {
                    let response: McpFetchResponse;
                    try {
                        response = await fetcher(url, { ...init, headers: { ...headers }, signal });
                    } catch (error) {
                        throw new McpRpcError(`MCP RPC ${method} failed: ${reason(error, timeoutMs)}`, method, 0);
                    }
                    try {
                        return await use(response);
                    } catch (error) {
                        if (error instanceof McpRpcError) throw error;
                        throw new McpRpcError(`MCP RPC ${method} failed: ${reason(error, timeoutMs)}`, method, signal.aborted ? 0 : response.status);
                    }
                })(),
                expired,
            ]);
        } finally {
            clearTimeout(timer);
        }
    }

    async function send<T>(method: string, params?: unknown): Promise<{ result: T; response: McpFetchResponse }> {
        const id = nextId++;
        const body = JSON.stringify({ jsonrpc: "2.0", id, method, ...(params === undefined ? {} : { params }) });
        return exchange(method, { method: "POST", body }, async (response) => {
            if (!response.ok) {
                const code = await errorCode(response);
                throw new McpRpcError(`MCP RPC ${method} failed: HTTP ${response.status}`, method, response.status, code);
            }
            const json = await readResponse<T>(response, id);
            if (json.error) {
                throw new McpRpcError(`MCP RPC ${method} error: ${json.error.message}`, method, response.status, json.error.code);
            }
            return { result: (json.result ?? ({} as T)) as T, response };
        });
    }

    const requested = options.protocolVersion || MCP_PROTOCOL_VERSION;
    const initParams = {
        protocolVersion: requested,
        capabilities: {},
        clientInfo: options.clientInfo || { name: "xyd-mcp-uniform", version: "0.1.0" },
    };
    let capabilities: Record<string, unknown> | undefined;
    let initialized = false;
    for (let attempt = 1; ; attempt++) {
        try {
            const { result, response } = await send<{ protocolVersion?: unknown; capabilities?: unknown }>(
                "initialize",
                initParams,
            );
            const answered = typeof result?.protocolVersion === "string" ? result.protocolVersion : "";
            headers["Mcp-Protocol-Version"] = answered || requested;
            sessionId = response.headers?.get("mcp-session-id") || "";
            if (sessionId) {
                headers["Mcp-Session-Id"] = sessionId;
            }
            if (result?.capabilities && typeof result.capabilities === "object") {
                capabilities = result.capabilities as Record<string, unknown>;
            }
            initialized = true;
            break;
        } catch (error) {
            if (unsupported(error)) {
                break; // Not an initialize-aware server: no handshake headers, as before.
            }
            if (attempt === 1 && transient(error)) {
                continue;
            }
            throw error;
        }
    }
    if (initialized) {
        try {
            await exchange(
                "notifications/initialized",
                { method: "POST", body: JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) },
                async () => undefined,
            );
        } catch {
            // A notification has no answer to wait for; a failed one shows in the next request.
        }
    }

    return {
        async list<T>(method: string, key: string, capability: string): Promise<T[]> {
            // A server that declared its capabilities and left this one out has no such list.
            if (capabilities && !(capability in capabilities)) {
                return [];
            }
            const items: T[] = [];
            let cursor: string | undefined;
            for (let page = 0; page < MAX_LIST_PAGES; page++) {
                let result: Record<string, unknown>;
                try {
                    ({ result } = await send<Record<string, unknown>>(method, cursor ? { cursor } : undefined));
                } catch (error) {
                    if (page === 0 && unsupported(error)) {
                        return [];
                    }
                    if (page > 0 && error instanceof Error) {
                        throw new McpRpcError(
                            `${error.message} (at cursor ${JSON.stringify(cursor)}, after ${items.length} items)`,
                            method,
                            error instanceof McpRpcError ? error.status : 0,
                            error instanceof McpRpcError ? error.code : undefined,
                        );
                    }
                    throw error;
                }
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
                await exchange("DELETE", { method: "DELETE", body: "" }, async () => undefined);
            } catch {
                // Best effort: the server expires idle sessions on its own.
            }
        },
    };
}

function reason(error: unknown, timeoutMs: number): string {
    if (error instanceof Error && (error.name === "TimeoutError" || error.name === "AbortError")) {
        return `no answer within ${timeoutMs} ms`;
    }
    return error instanceof Error ? error.message : String(error);
}

/** The JSON-RPC error code of an error answer's body, when it has one. */
async function errorCode(response: McpFetchResponse): Promise<number | undefined> {
    try {
        const raw = response.text ? JSON.parse(await response.text()) : await response.json();
        const code = (raw as { error?: { code?: unknown } })?.error?.code;
        return typeof code === "number" ? code : undefined;
    } catch {
        return undefined;
    }
}

function isAnswer<T>(message: unknown, id: number): message is JsonRpcResponse<T> {
    return (
        !!message &&
        typeof message === "object" &&
        (message as { id?: unknown }).id === id &&
        ("result" in message || "error" in message)
    );
}

/**
 * The JSON-RPC response with `id`, from a plain JSON answer or from a
 * server-sent-event stream (`Content-Type: text/event-stream`), where it may
 * follow notifications or requests of the server's own. A stream is read only
 * up to that response: a server may leave it open.
 */
async function readResponse<T>(response: McpFetchResponse, id: number): Promise<JsonRpcResponse<T>> {
    const contentType = (response.headers?.get("content-type") || "").toLowerCase();
    const sse = contentType.includes("text/event-stream");
    if (sse && response.body && typeof response.body.getReader === "function") {
        return readEventStream<T>(response.body, id);
    }
    if (!response.text) {
        return (await response.json()) as JsonRpcResponse<T>;
    }
    const raw = await response.text();
    const messages: unknown[] = [];
    if (sse) {
        for (const data of sseData(raw)) {
            const message = parseJson(data);
            if (message !== undefined) messages.push(message);
        }
    } else {
        const parsed = JSON.parse(raw);
        messages.push(...(Array.isArray(parsed) ? parsed : [parsed]));
    }
    const answer = messages.find((m): m is JsonRpcResponse<T> => isAnswer<T>(m, id));
    if (!answer) {
        throw new Error(`no response with id ${id}`);
    }
    return answer;
}

/** Reads events off `stream` until one carries the response with `id`, then stops reading. */
async function readEventStream<T>(stream: ReadableStream<Uint8Array>, id: number): Promise<JsonRpcResponse<T>> {
    const reader = stream.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    try {
        for (;;) {
            const { done, value } = await reader.read();
            if (value) {
                buffer += decoder.decode(value, { stream: true });
            }
            for (let boundary = buffer.search(/\r?\n\r?\n/); boundary >= 0; boundary = buffer.search(/\r?\n\r?\n/)) {
                const event = buffer.slice(0, boundary);
                buffer = buffer.slice(boundary).replace(/^\r?\n\r?\n/, "");
                const message = eventMessage(event);
                if (isAnswer<T>(message, id)) {
                    return message;
                }
            }
            if (done) {
                const message = eventMessage(buffer);
                if (isAnswer<T>(message, id)) {
                    return message;
                }
                throw new Error(`no response with id ${id}`);
            }
        }
    } finally {
        reader.cancel().catch(() => undefined);
    }
}

function eventMessage(event: string): unknown {
    const [data] = sseData(event);
    return data === undefined ? undefined : parseJson(data);
}

function parseJson(text: string): unknown {
    try {
        return JSON.parse(text);
    } catch {
        return undefined; // A keep-alive or a non-JSON event: not ours.
    }
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
