# sdk/ — what generates the SDKs and the CLI

Everything this repo publishes as a client is generated from an OpenAPI spec by the
[opensdk](https://github.com/livesession/opensdk) toolchain, and this directory is the
single declaration of what comes from what.

```bash
pnpm sdk:generate                      # everything
pnpm sdk:generate -- --target api-cli  # one target
pnpm sdk:generate -- --dry-run         # print the file list, write nothing
```

| | |
|---|---|
| `chain.json` | The declaration: `sources` (which spec) → `targets` (what to generate, where) |
| `generate.sh` | Runs the toolchain over it, then applies two post-steps the generator cannot know about |
| `.chain/` | Merged/processed specs, written at run time. Gitignored |

## What it produces

| Source spec | → target | Output |
|---|---|---|
| `apps/gitprovider/openapi/__generated__/openapi.yaml` | `gitprovider-node` | `packages/apitoolchainapp-gitprovider-node` |
| `apps/registry-api/openapi/v1/__generated__/openapi.yaml` | `registry-api-node` | `packages/apitoolchainapp-registry-api-node` |
| `apps/api/openapi/v1/__generated__/openapi.yaml` | `api-node` | `packages/apitoolchainapp-api-node` |
| `apps/api/openapi/v1/__generated__/openapi.yaml` | `api-cli` | `cli` (the `api` binary) |

## The cwd contract — why every path here is repo-root-relative

**opensdk resolves every relative path in `chain.json` against the process working
directory, never against `chain.json`'s own location.** That is not an implementation
detail you can ignore; it decides what this file *means*:

- `opensdk cli/src/command.rs` — `let cwd = std::env::current_dir()`
- `run.rs` — `run_chain(opts, cwd)`; source inputs and target outputs both resolve
  against that `cwd`, with the comment *"Resolve against the run cwd so a
  relative/default output roots at the caller's cwd."*

So "where you ran it from" and "what the paths mean" are the same question. `generate.sh`
answers it once, by `cd`-ing to the repo root before doing anything — which is why every
path above reads like a plain repo path with no `../` in it, and why running the script
from a subdirectory works identically.

This is also why `sdk/` sits at the root rather than inside `packages/`: the hop to the
root is a single `..` that cannot rot when a package is renamed. The previous location
was two levels deep, and its path arithmetic rotted twice — once for the binary lookup,
once for the post-generate patch list, the latter silently.

## Every target must declare an explicit `output`

When `output` is omitted, opensdk defaults to `./sdk/<targetName>` relative to cwd —
which, with cwd pinned to the repo root, is **this directory**. A target that forgets it
would generate an SDK on top of the config.

The JSON schema does not prevent that (only `target` and `source` are required), so
`generate.sh` asserts it before invoking the toolchain and exits non-zero naming the
offending targets.

## The two post-steps

Neither belongs to the generator, and both are derived from `chain.json` rather than
repeated — a duplicated list here is exactly what rotted before.

1. **Node packages are pointed at `src/`.** The bun islands consume TypeScript source
   with no build step, so `main`/`types`/`exports` are rewritten after generation.
   Without this the generated manifests would win and the islands would import a `dist/`
   that is never built.
2. **The generated Rust CLI is formatted.** The `rust-cli` emitter writes unformatted
   Rust while the committed tree is rustfmt-clean, so skipping it leaves
   `cargo fmt --all --check` red in CI for reasons unrelated to the change.

## The opensdk binary

The toolchain is **not vendored in this repo** — there are no submodules here, and
`oas_doc` arrives as a pinned git dependency of `crates/apitoolchain_openapi`. Only
generation needs the binary, and `generate.sh` looks for it in this order:

1. `$OPENSDK_BIN`
2. `opensdk` on `$PATH` (`xyd components install opensdk`)
3. `../opensdk/target/{release,debug}/opensdk` — a sibling checkout, which is also where
   it lands when this repo is used as a submodule of xyd

If none is found it exits non-zero with those three options, rather than skipping
generation and reporting success.
