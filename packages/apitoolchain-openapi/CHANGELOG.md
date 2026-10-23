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

- `allOf: [{ $ref: <scalar> }]` + `description` is a scalar row with the
  member's type and meta (an enum member folds its values in) instead of a
  parent with one nameless child — the bogus "show properties" toggle on every
  described id and timestamp.
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
  is a feature probe (does the addon emit `const`?), so it lifts by itself once
  an addon with this core resolves.
- `scripts/build-fused-goldens.ts` takes `pluginNavigation` from the addon:
  its JS impl was reaped and the script could no longer run.
