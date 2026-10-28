import { describe, expect, it } from "vitest";

import {
    MCP_PROTOCOL_VERSION,
    McpRpcError,
    resolveMcpSurface,
    type McpFetcher,
    type McpFetchResponse,
} from "../src/transport";

interface Call {
    method: string;
    rpc: string | undefined;
    headers: Record<string, string>;
    body: any;
    signal?: AbortSignal;
}

/** A streamable-HTTP server that speaks the lifecycle the spec asks for. */
function strictServer(options: { sse?: boolean; pages?: number; protocolVersion?: string } = {}) {
    const calls: Call[] = [];
    const sessionId = "session-1";
    const fetcher: McpFetcher = async (_url, init) => {
        const body = init.body ? JSON.parse(init.body) : undefined;
        calls.push({ method: init.method, rpc: body?.method, headers: init.headers, body, signal: init.signal });

        if (init.method === "DELETE") {
            return reply(200, undefined);
        }
        if (body.method === "initialize") {
            return reply(
                200,
                {
                    jsonrpc: "2.0",
                    id: body.id,
                    result: {
                        protocolVersion: options.protocolVersion || body.params.protocolVersion,
                        capabilities: { tools: {}, resources: {} },
                        serverInfo: { name: "strict", version: "1.0.0" },
                    },
                },
                { "mcp-session-id": sessionId },
            );
        }
        if (init.headers["Mcp-Session-Id"] !== sessionId) {
            return reply(400, { jsonrpc: "2.0", id: null, error: { code: -32000, message: "No valid session ID" } });
        }
        if (!body.id) {
            return reply(202, undefined);
        }
        if (body.method === "tools/list") {
            const page = body.params?.cursor ? Number(body.params.cursor) : 0;
            const last = (options.pages || 1) - 1;
            return reply(200, {
                jsonrpc: "2.0",
                id: body.id,
                result: {
                    tools: [{ name: `tool_${page}`, inputSchema: { type: "object" } }],
                    ...(page < last ? { nextCursor: String(page + 1) } : {}),
                },
            });
        }
        if (body.method === "resources/list") {
            return reply(200, {
                jsonrpc: "2.0",
                id: body.id,
                result: { resources: [{ uri: "docs://readme", name: "readme" }] },
            });
        }
        return reply(200, { jsonrpc: "2.0", id: body.id, error: { code: -32601, message: "Method not found" } });
    };

    function reply(status: number, message: unknown, extra: Record<string, string> = {}): McpFetchResponse {
        const sse = options.sse && message !== undefined && status === 200;
        const text =
            message === undefined
                ? ""
                : sse
                  ? // A server notification first, then the answer — as a server may stream them.
                    `event: message\ndata: ${JSON.stringify({ jsonrpc: "2.0", method: "notifications/message", params: {} })}\n\n` +
                    `event: message\ndata: ${JSON.stringify(message)}\n\n`
                  : JSON.stringify(message);
        const headers: Record<string, string> = {
            "content-type": sse ? "text/event-stream" : "application/json",
            ...extra,
        };
        return {
            ok: status >= 200 && status < 300,
            status,
            json: async () => JSON.parse(text),
            text: async () => text,
            headers: { get: (name: string) => headers[name.toLowerCase()] ?? null },
        };
    }

    return { calls, fetcher, sessionId };
}

describe("resolveMcpSurface — the streamable-HTTP lifecycle", () => {
    it("initializes, then lists with the session and protocol headers, then ends the session", async () => {
        const server = strictServer();
        const surface = await resolveMcpSurface("https://example.com/mcp", {
            token: "secret-token",
            fetcher: server.fetcher,
        });

        expect(surface.tools.map((t) => t.name)).toEqual(["tool_0"]);
        expect(surface.resources.map((r) => r.uri)).toEqual(["docs://readme"]);

        expect(server.calls[0].rpc).toBe("initialize");
        expect(server.calls[0].body.params.protocolVersion).toBe(MCP_PROTOCOL_VERSION);
        expect(server.calls[0].headers["Mcp-Session-Id"]).toBeUndefined();
        expect(server.calls[1].rpc).toBe("notifications/initialized");
        expect(server.calls[1].body.id).toBeUndefined();

        const lists = server.calls.filter((c) => c.rpc === "tools/list" || c.rpc === "resources/list");
        expect(lists).toHaveLength(2);
        for (const call of lists) {
            expect(call.headers["Mcp-Session-Id"]).toBe(server.sessionId);
            expect(call.headers["Mcp-Protocol-Version"]).toBe(MCP_PROTOCOL_VERSION);
        }

        const last = server.calls[server.calls.length - 1];
        expect(last.method).toBe("DELETE");
        expect(last.headers["Mcp-Session-Id"]).toBe(server.sessionId);

        for (const call of server.calls) {
            expect(call.headers.Authorization).toBe("Bearer secret-token");
            expect(call.signal).toBeInstanceOf(AbortSignal);
        }
    });

    it("sends the protocol version the server answered", async () => {
        const server = strictServer({ protocolVersion: "2025-03-26" });
        await resolveMcpSurface("https://example.com/mcp", { fetcher: server.fetcher });
        const list = server.calls.find((c) => c.rpc === "tools/list");
        expect(list?.headers["Mcp-Protocol-Version"]).toBe("2025-03-26");
    });

    it("reads answers streamed as server-sent events", async () => {
        const server = strictServer({ sse: true });
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher: server.fetcher });
        expect(surface.tools.map((t) => t.name)).toEqual(["tool_0"]);
        expect(surface.resources).toHaveLength(1);
    });

    it("follows nextCursor across pages", async () => {
        const server = strictServer({ pages: 3 });
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher: server.fetcher });
        expect(surface.tools.map((t) => t.name)).toEqual(["tool_0", "tool_1", "tool_2"]);
        const cursors = server.calls.filter((c) => c.rpc === "tools/list").map((c) => c.body.params?.cursor);
        expect(cursors).toEqual([undefined, "1", "2"]);
    });

    it("reads a server without initialize the way it always did", async () => {
        const calls: Call[] = [];
        const fetcher: McpFetcher = async (_url, init) => {
            const body = JSON.parse(init.body);
            calls.push({ method: init.method, rpc: body.method, headers: init.headers, body });
            const results: Record<string, unknown> = {
                "tools/list": { tools: [{ name: "echo" }] },
                "resources/list": { resources: [] },
            };
            if (!(body.method in results)) {
                return { ok: false, status: 404, json: async () => ({}) };
            }
            return { ok: true, status: 200, json: async () => ({ jsonrpc: "2.0", id: body.id, result: results[body.method] }) };
        };
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher });
        expect(surface.tools.map((t) => t.name)).toEqual(["echo"]);
        expect(calls.map((c) => c.rpc)).toEqual(["initialize", "tools/list", "resources/list"]);
        for (const call of calls.slice(1)) {
            expect(call.headers["Mcp-Protocol-Version"]).toBeUndefined();
            expect(call.headers["Mcp-Session-Id"]).toBeUndefined();
        }
    });

    it("treats a JSON-RPC 'method not found' on initialize as a server without it", async () => {
        const calls: string[] = [];
        const fetcher: McpFetcher = async (_url, init) => {
            const body = JSON.parse(init.body);
            calls.push(body.method);
            const answer =
                body.method === "initialize"
                    ? { jsonrpc: "2.0", id: body.id, error: { code: -32601, message: "Method not found" } }
                    : { jsonrpc: "2.0", id: body.id, result: body.method === "tools/list" ? { tools: [{ name: "echo" }] } : { resources: [] } };
            return { ok: true, status: 200, json: async () => answer };
        };
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher });
        expect(surface.tools.map((t) => t.name)).toEqual(["echo"]);
        expect(calls).toEqual(["initialize", "tools/list", "resources/list"]);
    });
});

/** A fetcher answering initialize from `initialize`, the rest from a strict server. */
function failingInitialize(initialize: Array<() => McpFetchResponse | Promise<McpFetchResponse>>) {
    const server = strictServer();
    let attempts = 0;
    const fetcher: McpFetcher = async (url, init) => {
        const body = init.body ? JSON.parse(init.body) : undefined;
        if (body?.method === "initialize" && attempts < initialize.length) {
            return initialize[attempts++]();
        }
        return server.fetcher(url, init);
    };
    return { fetcher, attempts: () => attempts, calls: server.calls };
}

const status = (code: number): McpFetchResponse => ({ ok: false, status: code, json: async () => ({}) });

describe("resolveMcpSurface — failures are not silent", () => {
    it("tries a 5xx initialize once more", async () => {
        const server = failingInitialize([() => status(503)]);
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher: server.fetcher });
        expect(surface.tools).toHaveLength(1);
        expect(server.attempts()).toBe(1);
    });

    it("throws when initialize fails twice, or is refused", async () => {
        const down = failingInitialize([() => status(502), () => status(502)]);
        await expect(resolveMcpSurface("https://example.com/mcp", { fetcher: down.fetcher })).rejects.toThrow(/HTTP 502/);

        const refused = failingInitialize([() => status(401)]);
        await expect(resolveMcpSurface("https://example.com/mcp", { fetcher: refused.fetcher })).rejects.toMatchObject({
            status: 401,
        });
        expect(refused.attempts()).toBe(1);

        const network = failingInitialize([
            () => Promise.reject(new TypeError("fetch failed")),
            () => Promise.reject(new TypeError("fetch failed")),
        ]);
        await expect(resolveMcpSurface("https://example.com/mcp", { fetcher: network.fetcher })).rejects.toBeInstanceOf(
            McpRpcError,
        );
    });

    it("propagates a strict server's refusal of a list", async () => {
        const server = strictServer();
        const fetcher: McpFetcher = async (url, init) => {
            const body = init.body ? JSON.parse(init.body) : undefined;
            if (body?.method === "tools/list") {
                return { ok: false, status: 400, json: async () => ({}), text: async () => '{"jsonrpc":"2.0","id":null,"error":{"code":-32000,"message":"Bad Request"}}' };
            }
            return server.fetcher(url, init);
        };
        await expect(resolveMcpSurface("https://example.com/mcp", { fetcher })).rejects.toMatchObject({
            status: 400,
            code: -32000,
        });
        // The session it opened is still ended.
        expect(server.calls[server.calls.length - 1].method).toBe("DELETE");
    });

    it("skips a list the server's capabilities leave out", async () => {
        const calls: string[] = [];
        const fetcher: McpFetcher = async (_url, init) => {
            const body = JSON.parse(init.body);
            calls.push(body.method);
            const result =
                body.method === "initialize"
                    ? { protocolVersion: MCP_PROTOCOL_VERSION, capabilities: { tools: {} }, serverInfo: { name: "t", version: "0" } }
                    : { tools: [{ name: "echo" }] };
            return { ok: true, status: 200, json: async () => ({ jsonrpc: "2.0", id: body.id, result }) };
        };
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher });
        expect(surface.resources).toEqual([]);
        expect(calls).toEqual(["initialize", "notifications/initialized", "tools/list"]);
    });

    it("throws with the cursor when a later page fails", async () => {
        const server = strictServer({ pages: 3 });
        const fetcher: McpFetcher = async (url, init) => {
            const body = init.body ? JSON.parse(init.body) : undefined;
            if (body?.method === "tools/list" && body.params?.cursor === "2") {
                return status(500);
            }
            return server.fetcher(url, init);
        };
        await expect(resolveMcpSurface("https://example.com/mcp", { fetcher })).rejects.toThrow(
            /at cursor "2", after 2 items/,
        );
    });
});

describe("resolveMcpSurface — streams and time", () => {
    it("stops reading an event stream at its answer, even when the server leaves it open", async () => {
        const server = strictServer();
        let cancelled = 0;
        const fetcher: McpFetcher = async (url, init) => {
            const body = init.body ? JSON.parse(init.body) : undefined;
            if (body?.method !== "tools/list") {
                return server.fetcher(url, init);
            }
            const events = [
                { jsonrpc: "2.0", method: "notifications/message", params: {} },
                { jsonrpc: "2.0", id: body.id, result: { tools: [{ name: "streamed" }] } },
            ];
            const stream = new ReadableStream<Uint8Array>({
                start(controller) {
                    for (const event of events) {
                        controller.enqueue(new TextEncoder().encode(`event: message\ndata: ${JSON.stringify(event)}\n\n`));
                    }
                    // …and never closed.
                },
                cancel() {
                    cancelled++;
                },
            });
            return {
                ok: true,
                status: 200,
                json: async () => ({}),
                text: () => new Promise<string>(() => undefined),
                headers: { get: (name) => (name.toLowerCase() === "content-type" ? "text/event-stream" : null) },
                body: stream,
            };
        };
        const surface = await resolveMcpSurface("https://example.com/mcp", { fetcher, timeoutMs: 2000 });
        expect(surface.tools.map((t) => t.name)).toEqual(["streamed"]);
        expect(cancelled).toBe(1);
    });

    it("gives up on a request after timeoutMs, signal or not", async () => {
        const server = strictServer();
        const hang: McpFetcher = async (url, init) => {
            const body = init.body ? JSON.parse(init.body) : undefined;
            if (body?.method === "tools/list") {
                return new Promise<McpFetchResponse>(() => undefined); // ignores the signal
            }
            return server.fetcher(url, init);
        };
        await expect(resolveMcpSurface("https://example.com/mcp", { fetcher: hang, timeoutMs: 100 })).rejects.toThrow(
            /no answer within 100 ms/,
        );
    });
});
