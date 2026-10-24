# @xyd-js/uniform

## Unreleased

### Added

- `DefinitionPropertyMeta` gains `"const"`: the literal of a single-value enum
  or an OpenAPI 3.1 `const` (`{ name: "const", value: "widget" }`), emitted by
  the OpenAPI converter beside the `$$enum` children so a renderer can print
  the value inline.
- `SymbolDef` gains `label?: string` (Rust: `SymbolDef::label`): what to call
  the linked symbol where a bare type would be shown. The OpenAPI converter
  sets it on every link to a component schema — the schema's constant
  `object` value (`widget`), else its name.
