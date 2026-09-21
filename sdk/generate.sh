#!/usr/bin/env bash
# Generate every SDK and CLI declared in sdk/chain.json.
#
# THE CWD CONTRACT — the reason this script exists rather than a bare
# `opensdk run --chain sdk/chain.json`:
#
# opensdk resolves every relative path inside chain.json against the PROCESS
# working directory, never against the chain file's own location
# (opensdk cli/src/command.rs `std::env::current_dir()` → run.rs `run_chain(opts, cwd)`
# → sources.rs `read_raw_doc_with(location, cwd)` / `resolve_path(cwd, out)`; run.rs
# says it outright: "Resolve against the run cwd so a relative/default output roots
# at the caller's cwd").
#
# So "where you run this from" and "what chain.json means" are the same question.
# Pinning cwd here makes the invocation site irrelevant — run it from anywhere and
# the paths mean the same thing — which is what lets chain.json spell every path
# repo-root-relative and read like a map of the repo.
set -euo pipefail

# sdk/ is a direct child of the root, so this is a single `..` that cannot rot when
# a package is renamed. The previous location was two levels deep and its arithmetic
# did rot, twice.
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

chain="sdk/chain.json"

# --- guard: every target must declare an explicit `output` -------------------
# When `output` is omitted opensdk defaults to `./sdk/<targetName>` relative to cwd
# (run.rs). With cwd pinned to the root that is THIS directory, so a target that
# forgets `output` would generate an SDK on top of the config. The schema does not
# prevent it (only `target` and `source` are required), so assert it here, before
# anything is written.
node -e '
  const chain = require("./" + process.argv[1]);
  const bad = Object.entries(chain.targets || {})
    .filter(([, t]) => !t.output)
    .map(([name]) => name);
  if (bad.length) {
    console.error("error: target(s) without an explicit `output`: " + bad.join(", "));
    console.error("  opensdk would default them to ./sdk/<target> — i.e. into this config directory.");
    process.exit(1);
  }
' "$chain"

# --- resolve the opensdk binary ----------------------------------------------
# This repo has NO submodules: `oas_doc` comes from a pinned git dependency
# (crates/apitoolchain_openapi/Cargo.toml), not from a nested checkout. An earlier
# version of this script looked for `opensdk/` inside the repo and could therefore
# never find it — in either the standalone clone or the xyd-nested one.
#
# `$root/../opensdk` is the one relative rule that is true in BOTH contexts:
# standalone it is the sibling clone, and as a submodule of xyd it is xyd's own
# opensdk, which sits beside apitoolchain.
opensdk_bin="${OPENSDK_BIN:-}"
if [ -z "$opensdk_bin" ]; then
  opensdk_bin="$(command -v opensdk || true)"
fi
if [ -z "$opensdk_bin" ]; then
  for candidate in \
    "$root/../opensdk/target/release/opensdk" \
    "$root/../opensdk/target/debug/opensdk"; do
    [ -x "$candidate" ] && { opensdk_bin="$candidate"; break; }
  done
fi
if [ -z "$opensdk_bin" ]; then
  echo "error: no opensdk binary found." >&2
  echo "  install:  xyd components install opensdk" >&2
  echo "  or build: cargo build --release -p opensdk   (in a sibling opensdk checkout)" >&2
  echo "  or set:   OPENSDK_BIN=/path/to/opensdk" >&2
  exit 1
fi
echo "using opensdk: $opensdk_bin"

# Arguments are forwarded to `opensdk run` (e.g. --target api-cli, --dry-run).
# --dry-run has to be honoured HERE too: steps 2 and 3 below rewrite manifests and
# run rustfmt, which a dry run must not do. They are idempotent against an
# already-generated tree, so skipping them is invisible on a clean checkout — and
# that is exactly why it would go unnoticed if they were left to run.
dry_run=0
for arg in "$@"; do
  [ "$arg" = "--dry-run" ] && dry_run=1
done

# 1. The toolchain does the work: each source spec is processed, each target generated.
"$opensdk_bin" run --chain "$chain" "$@"

if [ "$dry_run" -eq 1 ]; then
  echo "dry run — skipping the post-generate patch and format steps"
  exit 0
fi

# 2. Point each generated node package at src/ (the bun islands consume source, no
#    build step) and ignore install output + the .sdk regen manifest.
#
#    The target list is DERIVED from chain.json rather than repeated here. The
#    previous copy hardcoded `gitprovider-node` etc. while the real directories had
#    been renamed to `apitoolchainapp-*`, so its `[ -f ] || continue` skipped all
#    three in silence and exited 0 — and because the patch it applies is committed,
#    the next successful run would have quietly reverted all three manifests.
node -e '
  const fs = require("fs");
  const path = require("path");
  const chain = require("./" + process.argv[1]);
  for (const [name, t] of Object.entries(chain.targets || {})) {
    if (t.target !== "node") continue;
    // A relative require() specifier must start with "./" — a bare "packages/x"
    // is a node_modules lookup, not a path.
    const pkgPath = path.join(t.output, "package.json");
    if (!fs.existsSync(pkgPath)) {
      console.error(`error: ${name} generated no package.json at ${pkgPath}`);
      process.exit(1);
    }
    const p = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
    p.main = "./src/index.ts";
    p.types = "./src/index.ts";
    p.exports = { ".": { types: "./src/index.ts", import: "./src/index.ts" } };
    p.files = ["src"];
    p.scripts = { typecheck: "tsc --noEmit" };
    fs.writeFileSync(pkgPath, JSON.stringify(p, null, 2) + "\n");
    fs.writeFileSync(path.join(t.output, ".gitignore"), "node_modules\n.sdk\n");
    console.log(`patched ${t.output} → src exports`);
  }
' "$chain"

# 3. The rust-cli emitter writes UNFORMATTED Rust while the committed tree is
#    rustfmt-clean (same convention as opencli2rust's own `regen` bin, which fmts
#    after write_project). Without this every regeneration leaves
#    `cargo fmt --all --check` red in CI for reasons unrelated to the change.
#
#    The crate name is read from the generated Cargo.toml rather than hardcoded, so
#    renaming a CLI in chain.json cannot leave this step silently formatting nothing.
for crate in $(node -e '
  const fs = require("fs");
  const path = require("path");
  const chain = require("./" + process.argv[1]);
  for (const t of Object.values(chain.targets || {})) {
    if (t.target !== "rust-cli") continue;
    const manifest = path.join(t.output, "Cargo.toml");
    if (!fs.existsSync(manifest)) continue;
    const m = fs.readFileSync(manifest, "utf8").match(/^\s*name\s*=\s*"([^"]+)"/m);
    if (m) console.log(m[1]);
  }
' "$chain"); do
  cargo fmt -p "$crate" && echo "formatted $crate"
done
