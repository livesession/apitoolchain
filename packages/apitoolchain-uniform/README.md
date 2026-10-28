# @xyd-js/uniform

uniform is a collection of common interfaces and utils for xyd api reference content.

## Agent emitters

Two emitters turn the REST operations of a `Reference[]` (what `@xyd-js/openapi`'s
`oapSchemaToReferences` returns) into what an AI agent consumes: MCP tool definitions and
an agent skill. Both are plain JavaScript — they never load `@xyd-js/native` and import no
Node builtins, so they run wherever the package installs (a server without the addon, a
browser, a build script). Each has a Rust twin in `crates/apitoolchain_uniform`
(`mcp::uniform_to_mcp_tools`, `skill::skill_files`) held to the same fixtures
(`__fixtures__/5.mcp-tools.*`, `__fixtures__/6.skill.*`).

Only REST operations (category `rest`, a `rest_<method>` type) are emitted; component
schemas and other categories are skipped. Output follows the references' order.

### MCP tools — `uniformToMcpTools`

```ts
import { deferencedOpenAPI, oapSchemaToReferences } from "@xyd-js/openapi";
import { uniformToMcpTools } from "@xyd-js/uniform";

const references = oapSchemaToReferences(await deferencedOpenAPI("openapi.yaml"));

const tools = uniformToMcpTools(references, {
    // a read-only POST besides every GET
    readOnly: (ref) => ref.type === "rest_get" || ref.canonical === "searchPets",
    limitCap: 50,
});
// tools[0] → {
//   name: "listPets", title: "List pets",
//   description: "Returns a page of pets, newest first. — GET /pets",
//   inputSchema: { type: "object", properties: { limit: {…}, status: {…}, … }, additionalProperties: false },
//   annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false },
//   binding: { method: "GET", path: "/pets", fullPath: "https://api.example.com/v1/pets",
//              pathParams: [], queryParams: ["limit", "status", …], body: null, group: ["pets"], canonical: "listPets" },
// }
```

- **name** — the canonical (an OpenAPI `operationId`) when it is a valid tool name
  (`^[A-Za-z0-9_-]{1,128}$`), else a slug of it; unique in reference order (`_2`, `_3`…).
  `toolName` overrides it.
- **description** — the description's first sentence (hard-wrapped lines joined first),
  cut to `descriptionChars` (default 240), then ` — METHOD path`.
- **inputSchema** — JSON Schema from the uniform property tree: types, enums, arrays,
  `anyOf`/`oneOf`, nullable, defaults, bounds, examples, required; descriptions keep their
  paragraphs and lists, with hard-wrapped lines joined. Objects deeper than `maxDepth`
  (default 7) lose their properties. `limitCap` caps an integer `limit` parameter (or
  top-level body property) unless its own `maximum` is lower: the maximum and a larger
  default come down to the cap, and the description ends "Capped at N by this server."
  in place of an "at most N" clause it carried.
- **the layouts** — by default (`flatten`) path, query and JSON-body properties sit side by
  side at the top level (`additionalProperties: false`); header and cookie parameters are
  left out. `flatten: false` groups the inputs under `pathParams`, `queryParams`,
  `headers`, `cookies` and `requestBody`.
- **annotations** — `readOnly` (default GET), `destructive` (DELETE), `idempotent` (GET,
  PUT, DELETE), `openWorld` (false): each a predicate over the reference.
- **binding** — how a call maps back onto the HTTP request; a server keeps it and strips
  it before listing the tool. `include` filters the operations, `extras` attaches data of
  your own to each tool (e.g. a documentation path).

Sending a call: fill `binding.path`'s `{placeholders}` from the `pathParams` arguments
and the query string from the `queryParams` ones. When `binding.body` is `"json"`, the
request body is:

- `arguments[binding.bodyProperty]`, sent as it is, when `bodyProperty` is set. It is set
  when the body cannot be flattened — it is not an object with known properties (an array,
  a union, a free-form object), or one of its properties shares a parameter's name — under
  the key `body` (`requestBody` if a parameter takes `body`), and always in the grouped
  layout (`requestBody`, with the parameters under `arguments.pathParams` /
  `arguments.queryParams`);
- otherwise (the flat layout) the object of every argument that is neither a path nor a
  query parameter.

```ts
// The flat layout (the default); in the grouped one, read the parameters from
// args.pathParams / args.queryParams instead.
function request(tool: McpTool, args: Record<string, unknown>) {
    const { binding } = tool;
    const url = new URL(
        binding.fullPath.replace(/\{([^}]+)\}/g, (_, name) => encodeURIComponent(String(args[name])))
    );
    for (const name of binding.queryParams) {
        if (args[name] !== undefined) url.searchParams.set(name, String(args[name]));
    }
    let body: unknown;
    if (binding.body === "json") {
        body = binding.bodyProperty
            ? args[binding.bodyProperty]
            : Object.fromEntries(
                  Object.entries(args).filter(
                      ([name]) => !binding.pathParams.includes(name) && !binding.queryParams.includes(name)
                  )
              );
    }
    return { method: binding.method, url, body };
}
```

### Agent skill — `skillFiles` from `@xyd-js/uniform/skill`

```ts
import { skillFiles } from "@xyd-js/uniform/skill";

const files = skillFiles(references, {
    name: "pet-store-api",
    description: "Use when working with the Pet Store API: pets, orders and the store inventory.",
    intro: "Every call needs `Authorization: Bearer <token>`.",
    extras: { "references/errors.md": "# Errors\n…" },
});
// Map {
//   "SKILL.md" → frontmatter (name, description), the intro, a table of the reference files,
//   "references/index.md" → every operation: canonical, method, path, group, title,
//   "references/pets.md", "references/store.md", … → one file per group,
//   "references/errors.md" → the extra, as given,
// }
for (const [path, contents] of files) {
    // write them under the skill's directory
}
```

Each group file holds, per operation, `## <canonical> — <title>`, the `METHOD path` line,
the description, tables of the path/query/header/cookie parameters and the request body
(nested objects one level down as `parent.child`), the responses (status, content type,
type, description) and one example request — a curl one when the references carry it.
`groupBy` (default `context.group[0]`, else `general`), `include`, `title`,
`references.dir` (default `references`) and `references.header` (the generated-file
comment) adjust the output; `intro` and `extras` are kept byte for byte.

`references: { perOperation: true }` writes one file per operation instead,
`references/<group>/<operation>.md` (the operation's sections one heading level up), and
the index links each operation to its file. The group directory is a slug of the group
name (lower-case ASCII); the operation file keeps its canonical's case, characters outside
`[A-Za-z0-9._-]` turned into `-`, and names that differ only in case get a `-2` suffix so
they stay apart on case-insensitive file systems.

### Installing

Use the published package (`@xyd-js/uniform` on npm), or a workspace / `link:` path to a
checkout of this repository after `pnpm build` — the package lives in a subdirectory of a
monorepo and its `dist/` is not committed, so a `github:` dependency on the repository
does not install it.

### Fixtures

The emitters' fixtures convert one sample document,
[`__fixtures__/agent-sample.openapi.yaml`](./__fixtures__/agent-sample.openapi.yaml), with
`@xyd-js/openapi` into each case's `input.json`
(`XYD_NATIVE=0 bun packages/apitoolchain-openapi/scripts/build-agent-fixture-inputs.ts`).
The oracles (`output.json`, `output/`) are this package's output
(`UNIFORM_BUILD_FIXTURES=1 pnpm vitest run`); the Rust tests compare against them as text.
