import { describe, expect, it } from "vitest";

import { MCP_PROTOCOL_VERSION, resolveMcpSurface, type McpFetcher, type McpFetchResponse } from "../src/transport";

interface Call {
    method: string;
    rpc: string | undefined;
    headers: Record<string, string>;
    body: any;
}

/** A streamable-HTTP server that speaks the lifecycle the spec asks for. */
function strictServer(options: { sse?: boolean; pages?: number; protocolVersion?: string } = {}) {
    const calls: Call[] = [];
    const sessionId = "session-1";
    const fetcher: McpFetcher = async (_url, init) => {
        const body = init.body ? JSON.parse(init.body) : undefined;
        calls.push({ method: init.method, rpc: body?.method, headers: init.headers, body });

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
});
