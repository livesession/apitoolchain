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
  `canonical` = `objects/<Name>`, `context.group` = `["Objects"]` (see
  [Options](#options) and [`x-docs.group`](#on-a-component-schema)), one
  definition with the schema's property rows, the page's own
  [link](#schema-links) as its `symbolDef`, and a generated JSON example.

The `canonical` is the page's path under the docs route. An `x-docs.path` or
the `operationPaths: "title"` option replaces the default — see
[Page paths](#page-paths).

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
"symbolDef": { "id": "#/components/schemas/Widget", "canonical": "objects/Widget", "label": "widget" }
```

- directly (`definition: { $ref: '#/components/schemas/WidgetDefinition' }`),
- through a described or nullable wrapper (`allOf: [{ $ref: ... }]` +
  `description` / `nullable`),
- for an array of a component (the row and its `ofProperty` both link),
- on the branches of a `oneOf` / `anyOf` that are components (the union row
  itself does not link),
- on parameters whose `schema` is a `$ref`.

Inline schemas carry no link, and neither does an `allOf` of two or more
component schemas (a composition has no single page to point at). The link is
decided by identity — the schema object *is* the component — never by
containment, so a schema that merely holds a `$ref` does not link to its
member. A `$ref` with sibling keys (the OpenAPI 3.1 form) is merged into a new
schema by the dereferencer and does not link.

The same link sits on a **request or response root** (the variant's
`symbolDef`; an array root links to its items' schema) and on a **component
page's own definition**, so a renderer can name what an operation takes and
returns and what a page documents. An inline root keeps the legacy
`{ "id": [<member paths>] }` of a composition, or no `symbolDef` at all.

#### Labels

Every link to a component schema carries `label` — what a renderer prints where
it would otherwise print a bare `object`: the schema's constant `object`
property value when it has one (a single-value `enum` or a `const`, the "object
type" convention — `{ "object": "widget", ... }`), else the schema name. The
constant is read on the schema's own `properties`, else on the first `allOf`
member that declares one; a multi-value `object` enum or a non-string literal
is not a constant.

```yaml
Widget:            # label: widget
  properties:
    object: { type: string, enum: [widget] }
Material:          # label: Material (no `object` property)
  properties:
    name: { type: string }
WidgetList:        # label: list; `data` → label: widget on the row and its items
  properties:
    object: { type: string, enum: [list] }
    data: { type: array, items: { $ref: '#/components/schemas/Widget' } }
```

`label` is independent of `x-docs.name` (the page title); it is the short tag
for a type column.

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
| `path` | the page's path (its canonical) instead of the `operationId` — see [Page paths](#page-paths) |
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
| `group` | the resource group the page belongs to (the one its endpoints sit in). Under the default `objectsLayout: grouped` the page sits in that group's sub-group of the top-level `Objects` group (`context.group = ["Objects", "widgets"]`); under `per-area` it is nested under the group's own `Objects` sub-group, after its operations (`["widgets", "Objects"]`) |
| `hidden` | drop the page from the generated navigation (envelopes, errors) — a property that links to a hidden schema keeps its `symbolDef.id` and loses the `canonical`, so hide a schema only when no page should link to it |
| `path` | the page's path (its canonical) instead of `objects/<Name>` — see [Page paths](#page-paths) |
| `example` | a JSON example for the page (sidebar mode) |

The keys are read on the schema itself, else on the first `allOf` member that
carries them — in both modes. `hidden` applies to the *generated* navigation: an
`x-docs.sidebar` is a complete enumeration and still shows a page it lists.
Schema links follow the page: once the navigation is final, a
`symbolDef.canonical` — a nested property's, a request/response root's or a
page's own — is the canonical of the schema's own reference (a sidebar that
moves an object page under another path included), and a link to a schema
without a page (hidden, or not listed by a sidebar) keeps its `id` and `label`
and drops its `canonical` — a link never lands on a page that does not exist.

With `group`, the generated sidebar of the example above is — under the
default layout, `objectsLayout: grouped` —

```json
[
  { "group": "widgets", "pages": [ "…/listWidgets", "…/createWidget" ] },
  { "group": "Objects", "pages": [
      "…/objects/PartBase",
      { "group": "widgets", "pages": [ "…/objects/Widget" ] },
      { "group": "parts",   "pages": [ "…/objects/Part" ] } ] }
]
```

— ONE `Objects` group, the last group of the navigation whatever the tag
order: the pages without a `group` first, then one sub-group per resource
group in the order the groups appear above it (`widgets` has operations;
`parts` has none, so it follows, in schema order). The sub-groups are ordinary
nested sidebar groups: a renderer that starts nested groups collapsed (xyd
does, unless a group holds the active page) shows a single `Objects` entry
with a folded `widgets` and `parts` toggle inside. Under `objectsLayout:
per-area` the object pages join their resource groups instead:

```json
[
  { "group": "widgets", "pages": [ "…/listWidgets", "…/createWidget",
      { "group": "Objects", "pages": [ "…/objects/Widget" ] } ] },
  { "group": "parts",   "pages": [ { "group": "Objects", "pages": [ "…/objects/Part" ] } ] },
  { "group": "Objects", "pages": [ "…/objects/PartBase" ] }
]
```

— each group's operations, then a folded `Objects` sub-group; the schemas
without a `group` keep the top-level `Objects` group, last. In both layouts the
object pages sort after every operation (even one tagged with the objects
group's name) and keep the schema order among themselves. An `x-docs.sidebar`
entry places its pages itself; no sub-group is added there.

### Page paths

A page's `canonical` is its path under the docs route. Two things replace the
converter's default (`operationId`, `objects/<Name>`), both applied by the
x-docs pass (`uniform(refs, {plugins: [uniformPluginXDocsSidebar]})`, the
fused endpoint) with or without an `x-docs.sidebar`:

**`x-docs.path`** on an operation or a component schema names the path
outright — relative, no leading slash (outer slashes and repeated slashes are
normalised away):

```yaml
paths:
  /widgets/{widgetId}:
    delete:
      summary: Delete widget
      x-docs:
        path: widgets/remove        # the page is …/widgets/remove
components:
  schemas:
    Assembly:
      x-docs:
        group: assemblies
        path: assemblies/object     # the page is …/assemblies/object
```

**`operationPaths: "title"`** (an [option](#options); the default
`"operation-id"` is today's behaviour) derives a path for every page that has
no `x-docs.path`:

- An **operation** → `<group>/[<sub-resource>/]<verb>` from its group (its
  `x-docs.group`, else its first tag, slugged) and a *verb-first* summary:

  ```text
  summary   := Verb Rest
  Verb      := the first word, lowercased, one of the verbs below; a third-person
               "s"/"es" is dropped ("Lists" → list, "Searches" → search)
  Rest      := the remaining words minus the function words (a, an, the, all, of,
               for, by, to, in, on, from, with) and the group's noun — every word
               of the group, singular and plural alike; what is left, lowercased
               and hyphenated, is the sub-resource
  ```

  In group `widgets`: "Create widget" → `widgets/create`, "List widget parts" →
  `widgets/parts/list`, "Retrieve part snapshot" →
  `widgets/part-snapshot/retrieve`, "Lists all widgets" → `widgets/list`. A
  summary that is not verb-first ("Widget archival") and an operation without a
  group keep the default canonical (the `operationId`).

  The verbs: accept, activate, add, approve, archive, assign, attach, cancel,
  check, clear, clone, close, complete, confirm, copy, count, create, deactivate,
  decline, delete, detach, disable, download, duplicate, enable, execute, export,
  fetch, finalize, find, generate, get, import, invite, link, list, lock, mark,
  merge, move, open, patch, pause, preview, publish, query, read, refresh,
  regenerate, register, reject, release, remove, rename, reopen, replace, reset,
  resend, resolve, restore, resume, retrieve, retry, revoke, rotate, run, search,
  send, set, share, start, stop, submit, subscribe, suspend, sync, test, toggle,
  trigger, unarchive, unassign, unlink, unlock, unpublish, unsubscribe, update,
  upload, upsert, validate, verify, void (`page_paths::VERBS` /
  `PAGE_PATH_VERBS`).
- A **component schema** with an `x-docs.group` → `<group>/<label>`, without
  one → `objects/<label>`, where the label is the schema's constant `object`
  value **verbatim** (the [link label](#labels): `widgets/widget`,
  `widgets/widget_part`, `objects/company_statistics`), else the schema name in
  kebab-case (`PartSnapshot` → `widgets/part-snapshot`, `Color` →
  `objects/color`).

Whatever decides the path, the sidebar grouping is unchanged (groups still come
from the tags, `x-docs.group` and the objects layout), and every link that
targets the page — a nested property's `symbolDef.canonical`, a request or
response root's, the page's own, the sidebar entry, the page map — follows the
final canonical, in whole-document and in regions mode alike.

**Collisions.** Two pages of the generated navigation at one path — two equal
`x-docs.path`s, or two summaries that derive the same canonical — are an
**error** naming both pages (`two pages share the path "widgets/list": GET
/widgets ("List widgets") and GET /widgets/all ("List all widgets")`); give
one of them an `x-docs.path`. Under an `x-docs.sidebar` the entries place the
pages themselves: an entry's own `path` wins over the page's `x-docs.path`, an
entry without one leaves the page at it, and no collision check runs.

## Options

`oapSchemaToReferences(doc, options)` / the Rust `Options` (JSON through the
native addon, camelCase):

| option | what |
|---|---|
| `regions` | convert only these regions: `"METHOD /path"` or `"/components/schemas/Name"` (the per-page conversion) |
| `objectsGroup` | the name of the component-schema pages' group, default `"Objects"` — the top-level objects group and, under `per-area`, the sub-group under a resource group (`x-docs.group`); an empty string means the default |
| `objectsLayout` | where an object page with a resource group (`x-docs.group`) goes: `"grouped"` (the default) — one top-level objects group, last, holding the pages without a group and then a sub-group per resource group in the order the groups appear above; `"per-area"` — nested under the resource group's own objects sub-group, after its operations. Anything else means the default (the Rust `ObjectsLayout` enum, `Options::objects_layout()`) |
| `operationPaths` | how a page without an `x-docs.path` gets its canonical: `"operation-id"` (the default) — the `operationId` / `objects/<Name>`; `"title"` — derived from the group and the verb-first summary / the schema label, see [Page paths](#page-paths). Anything else means the default (the Rust `OperationPaths` enum, `Options::operation_paths()`) |

The fused endpoint (`fused::uniform_oas_pages`, `FusedInput::objects_group` /
`FusedInput::objects_layout` / `FusedInput::operation_paths`) takes the same
names, since it builds the sidebar. The Rust x-docs pass
(`xdocs::apply_xdocs_sidebar_with_options`) returns `Err` on a page-path
collision; the JS plugin throws.

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
