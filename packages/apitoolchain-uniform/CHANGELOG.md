# @xyd-js/uniform

## Unreleased

### Added

- `DefinitionPropertyMeta` gains `"const"`: the literal of a single-value enum
  or an OpenAPI 3.1 `const` (`{ name: "const", value: "widget" }`), emitted by
  the OpenAPI converter beside the `$$enum` children so a renderer can print
  the value inline.
