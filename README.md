# apitoolchain

A reusable toolchain for everything about APIs: converting specs into a normalized
model, generating clients and CLIs from them, and the services and tooling around
managing them.

## Layout

| Path | What |
|------|------|
| `crates/` | The Rust converters: `uniform` (the normalized API model), `openapi`, `gql`, `mcp_uniform`, `opencli_uniform`, `oas_snippet`, plus the `parity` fixture harness |
| `packages/apitoolchain-*` | The five npm shims over those crates (`@xyd-js/uniform`, `/gql`, `/openapi`, `/mcp-uniform`, `/opencli`), each dispatching to the Rust core through `@xyd-js/native`. `gql`, `uniform` and `mcp-uniform` are native-only and throw without it; `openapi` still keeps a JS fallback |
| `packages/` (rest) | Toolchain packages: generated SDKs (`apitoolchainapp-*-node`), schemas, the release manager, filters, the design systems, the sdk.json wizard |
| `apps/` | `api`, `gitprovider`, `registry-api`, `app`, `storybook-federation` |
| `sdk.json` | What generates the SDKs and the CLI — sources (which spec) → targets (what, where). Built by `opensdk run` from this directory |
| `cli/` | The `api` binary — generated from the API's OpenCLI spec by `opencli2rust`, with hand-owned code in `src/custom/` |
| — | The SDK/CLI generation toolchain ([livesession/opensdk](https://github.com/livesession/opensdk)) is NOT vendored here: `crates/apitoolchain_openapi` takes `oas_doc` as a pinned git dependency, and generation uses the `opensdk` binary from `$PATH` |

## Setup

```bash
git clone https://github.com/livesession/apitoolchain
cd apitoolchain
pnpm install
pnpm build          # the five shims; their dist/ is what the tests import
```

This repo has **no submodules**. It used to document `--recurse-submodules` and a
`git submodule update --init opensdk`, but there is no `opensdk` gitlink and never
was one here: `crates/apitoolchain_openapi` takes `oas_doc` as a pinned **git
dependency**, which cargo fetches on its own.

Generating the SDKs and the CLI does need the `opensdk` **binary**, but only at
that moment — see "Generating the SDKs and the CLI" below.

## Tests

```bash
cargo test --workspace     # 68 tests: the converters' fixture-parity tiers
pnpm test:unit             # 115 tests across the five shims
```

Both suites are fixture-driven: every converter has a committed corpus under
`__fixtures__/<case>/` whose `output.json` is the frozen oracle. Suites that
enumerate a corpus assert its exact size, so a corpus that goes missing fails
rather than silently testing less.

`XYD_PARITY_DUMP=1` writes each Rust result beside its fixture as `output.rust.json`
for eyeball diffing. Never set `XYD_BLESS` in CI — it regenerates goldens instead of
checking them.

## The native addon

The shims resolve `@xyd-js/native` — the napi addon that compiles these crates into a
cdylib — as an **optional** dependency. It is built in the
[xyd](https://github.com/livesession/xyd) repo, which consumes this one as a submodule,
and is also published to npm, so a standalone `pnpm install` here resolves a real binary.

Three shims are **native-only**: `gql`, `uniform` and `mcp-uniform` deleted their frozen
JS implementations and now throw if the addon does not load, rather than silently running
a second implementation. Only `openapi` still carries a JS fallback, because most of its
`impl-js` is not a duplicate — it is live code on the native path (deref, code samples)
plus public API with no napi binding. The dual-mode parity gate (native ⇄ JS) therefore
covers `openapi` alone, and runs in xyd's CI where both halves exist.

## Generating the SDKs and the CLI

Everything this repo ships as a client is generated from an OpenAPI spec, and
`sdk.json` at the root is the single declaration of what comes from what:

```bash
opensdk run                     # everything
opensdk run --target api-cli    # one target
opensdk run --dry-run           # list the files, write nothing
```

| Source spec | → | Output |
|---|---|---|
| `apps/gitprovider/openapi/__generated__/openapi.yaml` | `gitprovider-node` | `packages/apitoolchainapp-gitprovider-node` |
| `apps/registry-api/openapi/v1/__generated__/openapi.yaml` | `registry-api-node` | `packages/apitoolchainapp-registry-api-node` |
| `apps/api/openapi/v1/__generated__/openapi.yaml` | `api-node` | `packages/apitoolchainapp-api-node` |
| `apps/api/openapi/v1/__generated__/openapi.yaml` | `api-cli` | `cli` (the `api` binary) |

Run it **from this directory**: opensdk resolves every relative path in `sdk.json`
against the process working directory, not against the config file, which is why
each path above reads like a plain repo path.

Two target options carry what used to be a wrapper script:

- `"entry": "source"` on the node targets — the manifests point at
  `./src/index.ts` instead of a `dist/` nobody builds, because the apps consume
  the TypeScript directly.
- `"format": true` on `api-cli` — the rust-cli emitter writes unformatted Rust
  while the committed tree is rustfmt-clean. opensdk formats the files before
  writing them, so `.sdk/sdk.lock` records what actually lands on disk and
  regeneration stays a byte-stable no-op.

The `opensdk` binary is resolved from `$PATH` and is not vendored here. Install
it with `cargo install --path cli` from an
[opensdk](https://github.com/livesession/opensdk) checkout, or
`xyd components install opensdk`. If it is out of date, rebuild it.

## The `api` CLI

```bash
cargo build --release -p api
./target/release/api --help
```

`cli/src/gen/**` is generated and carries a "DO NOT EDIT" header; `cli/src/custom/`
is yours and survives regeneration. Regenerate through the chain:

```bash
opensdk run          # or: pnpm sdk:generate
```
