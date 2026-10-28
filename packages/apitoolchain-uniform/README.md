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
- **description** — the description's first sentence, cut to `descriptionChars` (default
  240), then ` — METHOD path`. Hard-wrapped lines are joined first (fenced code, headings
  and lists keep their lines; as in CommonMark, only an item numbered 1 starts a list
  inside a paragraph, so a wrapped "…at most\n5000. …" stays one sentence). A sentence ends at `.`, `!` or `?` followed by the end of
  the line, or by whitespace and an uppercase letter, a digit, a quote or a bracket — never
  at the period of an abbreviation (`e.g.`, `i.e.`, an initial, `No.`, `vs.`, `cf.`…).
- **inputSchema** — JSON Schema from the uniform property tree: types, enums, arrays,
  `anyOf`/`oneOf`, nullable, defaults, bounds, examples, required. A top-level input is at
  depth 1; an object deeper than `maxDepth` (default 7) is emitted as `{ type: "object" }`
  without its properties. A nullable enum ends with a real `null` (a `"null"` member the
  converter emits for a `null` enum value is dropped for it). `limitCap` caps an integer
  `limit` parameter (or top-level body property) unless its own `maximum` is lower: the
  maximum and a larger default come down to the cap, and the description ends "Capped at
  N by this server." in place of an "at most N" clause it carried (removed with the comma,
  semicolon or `and`/`or`/`but` that joined it).
- **the layouts** — by default (`flatten`) path, query and JSON-body properties sit side by
  side at the top level; header and cookie parameters are left out. An input that shares
  an earlier input's name (path parameters first, then query parameters, then body
  properties) is renamed `<name>_query` / `<name>_body` (`_2`, `_3`… if taken) and mapped
  back in `binding.inputs`. The top level is closed (`additionalProperties: false`) unless
  the flattened body declares itself open: an `additionalProperties` meta (`true` or a
  JSON Schema) on its variant or definition is carried through. An optional body's
  required properties are not required inputs (they are required only when a body is
  sent). `flatten: false` groups the inputs under `pathParams`, `queryParams`, `headers`,
  `cookies` and `requestBody`, nothing renamed.
- **operations** — every REST operation whose request body, if it has one, has a JSON
  variant; one that only takes a form, multipart or binary body is skipped. `include`
  replaces that rule: an operation it lets in without a JSON body becomes a tool with no
  body input and `binding.body: null`.
- **annotations** — `readOnly` (default GET, HEAD, OPTIONS, TRACE), `destructive` (DELETE,
  PUT, PATCH — they remove or overwrite; POST only adds), `idempotent` (the read-only
  methods, PUT, DELETE), `openWorld` (false): each a predicate over the reference.
- **binding** — how a call maps back onto the HTTP request; a server keeps it and strips
  it before listing the tool. `binding.layout` is `"flat"` or `"grouped"` (the `flatten`
  option). `extras` attaches data of your own to each tool (e.g. a documentation path).

Sending a call: read the arguments by `binding.layout` — never by guessing from the input
property names, since a flat body may well have a property called `headers`.
`binding.pathParams` and `binding.queryParams` list INPUT names; an input
listed in `binding.inputs` goes out under the `name` given there (`{ id_query: { in:
"query", name: "id" } }` → the query parameter `id`), every other one under its own name.
Fill `binding.path`'s `{placeholders}` from the path inputs and the query string from the
query inputs. When `binding.body` is `"json"`, the request body is:

- `arguments[binding.bodyProperty]`, sent as it is, when `bodyProperty` is set. It is set
  when the body cannot be flattened — it is not an object with known properties (an array,
  a union, a free-form object) — under the key `body` (`requestBody` if a parameter takes
  `body`), and always in the grouped layout (`requestBody`, with the parameters under
  `arguments.pathParams` / `arguments.queryParams`);
- otherwise (the flat layout) the object of every argument that is neither a path nor a
  query input, each under its wire name.

```ts
// The flat layout (the default, binding.layout === "flat"); in the grouped one,
// read the parameters from args.pathParams / args.queryParams instead.
function request(tool: McpTool, args: Record<string, unknown>) {
    const { binding } = tool;
    const wire = (input: string) => binding.inputs?.[input]?.name ?? input;
    let path = binding.fullPath;
    for (const input of binding.pathParams) {
        path = path.replace(`{${wire(input)}}`, encodeURIComponent(String(args[input])));
    }
    const url = new URL(path);
    for (const input of binding.queryParams) {
        if (args[input] !== undefined) url.searchParams.set(wire(input), String(args[input]));
    }
    let body: unknown;
    if (binding.body === "json") {
        body = binding.bodyProperty
            ? args[binding.bodyProperty]
            : Object.fromEntries(
                  Object.entries(args)
                      .filter(([input]) => !binding.pathParams.includes(input) && !binding.queryParams.includes(input))
                      .map(([input, value]) => [wire(input), value])
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
type, description — then a response's fields, one level deep) and one example request.
`responseFields` picks the responses whose fields are tabulated: `"success"` (the default)
the 2xx ones, `"all"` every one, `"none"` none; the others stay one summary row each. An
error envelope every operation shares is best described once, in a file passed as an
extra, rather than under every operation and status.
`exampleRequest` picks it: `"auto"` (the default) takes the first request example the
references carry — a curl one when there is one — and synthesizes a curl request when
there is none; `"synthesized"` always synthesizes it; `"none"` leaves it out. A
synthesized request sends only the required inputs plus `limit`, each with its example or
default when it states one (`limit`: else 10) and a typed placeholder otherwise (`<name>`,
`0`, `false`) — so optional aliases of a required input stay out of it.
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

`5.mcp-tools.open-body` is hand-written: it pins the `additionalProperties` meta, which the
OpenAPI converter does not emit yet. The other emitter fixtures convert one sample document,
[`__fixtures__/agent-sample.openapi.yaml`](./__fixtures__/agent-sample.openapi.yaml), with
`@xyd-js/openapi` into each case's `input.json`
(`XYD_NATIVE=0 bun packages/apitoolchain-openapi/scripts/build-agent-fixture-inputs.ts`).
The oracles (`output.json`, `output/`) are this package's output
(`UNIFORM_BUILD_FIXTURES=1 pnpm vitest run`); the Rust tests compare against them as text.
