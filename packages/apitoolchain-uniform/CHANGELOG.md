# @xyd-js/uniform

## Unreleased

### Added

- `uniformToMcpTools(references, options?)`: one MCP tool definition per REST
  operation — name, title, description, a JSON Schema input (flat by default,
  or grouped), annotations from predicates, and a `binding` back onto the HTTP
  request. Pure JavaScript (no native addon); Rust twin
  `apitoolchain_uniform::mcp::uniform_to_mcp_tools`.
- `@xyd-js/uniform/skill`: `skillFiles(references, options)` renders an agent
  skill — `SKILL.md`, an operation index and one markdown reference file per
  group (or per operation, `references.perOperation`). Pure JavaScript; Rust
  twin `apitoolchain_uniform::skill::skill_files`.
- `DefinitionOpenAPIMeta` / `DefinitionVariantOpenAPIMeta` gain
  `"additionalProperties"` (`true` or a JSON Schema): a request body that
  declares itself open. The MCP emitter carries it onto a flattened input
  object; the OpenAPI converter does not emit it yet.
- `DefinitionPropertyMeta` gains `"const"`: the literal of a single-value enum
  or an OpenAPI 3.1 `const` (`{ name: "const", value: "widget" }`), emitted by
  the OpenAPI converter beside the `$$enum` children so a renderer can print
  the value inline.
- `SymbolDef` gains `label?: string` (Rust: `SymbolDef::label`): what to call
  the linked symbol where a bare type would be shown. The OpenAPI converter
  sets it on every link to a component schema — the schema's constant
  `object` value (`widget`), else its name.
