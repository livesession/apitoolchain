# @xyd-js/mcp-uniform

Reads an MCP server's surface — its tools and resources — and converts it into uniform
`Reference[]`, one reference per tool (`mcp_tool`) and per resource (`mcp_resource`), so an
MCP server can be documented like any other API.

```ts
import { mcpUrlToReferences } from "@xyd-js/mcp-uniform";

const references = await mcpUrlToReferences("https://mcp.example.com/mcp", {
    token: process.env.MCP_TOKEN, // sent as `Authorization: Bearer <token>`
    headers: { "X-Custom": "value" },
});
```

The source is either a URL or the path of a local JSON manifest shaped like the combined
answers of `tools/list` and `resources/list` (`{ tools, resources?, serverUrl? }`).

The conversion runs in the Rust core through `@xyd-js/native` (there is no JavaScript
fallback); the transport below is plain JavaScript.

## The transport

A URL is read over [streamable HTTP](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http):

1. `initialize` (asking for `MCP_PROTOCOL_VERSION`, `2025-06-18`, or `protocolVersion`),
   then the `notifications/initialized` notification;
2. `tools/list` and `resources/list`, each following `nextCursor` across pages, with
   `Mcp-Protocol-Version: <the version the server answered>` and, when the server issued
   one, `Mcp-Session-Id`;
3. a `DELETE` that ends the session, when there is one.

Answers may be plain JSON or a `text/event-stream`; the response is matched by its id, and an
event stream is read only up to the event that carries it (a server may leave the stream open).
Every request — the `DELETE` included — is abandoned after `timeoutMs` (default 30 s).

Failures are not silent:

- a server whose `initialize` is unknown to it (HTTP 404, 405 or 501, or the JSON-RPC errors
  -32601 / -32600) is read without the handshake headers, the way earlier versions read every
  server;
- a network failure, a timeout or a 5xx on `initialize` is tried once more, then thrown;
  a 401, a 403 or any other error is thrown at once;
- a list the server does not offer (left out of its capabilities, or answered as an unknown
  method) is empty; any other failure of a list is thrown (`McpRpcError`, with the HTTP
  `status` and the JSON-RPC `code`), and a failure on a later page names the cursor.

Every request carries `Accept: application/json, text/event-stream`, the bearer token and
the extra `headers`. `fetcher` replaces the network call (tests inject one); it receives
`{ method, headers, body, signal }` and answers `{ ok, status, json(), text?(), headers?, body? }`
— a `fetch` `Response` fits.
