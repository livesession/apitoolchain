# @xyd-js/openapi

## Unreleased

Both halves of the converter (the Rust core in `crates/apitoolchain_openapi`
and the JS impl in `src/impl-js`) change together; the fixtures `9.schema-links`
and `5.xdocs.objects` pin the new output on both sides.

### Added

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
- The fixture matrix skips, under the native path only, the fixtures a
  published `@xyd-js/native` built from an older core cannot produce. The gate
  is a feature probe (does the addon produce this core's `9.schema-links` rows:
  the `const` meta, a nested link, the scalar fold, a composed member's merged
  rows?), so it lifts by itself once an addon with this core resolves.
- `scripts/build-fused-goldens.ts` takes `pluginNavigation` from the addon:
  its JS impl was reaped and the script could no longer run.
