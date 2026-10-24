# @xyd-js/openapi

OpenAPI 3.x → Uniform `Reference[]`. The conversion runs in the Rust core
(`crates/apitoolchain_openapi`, reached through `@xyd-js/native`) for local files
and in the frozen JavaScript implementation (`src/impl-js`) for URL sources; the
two are held to byte-parity by the fixtures under `__fixtures__/` — `cargo test`
on the Rust side (`tests/parity.rs`, `tests/fused.rs`), `vitest` through this shim.

## What a reference carries

- One `Reference` per operation: `title` = the summary (else the `operationId`,
  else `METHOD/path`), `canonical` = the `operationId` (else a slug of the
  summary), `context.group` = the first tag. Its definitions are the parameter
  groups, the request body and the response, each a list of property rows.
- One `Reference` per `components.schemas` entry: `title` = the schema name,
  `canonical` = `objects/<Name>`, `context.group` = `["Objects"]`, one definition
  with the schema's property rows and a generated JSON example.

A property row (`DefinitionProperty`) has a `name`, a `type` (a JSON type or one
of the `$$union` / `$$xor` / `$$array` / `$$enum` markers), a `description`, a
`meta` list, child `properties`, an `ofProperty` (the items of an array, the
base type of an enum) and, where it applies, a `symbolDef` link.

### Constants

A property with a single-value `enum`, or a `const` (OpenAPI 3.1), is a `$$enum`
row with one value child, the `enum-type` meta, and the meta
`{ name: "const", value: <literal> }` — the literal a renderer prints inline:

```yaml
object:
  type: string
  enum: [widget]
  description: The object type, always `widget`.
```

```json
{ "name": "object", "type": "$$enum", "meta": [
    { "name": "required", "value": "true" },
    { "name": "enum-type", "value": "string" },
    { "name": "const", "value": "widget" } ],
  "properties": [ { "name": "widget", "type": "string" } ],
  "ofProperty": { "type": "string" } }
```

Scalar literals keep their type (`"widget"`, `2`, `true`); an object or array
literal is JSON-stringified, like the `example` meta. Multi-value enums are
unchanged (no `const`).

### Schema links

A nested property whose schema **is** a component schema links to that schema's
page:

```json
"symbolDef": { "id": "#/components/schemas/WidgetDefinition", "canonical": "objects/WidgetDefinition" }
```

- directly (`definition: { $ref: '#/components/schemas/WidgetDefinition' }`),
- through a described wrapper (`allOf: [{ $ref: ... }]` + `description`),
- for an array of a component (the row and its `ofProperty` both link),
- on parameters whose `schema` is a `$ref`.

Inline schemas carry no link, and neither does an `allOf` of two or more
component schemas (a composition has no single page to point at). The link is
decided by identity — the schema object *is* the component — never by
containment, so a schema that merely holds a `$ref` does not link to its
member. A `$ref` with sibling keys (the OpenAPI 3.1 form) is merged into a new
schema by the dereferencer and does not link.

### `allOf` with one scalar member

```yaml
id:
  allOf:
    - $ref: '#/components/schemas/WidgetId'   # { type: string, format: uuid }
  description: Unique identifier for the widget.
```

is a `string` row with the wrapper's description, the member's meta and the link
above — never a parent with a nameless child. An enum member folds its values
in (the row becomes `$$enum`); when the wrapper declares no `type`, the member's
decides; the wrapper's own meta wins over the member's by name.

## `x-docs`

The `x-docs` extension customises the generated reference beyond what the
specification carries. It is read with or without an `x-docs.sidebar`.

### On the document

| key | what |
|---|---|
| `route` | the base path the pages are served from (`docs.json` wins when it sets one) |
| `codeLanguages` | the request code-sample languages, in order |
| `sidebar` | a complete navigation: groups of `{ type: endpoint \| object, key, path }` pages — every page not listed is dropped |
| `sidebarPathStrategy` | `inherit` builds page paths from the operation path |

### On an operation

| key | what |
|---|---|
| `name` | the reference title (page title and sidebar label) instead of the summary |
| `group` | the sidebar group instead of the first tag |
| `hidden` | drop the page from the generated navigation |
| `returns` | replaces the response definition with this description |
| `examples` | request / response samples |

### On a component schema

```yaml
components:
  schemas:
    Widget:
      x-docs:
        name: The widget object
        group: widgets          # beside the `widgets` endpoints, after them
      type: object
      properties: ...
    WidgetList:
      x-docs:
        hidden: true            # an envelope: no page
      type: object
      properties: ...
    Part:
      allOf:
        - $ref: '#/components/schemas/PartBase'
        - x-docs:               # a composition documents itself on a member
            name: The part object
            group: parts
          type: object
          properties: ...
```

| key | what |
|---|---|
| `name` | the reference title instead of the schema name |
| `group` | the sidebar group instead of `Objects` — what puts an object page beside its resource's endpoints |
| `hidden` | drop the page from the generated navigation (envelopes, errors) — a property that links to a hidden schema keeps its `symbolDef.id` and loses the `canonical`, so hide a schema only when no page should link to it |
| `example` | a JSON example for the page (sidebar mode) |

The keys are read on the schema itself, else on the first `allOf` member that
carries them — in both modes. `hidden` applies to the *generated* navigation: an
`x-docs.sidebar` is a complete enumeration and still shows a page it lists.
Schema links follow the page: once the navigation is final, a nested
`symbolDef.canonical` is the canonical of the schema's own reference (a sidebar
that moves an object page under another path included), and a link to a schema
without a page (hidden, or not listed by a sidebar) keeps its `id` and drops its
`canonical` — a link never lands on a page that does not exist.

## Tests

```bash
cargo test -p apitoolchain_openapi                 # unit + the parity and fused tiers
XYD_NATIVE=0 vitest run packages/apitoolchain-openapi   # the JS impl against the same oracles
vitest run packages/apitoolchain-openapi           # the native path (a stale addon skips the
                                                   # fixtures it cannot produce, see the probe
                                                   # in __tests__/oapSchemaToReferences.test.ts)
```

Oracle regeneration is explicit: `OAS_BUILD_FIXTURES=1 XYD_NATIVE=0 vitest run
...` rewrites `output.json`; `XYD_NATIVE=0 bun scripts/build-fused-goldens.ts`
rewrites the `fused.golden.json` files (conversion and the x-docs plugin from
the JS impl, `pluginNavigation` from the addon — it has no JS impl any more).
