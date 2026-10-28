# @xyd-js/openapi

## Unreleased

Both halves of the converter (the Rust core in `crates/apitoolchain_openapi`
and the JS impl in `src/impl-js`) change together; the fixtures `9.schema-links`,
`5.xdocs.objects` (the per-area objects layout), `11.objects-layouts` (the
grouped default) and `12.page-paths` (explicit and title-derived page paths)
pin the new output on both sides, and the fused goldens of every
grouped-layout case pin the `collapsed` flag.

### Added

- **`additionalProperties` meta**: a schema that sets `additionalProperties`
  (anything but `false`) gives its property, and a request body gives its
  variant, the meta `{ name: "additionalProperties", value }` — `true` for
  `true` or a schema without a `type`, else `{ type }` (the value schema's
  type, `"null"` added for `nullable: true`; a looser form that never rejects
  what the schema accepts — nested shapes are not copied). Fixture
  `13.open-objects-null-enums`; the `-2.complex.openai` oracle gains 153 of
  these metas and nothing else.

- **Explicit page paths — `x-docs.path`** on an operation or a component
  schema (read on the schema or on the first `allOf` member that carries it):
  the page's canonical becomes that path, relative and without a leading
  slash (`widgets/create`, `widgets/parts/list`, `widgets/object`). Every link
  to the page — a nested `symbolDef.canonical`, a request/response root's, the
  page's own, the sidebar, the page map — follows the final canonical; the
  sidebar grouping is unchanged. Under an `x-docs.sidebar` an entry's own
  `path` wins; an entry without one leaves the page at its `x-docs.path`.
  `XDocsMeta.path`.
- **A derived-canonical option — `operationPaths`**
  (`uniformOasOptions.operationPaths`, the Rust `Options::operation_paths` /
  `FusedInput::operation_paths` and the `OperationPaths` enum, JSON
  `operationPaths`): `"operation-id"` (the default, today's canonicals) or
  `"title"`, under which a page without an `x-docs.path` derives its path —
  an operation `<group>/[<sub-resource>/]<verb>` from its group and a
  verb-first summary ("Create widget" in `widgets` → `widgets/create`, "List
  widget parts" → `widgets/parts/list`, "Retrieve part snapshot" →
  `widgets/part-snapshot/retrieve`; the grammar and the verb table are in the
  README and in `page_paths.rs` / `xdocs/pagePaths.ts`), falling back to the
  `operationId` when the summary is not verb-first or the operation has no
  group; a component page `<group>/<label>` with an `x-docs.group`, else
  `objects/<label>`, the label its constant `object` value verbatim
  (`widgets/widget`, `widgets/widget_part`) else its name in kebab-case
  (`objects/color`). Applied by the x-docs pass, in whole-document and regions
  mode alike.
- **Page-path collisions are an error.** Two pages of the generated navigation
  at one path — two equal `x-docs.path`s, or two summaries that derive the
  same canonical — fail the pass naming both pages (`two pages share the path
  "widgets/list": GET /widgets ("List widgets") and GET /widgets/all ("List all
  widgets")`). The Rust `xdocs::apply_xdocs_sidebar` /
  `apply_xdocs_sidebar_with_options` now return `Result<(), OasError>` (the
  fused endpoint propagates it); the JS plugin throws. An `x-docs.sidebar`
  places its pages itself and is not checked.
- `xdocs::apply_xdocs_links_with_options` — the regions-mode relink with the
  options (the derived paths the links follow); `apply_xdocs_links` keeps the
  defaults.
- **A constant's value.** A property with a single-value `enum` or a `const`
  carries the meta `{ name: "const", value: <literal> }` beside its `$$enum`
  children and `enum-type` (scalars as they are, object/array literals
  JSON-stringified like `example`). `const` (OpenAPI 3.1) is now read at all —
  it used to be dropped. Multi-value enums are unchanged.
- **Schema links on nested properties.** A property whose schema is a component
  schema gets `symbolDef: { id: "#/components/schemas/<Name>", canonical:
  "objects/<Name>" }` — directly, through `allOf: [$ref]`, for an array of a
  component (the row and its `ofProperty`) and on `$ref` parameters. Decided by
  identity, never by containment; a composition of two or more components does
  not link.
- **`x-docs` on component schemas outside sidebar mode.** `name` → the reference
  title, `group` → the sidebar group, `hidden` → no page; read on the schema or
  on the first `allOf` member that carries them. `x-docs.hidden` on operations
  too. An `x-docs.sidebar` that lists a hidden page still shows it.
- `XDocsMeta` / `XDocsOperationMeta` / `XDocsSchemaMeta` types.
- **A label on every schema link.** A `symbolDef` that points at a component
  schema carries `label`: the schema's constant `object` property value when
  it has one (a single-value `enum` or a `const` — read on the schema or on
  the first `allOf` member that declares one), else the schema name. A
  renderer prints it where it would otherwise print a bare `object`
  (`data: array of widget`). `SymbolDef.label` in `@xyd-js/uniform`.
- **Links on the request/response roots and on the component pages.** A
  request-body or response variant whose schema is a component schema (an
  array root: its items) carries that schema's link as its `symbolDef`
  (`id`, `canonical`, `label`) instead of nothing; a component page's
  definition carries the page's own link. An inline composition keeps the
  legacy `{ id: [<member paths>] }`. The x-docs pass relinks these like the
  nested ones (a hidden page's link loses its `canonical`).
- **Object pages grouped by their resource group — the `objectsLayout`
  option** (`uniformOasOptions.objectsLayout`, the Rust
  `Options::objects_layout` / `FusedInput::objects_layout` and the
  `ObjectsLayout` enum, JSON `objectsLayout`). A component schema's
  `x-docs.group` names the resource group its page belongs to; the layout
  says where that page goes in the generated navigation:
  - `"grouped"` — **the default**: ONE top-level `Objects` group, the last
    group of the navigation whatever the tag order, holding the pages without
    a resource group first and then one sub-group per resource group in the
    order the groups appear above it (`context.group = ["Objects",
    <group>]`; a group with no operations follows, in schema order). A
    renderer that starts nested groups collapsed shows a single `Objects`
    entry with a folded toggle per resource group.
  - `"per-area"`: the page is nested under its resource group's own `Objects`
    sub-group, AFTER the group's operations (`context.group = [<group>,
    "Objects"]`); the schemas without a resource group keep the top-level
    `Objects` group.

  An `x-docs.sidebar` entry is placed where it is listed; an operation's
  `x-docs.group` is never nested. Anything but `"per-area"` means the default.
- **`objectsGroup` option** (`uniformOasOptions.objectsGroup`, the Rust
  `Options::objects_group` / `FusedInput::objects_group`, JSON
  `objectsGroup`): renames the component-schema pages' group — the top-level
  group and the per-area sub-group alike; default `"Objects"`.
- **The Objects group collapsible, collapsed by default — the
  `objectsCollapsed` option** (`uniformOasOptions.objectsCollapsed`, the Rust
  `Options::objects_collapsed` / `FusedInput::objects_collapsed`, JSON
  `objectsCollapsed`; default `true`). Under the grouped layout the generated
  navigation's TOP-LEVEL objects group entry carries `"collapsed": true` —
  `{ "group": "Objects", "collapsed": true, "pages": [...] }` — so a renderer
  can draw a toggle on the group header and start the group folded, like the
  area sub-groups inside it. The flag sits on that one entry only: never on an
  operation group or a sub-group, never under `per-area` or with an
  `x-docs.sidebar`, and only when the group holds object pages alone — an
  operation tagged with the objects group's name shares the group and keeps it
  open. `objectsCollapsed: false` leaves the group open. The flag is set by the
  navigation's finishing pass, run by the fused endpoint after
  pluginNavigation (the Rust `navigation::collapse_objects_group`; the JS twin
  `collapseObjectsGroup(sidebar, doc, references, urlPrefix, options)`, also
  exported, builds the fused goldens).

### Changed

- **A `null` enum member** is no longer a value row named `"null"`: it is
  dropped from the children and the enum gets the `nullable` meta (once,
  beside an explicit `nullable`), so consumers see one real null. A one-value
  enum of `null` keeps its `const`.

- **The object pages' default navigation.** Out of the box the component-schema
  pages no longer form one flat list: the `Objects` group (still the last group
  of the navigation) lists the schemas without an `x-docs.group` first and then
  a collapsed sub-group per resource group — `objectsLayout: "grouped"`. The
  per-area nesting stays available as `objectsLayout: "per-area"`; a document
  whose schemas carry no `x-docs.group` still gets the flat list.
- The object pages sort after every operation, whatever the tags — an
  operation tagged with the objects group's name used to pull them up among
  the operations; among themselves they keep the schema order.
- **The `Objects` group starts collapsed.** The generated navigation's
  top-level objects group entry carries `"collapsed": true` out of the box
  (`objectsCollapsed`, above); the fused goldens of the grouped-layout cases
  carry the key. A renderer that ignores the key shows the group as before.

### Fixed

- An `allOf` member that is not an of-type is folded onto the row instead of
  being pushed as a parent's one nameless child (the bogus "show properties"
  toggle on every described id and timestamp): a scalar's type and meta, an
  enum's values (the `$$enum` marker wins over a wrapper's declared base type,
  so `type: string` + `allOf: [$ref Color]` is an enum row), and the rows of an
  object or of a composition merged by name with the rows the other members
  declare — whatever the member order, nothing is dropped.
- Nested schema links follow their page. After the x-docs pass, a property's
  `symbolDef.canonical` is the canonical of the linked schema's own reference
  (a sidebar that moves an object page under another path included), and a
  link to a schema without a page (`x-docs.hidden`, or not listed by an
  `x-docs.sidebar`) keeps its `id` and drops its `canonical`.
- The same in regions mode. A page generated from one region (the per-page
  conversion) carries a one-page reference list, which cannot say which pages
  the navigation keeps; the sidebar plugin now reads the page links from the
  document instead — a schema has a page unless it is `x-docs.hidden`, or,
  under an `x-docs.sidebar`, only where an entry lists it — so a property
  typed by a hidden or unlisted schema is plain text on every page and a link
  follows the listed page's path. The Rust twin exposes the pass as
  `xdocs::apply_xdocs_links`.
- In sidebar mode a listed object page reads its `x-docs` from the same place
  as the generated navigation (the schema's own first, then the first `allOf`
  member's); a listed composition without any `x-docs` keeps its page and its
  default title instead of being dropped with a warning.
- The Rust core keyed its circular-reference snapshots by a stamp that the
  shared deref crawl writes differently from the JS dereferencer (a component
  stamped with its last member's path, an `allOf` wrapper stamped with its
  member's). Under one root, a second `allOf: [$ref A]` wrapper, or one that
  followed an `array of A`, came back as a renamed copy of the earlier row.
  The core now reads the stamp the way the JS leaves it (`core::js_ref_path`).

### Tests

- `9.schema-links` (constants, links, the allOf fold) and `5.xdocs.objects`
  (the per-schema x-docs, fused golden included); the `-3.random`, `8.enums`
  and `-2.complex.openai` oracles regenerated (new `symbolDef` / `const`
  entries, the folded rows).
- `10.schema-labels` (the labels: a widget, a schema without an `object`
  property, an array of widgets, a nullable widget, a union branch, the
  roots and the pages); every oracle regenerated (the `label` on each link,
  the root and page links), the `5.xdocs.objects` oracle and fused golden
  with the nested `Objects` sub-group. Unit tests on both twins for the
  labels, the nesting and the `objectsGroup` option (incl. the fused path).
- `12.page-paths` (an `x-docs.path` on an operation and on a schema, the
  title-derived paths of every other page — the group's noun dropped, a
  third-person verb, a summary that is not verb-first, an untagged operation,
  the `object` constant verbatim and a kebab-cased name — and the links that
  follow them; oracle + fused golden), converted with `operationPaths:
  "title"`; the other fixtures are unchanged under the default. Unit tests on
  both twins: the derivation table, the explicit paths, the collision error
  naming both pages, the sidebar precedence, regions mode. The Rust tier-1
  parity now also runs the x-docs fixtures THROUGH the ported plugin
  (`xdocs_case!` in tests/parity.rs: the whole reference list against the
  oracle, not only the fused sidebar/page map).
- The fixture matrix skips, under the native path only, the fixtures a
  published `@xyd-js/native` built from an older core cannot produce. The gate
  is a feature probe (does the addon produce this core's `9.schema-links` rows:
  the `const` meta, a nested link, the scalar fold, a composed member's merged
  rows?), so it lifts by itself once an addon with this core resolves.
- `scripts/build-fused-goldens.ts` takes `pluginNavigation` from the addon:
  its JS impl was reaped and the script could no longer run.
