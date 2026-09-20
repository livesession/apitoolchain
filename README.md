# apitoolchain

A reusable toolchain for everything about APIs: converting specs into a normalized
model, generating clients and CLIs from them, and the services and tooling around
managing them.

## Layout

| Path | What |
|------|------|
| `crates/` | The Rust converters: `xyd_uniform` (the normalized API model), `xyd_openapi`, `xyd_gql`, `xyd_mcp_uniform`, `xyd_opencli_uniform`, `xyd_oas_snippet`, plus the `xyd_parity` fixture harness |
| `packages/xyd-*` | The five npm shims over those crates (`@xyd-js/uniform`, `/gql`, `/openapi`, `/mcp-uniform`, `/opencli`). Each dispatches to the Rust core through `@xyd-js/native` when present and falls back to a frozen JS implementation otherwise |
| `packages/` (rest) | Toolchain packages: generated SDKs (`*-node`), the SDK chain config, schemas, the release manager, filters, the design systems, the sdk.json wizard |
| `apps/` | `api`, `gitprovider`, `registry-api`, `web`, `storybook-federation` |
| `cli/` | The `api` binary — generated from the API's OpenCLI spec by `opencli2rust`, with hand-owned code in `src/custom/` |
| `opensdk/` | Submodule ([livesession/opensdk](https://github.com/livesession/opensdk)) — the SDK/CLI generation toolchain. `crates/xyd_openapi` path-deps its `oas_doc` |

## Setup

```bash
git clone --recurse-submodules https://github.com/livesession/apitoolchain
cd apitoolchain
pnpm install
pnpm build          # the five shims; their dist/ is what the tests import
```

If you already cloned without `--recurse-submodules`:

```bash
git submodule update --init opensdk
```

The submodule is not optional: `crates/xyd_openapi` path-deps `opensdk/crates/oas_doc`,
so without it cargo cannot load the workspace at all.

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
[xyd](https://github.com/livesession/xyd) repo, which consumes this one as a
submodule. Standalone, the addon is absent and every shim falls back to its frozen JS
implementation: correct, just slower. The dual-mode parity gate (native ⇄ JS) runs in
xyd's CI, where both halves exist.

## The `api` CLI

```bash
cargo build --release -p api
./target/release/api --help
```

`cli/src/gen/**` is generated and carries a "DO NOT EDIT" header; `cli/src/custom/`
is yours and survives regeneration. Regenerate through the chain:

```bash
packages/apitoolchainapp-sdk-chain/generate.sh
```
