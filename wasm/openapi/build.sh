#!/usr/bin/env bash
# Regenerate the vendored `api rest` engine from upstream speakeasy-api/openapi.
#
#   wasm/openapi/build.sh            rebuild and install the artifact
#   wasm/openapi/build.sh --check    rebuild into a temp dir and diff; write nothing
#
# A MANUAL regeneration, not a CI step. It needs network, git and a Go 1.26
# toolchain — none of which this repo's CI has, and none of which it should grow
# for a file that changes a few times a year. See README.md.
#
# Reproducibility rests on four things and nothing else:
#   UPSTREAM_SHA        pins every line of Go source, ours and theirs
#   GO_VERSION (exact)  Go output is byte-stable per toolchain, not across them
#   -trimpath           strips $HOME and $GOMODCACHE out of the binary
#   -buildvcs=false     the clone is a git repo AND we dirty it with shims, so
#                       VCS stamping would embed vcs.modified=true and a revision
#                       that says nothing about what was built. Version strings
#                       come from -ldflags instead.
set -euo pipefail
cd "$(dirname "$0")"
HERE="$PWD"
. ./pin.env

check_only=0
[ "${1:-}" = "--check" ] && check_only=1

export GOTOOLCHAIN="go${GO_VERSION}"
export CGO_ENABLED=0
# The upstream repo root has a go.work that `use`s both ./ and ./cmd/openapi.
# Left on, the CLI would resolve the parent library BY PATH (the tree at
# UPSTREAM_SHA) instead of by the pseudo-version its own go.mod pins. Those are
# different code. Off = build the module exactly as published.
export GOWORK=off

command -v go >/dev/null || { echo "error: go is not on PATH" >&2; exit 1; }
command -v zstd >/dev/null || { echo "error: zstd is not on PATH" >&2; exit 1; }
echo "toolchain: $(go version)"

# A FIXED workspace path, not mktemp. Go's build ID incorporates the module
# paths it was given, and the shims are injected through `replace` directives
# pointing into this directory — so a per-run temp path makes every build
# produce different bytes at an identical size. Verified: with mktemp, two
# consecutive builds of identical source differ; with a fixed path they do not.
WS="${APITOOLCHAIN_WASM_BUILD_DIR:-/tmp/apitoolchain-openapi-wasm-build}"
rm -rf "$WS"; mkdir -p "$WS"
SRC="$WS/src"

git clone --quiet --filter=blob:none --no-checkout "$UPSTREAM_REPO" "$SRC"
git -C "$SRC" checkout --quiet --detach "$UPSTREAM_SHA"
COMMIT_DATE="$(git -C "$SRC" show -s --format=%cI HEAD)"   # deterministic from the SHA
MOD="$SRC/cmd/openapi"

# The dependency set is REVIEWABLE. A new upstream release that bumps a
# transitive dependency shows up as a diff in upstream/go.sum in the pull
# request, rather than as an unexplained swing in an opaque 8 MB blob.
for f in go.mod go.sum; do
  if ! diff -q "upstream/$f" "$MOD/$f" >/dev/null 2>&1; then
    echo "error: upstream/$f disagrees with the clone at $UPSTREAM_SHA." >&2
    echo "       Review the diff, then copy it in deliberately:" >&2
    diff -u "upstream/$f" "$MOD/$f" 2>&1 | head -40 >&2
    exit 1
  fi
done

( cd "$MOD" && go mod download && go mod verify >/dev/null )

# Inject the two shims. `cmd/openapi` does not compile for wasip1 without them:
# bubbletea v2 and atotto/clipboard each split their platform files between a
# unix tag set and a windows one, and wasip1 matches neither, leaving six
# symbols undefined. Both shims are transcriptions of upstream's own _windows.go
# variants — see shims/*/ for the reasoning.
inject() {  # inject <module@version> <shim-dir>
  local want="$1" shimdir="$2"
  local mod="${want%@*}" ver="${want#*@}" got cache dst
  got="$(cd "$MOD" && go list -m -f '{{.Version}}' "$mod")"
  if [ "$got" != "$ver" ]; then
    echo "error: $mod resolved to $got, but shims/$shimdir is written for $ver." >&2
    echo "       Re-read that module's platform files before bumping pin.env:" >&2
    echo "       a shim that no longer matches its target is the one failure" >&2
    echo "       here that would not be loud." >&2
    exit 1
  fi
  cache="$(cd "$MOD" && go list -m -f '{{.Dir}}' "$mod")"
  dst="$WS/shimmed/$(echo "$mod" | tr '/@' '__')"
  mkdir -p "$dst"; cp -R "$cache/." "$dst/"; chmod -R u+w "$dst"
  cp "$HERE/shims/$shimdir/"*.go "$dst/"
  ( cd "$MOD" && go mod edit -replace "$mod=$dst" )
  echo "  shimmed $mod@$ver"
}
inject "$PIN_BUBBLETEA" bubbletea-v2
inject "$PIN_CLIPBOARD" clipboard

OUT="$WS/openapi.wasm"
echo "building (GOOS=wasip1 GOARCH=wasm) …"
( cd "$MOD" && GOFLAGS=-mod=mod GOOS=wasip1 GOARCH=wasm \
  go build \
    -trimpath \
    -buildvcs=false \
    -ldflags "-s -w \
      -X main.version=${UPSTREAM_REF} \
      -X main.commit=${UPSTREAM_SHA:0:7} \
      -X main.date=${COMMIT_DATE}" \
    -o "$OUT" . )

RAW_SIZE=$(wc -c < "$OUT" | tr -d ' ')
RAW_SHA=$(shasum -a 256 "$OUT" | cut -d' ' -f1)

# zstd -19: ~66 MB of Go text+data compresses about 8x. Committing it raw would
# add 66 MB to git history on every regeneration, permanently. -T1 deliberately,
# NOT -T0: the thread count changes the frame layout and therefore the bytes.
zstd -19 --long=27 -T1 -q -f -o "$WS/openapi.wasm.zst" "$OUT"
ZST_SHA=$(shasum -a 256 "$WS/openapi.wasm.zst" | cut -d' ' -f1)
ZST_SIZE=$(wc -c < "$WS/openapi.wasm.zst" | tr -d ' ')

cat > "$WS/openapi.wasm.json" <<JSON
{
  "upstream":       "$UPSTREAM_REPO",
  "upstreamSha":    "$UPSTREAM_SHA",
  "upstreamRef":    "$UPSTREAM_REF",
  "upstreamModule": "github.com/speakeasy-api/openapi/cmd/openapi@$UPSTREAM_MODVER",
  "goToolchain":    "go$GO_VERSION",
  "goos":           "wasip1",
  "goarch":         "wasm",
  "buildFlags":     "-trimpath -buildvcs=false -ldflags='-s -w -X main.version/commit/date'",
  "shims":          ["$PIN_BUBBLETEA", "$PIN_CLIPBOARD"],
  "wasmSha256":     "$RAW_SHA",
  "wasmBytes":      $RAW_SIZE,
  "zstSha256":      "$ZST_SHA",
  "zstBytes":       $ZST_SIZE
}
JSON

DEST="$HERE/../../crates/apitoolchain_rest/src"

if [ "$check_only" = 1 ]; then
  if ! cmp -s "$WS/openapi.wasm.zst" "$DEST/openapi.wasm.zst"; then
    echo "error: the rebuild does not match the committed artifact." >&2
    echo "  committed: $(shasum -a 256 "$DEST/openapi.wasm.zst" | cut -d' ' -f1)" >&2
    echo "  rebuilt:   $ZST_SHA" >&2
    exit 1
  fi
  echo "check: byte-identical ($ZST_SIZE bytes)"
  exit 0
fi

cp "$WS/openapi.wasm.zst" "$DEST/openapi.wasm.zst"
cp "$WS/openapi.wasm.json" "$HERE/openapi.wasm.json"
# The cache key includes this, so a re-vendored module can never be run against
# an artifact compiled from the previous one.
printf '%s+%s\n' "$UPSTREAM_REF" "${UPSTREAM_SHA:0:12}" > "$DEST/openapi.wasm.version"

printf 'wrote openapi.wasm.zst  %s bytes (from %s raw, %.1fx)\n' \
  "$ZST_SIZE" "$RAW_SIZE" "$(echo "scale=2; $RAW_SIZE/$ZST_SIZE" | bc)"
echo "version: $(cat "$DEST/openapi.wasm.version")"
