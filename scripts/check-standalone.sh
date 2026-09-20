#!/usr/bin/env bash
# Guard: this repo must not reach back into xyd.
#
# The crates and the five shims keep their `xyd_*` / `xyd-*` names on purpose —
# renaming them would have destroyed the blob-SHA proof the extraction rests on,
# and their fixture paths resolve `crates/<c>/../../packages/<p>` on BOTH sides
# of the split precisely because neither name changed. So the thing to check is
# not the NAME but the REACH: a path, a dep, or an import that only resolves
# inside the xyd monorepo.
#
# Every guard reports the size of the corpus it scanned. A guard that matches
# nothing because its pathspec is broken is otherwise indistinguishable from a
# guard that passed.
set -euo pipefail
cd "$(dirname "$0")/.."

EXCLUDES=(
  ':(exclude)*/__fixtures__/*'
  ':(exclude)*/__oracle__/*'
  ':(exclude)Cargo.lock'
  ':(exclude)pnpm-lock.yaml'
  ':(exclude)*/package-lock.json'
  ':(exclude)*/bun.lock'
)

fail=0

# guard <label> <min-corpus> <regex> <pathspec>...
guard() {
  local label="$1" min="$2" regex="$3"; shift 3
  local paths=("$@") n status

  n=$(git ls-files -- "${paths[@]}" "${EXCLUDES[@]}" | wc -l | tr -d ' ')

  if [ "$n" -lt "$min" ]; then
    echo "ERROR: [$label] scanned only $n file(s), expected >= $min — the" >&2
    echo "       pathspec is broken, so a clean result proves nothing." >&2
    fail=1; return
  fi
  if [ "$n" -eq 0 ]; then
    echo "  [$label] no files match this corpus — check INAPPLICABLE (not a pass)"; return
  fi

  # git grep: 0 = matched, 1 = no match, >1 = real error. `if git grep` would
  # collapse 1 and 2 into "clean", so a broken invocation would read as a pass.
  set +e
  git grep -n -E "$regex" -- "${paths[@]}" "${EXCLUDES[@]}"
  status=$?
  set -e

  case "$status" in
    0) echo "ERROR: [$label] matched (see above) across $n file(s)." >&2; fail=1 ;;
    1) echo "  [$label] clean ($n file(s) scanned)" ;;
    *) echo "ERROR: [$label] git grep failed with status $status." >&2; fail=1 ;;
  esac
}

# 1. Cargo path deps escaping the repo. Exactly ONE `../..` prefix is legitimate:
#    crates/openapi -> ../../opensdk/crates/oas_doc, the nested submodule.
#    Anything else at that depth lands outside this repo.
#
#    Not written with the generic guard above: `git grep -E` is POSIX ERE and has
#    no negative lookahead, so the obvious `(?!/opensdk/)` form fails with
#    "repetition-operator operand invalid" — and that error surfaced as status
#    128, which the guard reports rather than swallowing. Match broadly, then
#    subtract the one allowed target.
check_path_deps() {
  local n offenders
  n=$(git ls-files -- '*/Cargo.toml' 'Cargo.toml' | wc -l | tr -d ' ')
  if [ "$n" -lt 7 ]; then
    echo "ERROR: [path-deps] scanned only $n Cargo.toml file(s), expected >= 7." >&2
    fail=1; return
  fi
  offenders=$(git grep -n -E 'path *= *"\.\./\.\.' -- '*/Cargo.toml' 'Cargo.toml' \
    | grep -v '\.\./\.\./opensdk/' || true)
  if [ -n "$offenders" ]; then
    echo "ERROR: [path-deps] a path dep escapes this repo (only ../../opensdk/ is allowed):" >&2
    echo "$offenders" >&2
    fail=1
  else
    echo "  [path-deps] clean ($n file(s) scanned)"
  fi
}
check_path_deps

# 2. Relative filesystem reaches into xyd's package layout from Rust. The crates'
#    OWN fixture paths (`../../packages/apitoolchain-gql`) resolve here too, so the thing
#    to ban is a reach at a package that did NOT move.
guard "rust-xyd-paths" 20 '"(\.\./)*packages/xyd-(core|native|atlas|framework|components|plugin|theme)' '*.rs'

# 3. TS/JS importing xyd packages that stayed behind. The five shims here are
#    themselves @xyd-js/*, and uniform legitimately type-imports @xyd-js/core
#    (an optional peer, installed from npm) — so enumerate the ones that must
#    never appear in the SHIMS. apps/ is excluded: apps/app consumes several
#    staying packages from npm on purpose.
guard "shim-imports" 30 "from ['\"]@xyd-js/(native|atlas|framework|components|themes?|theme-|plugin-|documan|content|composer|host|cli)" \
  'packages/apitoolchain-uniform/*' 'packages/apitoolchain-gql/*' 'packages/apitoolchain-openapi/*' \
  'packages/apitoolchain-mcp-uniform/*' 'packages/apitoolchain-opencli/*'

# 4. workspace: protocol deps naming a package that is not in THIS workspace.
#    pnpm fails loudly on these at install time, but only once someone installs;
#    this catches it in CI on the file alone.
guard "dangling-workspace-deps" 5 '"@xyd-js/(core|native|openapi-sampler|atlas|framework|components)" *: *"workspace:' \
  'packages/*/package.json' 'package.json'

[ "$fail" -eq 0 ] && echo "check-standalone: OK"
exit "$fail"
