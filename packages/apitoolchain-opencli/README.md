# @xyd-js/opencli

The **OpenCLI core** for xyd: the data model plus the pure spec/loader/generator
helpers shared by [`@xyd-js/opencli-remark`](../xyd-opencli-remark) (CLI docs) and the
`openapi2opencli` / `opencli2*` code generators.

## What's here

- **The model** (`src/types.ts`) — generated from [`opencli-spec.json`](./opencli-spec.json),
  the [OpenCLI](https://opencli.org) JSON Schema. `OpencliSpecJson`, `Command`, `Option`,
  `Argument`, `Arity`, `ExitCode`, `Conventions`, `CliInfo`, …
- **The `x-openapi` extension** — an xyd addition to the schema that binds an OpenCLI
  document back to its OpenAPI/HTTP origin so generators can build real requests:
  - `XOpenApiRoot` on the spec root — `servers`, `security` (with an `envVar` hint).
  - `XOpenApiCommand` on each command — `method`, `path`, `contentType`, `security`,
    `params[]` (each maps a CLI `argument:`/`option:` `from` token to an HTTP param),
    and `body` (`flatten` | `json` | `multipart`, per-property field mappings).
- **The generic extensions** — `x-interactive` (the commands typed inside a session a
  command opens, and its key bindings), `x-environment` (the environment variables a
  command reads) and `x-passthrough` (the command hands its arguments to another
  program). Their contract is [`opencli-extensions.schema.json`](./opencli-extensions.schema.json),
  vendored byte for byte from opensdk; its source path, commit and sha256 are in
  [`opencli-extensions.schema.source.json`](./opencli-extensions.schema.source.json). The
  types (`XInteractive`, `InteractiveCommand`, `KeyBinding`, `EnvVar`, `XPassthrough`)
  are hand-written in `src/extensions.ts`. Every fixture input is validated against both
  schemas (`crates/apitoolchain_opencli_uniform/tests/schema.rs`).
- **Helpers**
  - `loadOpencliSpec(source, { cwd? })` — load a spec from a file path or URL.
  - `findCommand(spec, "a b c")` — resolve a command by path (alias-aware).
  - `generateUsage / generateDescription / generateArguments / generateOptions /
    generateCommands / generateFlags` — render command docs (code or list style).
  - `opencliToReferences(spec, { regions?, globalOptionsPerCommand? })` — one uniform
    reference per command, for the docs engine's CLI pages.

## CLI reference pages

`opencliToReferences` gives each visible command a page. Its definitions, in order, each
only when it has rows:

| Section | From |
|---|---|
| Arguments, Options | the command's own |
| Inherited options | the recursive options of its ancestor commands, each name resolved to its nearest declaration (recursive or not, so a plain redeclaration ends the chain); the root's are the global options |
| Commands | its subcommands |
| Global options | the root's recursive options, with `globalOptionsPerCommand` (else one "Global options" page), less any name the command or an ancestor command redeclares |
| Interactive commands, Keys | `x-interactive`: session commands as rows with their arguments and options nested (never a page of their own), key bindings with their context |
| Environment | `x-environment`, accepted values as an `examples` meta |
| Exit codes | the command's own `exitCodes`, else its nearest ancestor's, the root's last |

The samples are the generated "CLI Tool" invocation, then the command's own `examples`
("Examples"), then an "Example response" from an `x-openapi` binding. A command group
with arguments or options of its own also runs without a subcommand, so its title and
usage read `[command]`, not `<command>`. `x-passthrough` changes nothing: what sits under
the command documents the other program and renders as usual.

The root gets a page of its own, ahead of the commands, only when it documents
arguments, examples, a session or environment variables. Its region and canonical are
its id, the CLI's title as a URL path segment: ASCII letters lowercased, every run of
anything but `a-z`, `0-9`, `_` and `-` one `-` (`Acme Agent` and `@acme/agent` →
`acme-agent`), `cli` when nothing is left. No other page answers it: while a top-level
command or the Global options page has that id, it takes the first free `-1`, `-2`, …
suffix. Its title and usage keep the title itself. Options or exit codes alone do not
make one.

Two implementations produce these pages: the TypeScript in `src/converters.ts`
(`opencliToReferencesJs`) and its Rust twin in `crates/apitoolchain_opencli_uniform`,
which the package entry prefers through `@xyd-js/native`. The goldens under
`__fixtures__/references/` pin both (`__tests__/references-golden.test.ts` and the
crate's `tests/references.rs`). Regenerate them from the TypeScript only:

```bash
# from the repo root
XYD_NATIVE=0 O2S_BUILD_DOCS=1 pnpm exec vitest run packages/apitoolchain-opencli/__tests__/references-golden.test.ts
```

## Regenerating the model

After editing `opencli-spec.json`:

```bash
pnpm --filter @xyd-js/opencli generate:types   # json2ts → src/types.ts (do not hand-edit)
pnpm --filter @xyd-js/opencli build
```
