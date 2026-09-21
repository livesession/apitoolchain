# The `api rest` engine

`api rest <args…>` forwards its arguments verbatim to
[`github.com/speakeasy-api/openapi`](https://github.com/speakeasy-api/openapi)
(MIT), compiled to WebAssembly and embedded in the `api` binary. So
`api rest spec validate openapi.yaml` is `openapi spec validate openapi.yaml`,
and every upstream command and flag works without being re-declared here.

The compiled module lives at `crates/apitoolchain_rest/src/openapi.wasm.zst`
(~8 MB compressed, ~66 MB raw); this directory is how it is produced.

## Regenerating

```bash
wasm/openapi/build.sh            # rebuild and install the artifact
wasm/openapi/build.sh --check    # rebuild and diff; write nothing
```

Needs `git`, `zstd` and a Go toolchain (any — `GOTOOLCHAIN` pulls the pinned
one). Takes about a minute, almost all of it `go build`.

To move to a new upstream release:

1. Get the SHA the `cmd/openapi` **module** is published at — it is not the same
   commit as the parent library's tag:
   ```bash
   curl -s https://proxy.golang.org/github.com/speakeasy-api/openapi/cmd/openapi/@latest
   ```
2. Update `UPSTREAM_SHA`, `UPSTREAM_REF` and `UPSTREAM_MODVER` in `pin.env`.
3. Run `build.sh`. It will stop if `upstream/go.{mod,sum}` disagree with the new
   tree — that is deliberate: copy the new ones in as a **separate, reviewable
   commit** so a transitive dependency bump appears as a diff in the pull
   request rather than as an unexplained swing in an opaque binary.
4. Run `build.sh` again, then `cargo test --workspace`.

## Why there are shims

`cmd/openapi` does not compile for `wasip1` out of the box. Its dependency tree
reaches bubbletea (via `explore.go` and `snip.go`, which import it at *package*
level — so it cannot be avoided by not calling those commands), and two modules
split their platform files between a unix tag set and a windows one with nothing
matching `wasip1`:

| Module | Undefined on wasip1 |
|---|---|
| `charm.land/bubbletea/v2` | `initInput`, `suspendSupported`, `suspendProcess`, `listenForResize` |
| `github.com/atotto/clipboard` | `readAll`, `writeAll` |

`shims/` supplies those six symbols, transcribed from each project's own
`_windows.go` — already the "no SIGWINCH, no termios" degenerate case. They are
injected via `go mod edit -replace` against a copy of the module cache; upstream
source is never edited.

The shims are written against **specific dependency versions**, pinned in
`pin.env`. `build.sh` hard-fails if either resolves differently, because a shim
that no longer matches its target is the one failure here that would not be
loud.

The long-term fix is upstream: a `tty_other.go` / `signals_other.go` in
bubbletea with the negated tag set, matching the `termios_other.go` it already
ships. If that lands, the first shim disappears.

## Reproducibility

`build.sh --check` rebuilds and asserts the result is **byte-identical** to the
committed artifact. Four things make that hold, and each is load-bearing:

- `UPSTREAM_SHA` pins every line of source;
- `GO_VERSION` is exact — Go's output is stable per toolchain, not across them;
- `-trimpath` removes `$HOME` and `$GOMODCACHE` from the binary;
- **a fixed workspace path.** Go's build ID incorporates the module paths it was
  given, and the shims arrive through `replace` directives pointing into the
  workspace. With `mktemp -d`, two consecutive builds of identical source
  produce different bytes at an identical size. This was measured, not assumed.

`-buildvcs=false` for a related reason: the clone is a git repo and the shims
dirty it, so VCS stamping would embed `vcs.modified=true` and a revision that
describes nothing. The version strings come from `-ldflags` instead, which is
why `api rest --version` reports `v1.25.2`.

## Known limitations

**No network.** WASI preview 1 has no `sock_connect`, and Go's `net` package
under `wasip1` is a stub whose own header says it exists so tests can pass. A
document with `https://` `$ref`s will fail here where the native binary
succeeds. The failure happens inside the guest and cannot be improved from the
host. Vendor remote documents locally first.

**`spec explore` and interactive `snip` do not run.** There is no TTY. The
bubbletea shim's `initInput` returns an error rather than a no-op, deliberately:
a no-op would leave the TUI rendering once and then blocking forever on an input
reader that can never produce a key. An error exits cleanly with a message.

**Help text says `openapi`, not `api rest`.** Cobra takes its command path from
the root command's `Use` field, not from `os.Args[0]`, so this is not reachable
from the host. Upstream's documentation therefore translates directly.

## Startup cost

Cranelift needs ~8 seconds to compile a module this size, so the compiled
artifact is cached under the user's cache directory
(`$APITOOLCHAIN_REST_CACHE_DIR`, else the platform default) and mmap'd
thereafter. Measured on an M-series mac:

| | |
|---|---|
| First run (compile + cache) | ~8 s |
| Subsequent runs | ~1.75 s |
| …of which `deserialize_file` | ~15 ms |

The warm cost is almost entirely Go's runtime and package initialization inside
the module — `--help` and `spec validate` are indistinguishable, and the same
host path with a trivial module is 1.3 ms. Reducing it means shrinking the
module; the biggest single candidate is `goja` + `esbuild` (~13–19 MB raw),
pulled in by `openapi/linter/customrules` for JS-authored lint rules. Dropping
them would mean patching upstream source on every release and losing that
feature, so it has not been done.
