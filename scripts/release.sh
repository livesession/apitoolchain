#!/usr/bin/env bash
# Cut a release: bump, verify, commit, tag.
#
#   scripts/release.sh 0.1.0
#   scripts/release.sh 0.2.0-rc1
#   scripts/release.sh 0.1.0 --retag     # the tag exists and points at the wrong commit
#
# release.yml refuses to publish a tag that disagrees with cli/Cargo.toml, and
# the ordering that satisfies it is easy to get backwards: the tag must point at
# a commit whose manifest ALREADY says the new version. Tagging first, or
# bumping without committing, fails in CI minutes later rather than here.
#
# The crate ships at 0.0.0 — the value opencli2rust generates — so the FIRST
# release is always a bump. Editing it is safe: cli/Cargo.toml is written
# `SkipIfExists`, so regeneration never overwrites the version you set.
#
# Nothing is pushed. The script prints the push commands and stops.
set -euo pipefail
cd "$(dirname "$0")/.."

retag=0
skip_regen_check=0
args=()
for a in "$@"; do
  case "$a" in
    --retag) retag=1 ;;
    --skip-regen-check) skip_regen_check=1 ;;
    -*) echo "error: unknown flag '$a'" >&2; exit 1 ;;
    *) args+=("$a") ;;
  esac
done
set -- ${args[@]+"${args[@]}"}

version="${1:-}"
if [ -z "$version" ]; then
  cat >&2 <<'USAGE'
usage: scripts/release.sh <version> [--retag] [--skip-regen-check]

  scripts/release.sh 0.1.0
  scripts/release.sh 0.2.0-rc1

  --retag             the tag already exists and points at the wrong commit
                      (e.g. a release that failed the manifest check). Moves it.
  --skip-regen-check  cut the release without confirming cli/ is current.
                      Only when the opensdk toolchain is unavailable.
USAGE
  exit 1
fi
# Strip a leading v so both `0.1.0` and `v0.1.0` work; the tag gets it back.
version="${version#v}"
tag="v${version}"

# Semver-ish, prerelease allowed. release.yml compares EXACTLY, so `0.2.0-rc1`
# in the tag needs `0.2.0-rc1` here too — not the `0.2.0` core.
if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
  echo "error: '$version' is not a version cargo will accept" >&2
  exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is dirty — commit or stash first" >&2
  git status --short >&2
  exit 1
fi

# Is the tag already published? Needed twice: to refuse safely below, and to
# print `--force` at the end only when it is actually required.
tag_on_remote=0
if git ls-remote --tags origin "refs/tags/$tag" 2>/dev/null | grep -q "refs/tags/$tag"; then
  tag_on_remote=1
fi

if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  if [ "$retag" = 0 ]; then
    echo "error: tag $tag already exists." >&2
    echo "       It points at $(git rev-parse --short "$tag^{commit}") — $(git log -1 --format=%s "$tag^{commit}")" >&2
    echo >&2
    echo "       If that release failed and you want to move the tag:" >&2
    echo "         scripts/release.sh $version --retag" >&2
    exit 1
  fi

  # Moving a tag that already has a published release would leave that release
  # pointing at assets built from the old commit — a worse state than the failed
  # one being fixed. Only refuse on evidence; a missing `gh` is not evidence.
  if command -v gh >/dev/null 2>&1 && gh release view "$tag" >/dev/null 2>&1; then
    echo "error: a GitHub release already exists for $tag." >&2
    echo "       Moving the tag would leave it pointing at the old assets." >&2
    echo "       Delete the release first, or cut a new version instead." >&2
    exit 1
  fi

  echo "moving existing tag $tag (was $(git rev-parse --short "$tag^{commit}"))"
  git tag -d "$tag" >/dev/null
fi

# The check opensdk's release does not need: this CLI is GENERATED, so the
# committed cli/ can silently fall behind sdk.json or the OpenAPI spec it is
# built from. Releasing then ships a binary that does not match the source of
# truth in the same commit — and the drift is invisible until someone
# regenerates months later and cannot explain the diff.
if [ "$skip_regen_check" = 1 ]; then
  echo "! skipping the regeneration check (--skip-regen-check)"
elif ! command -v opensdk >/dev/null 2>&1; then
  cat >&2 <<'MISSING'
error: `opensdk` is not on PATH, so cli/ cannot be confirmed current.

  This CLI is generated. Releasing from a stale tree ships a binary that
  disagrees with the spec committed beside it.

  Install the toolchain, or re-run with --skip-regen-check if you have
  confirmed cli/ is current some other way.
MISSING
  exit 1
else
  echo "checking cli/ is current..."
  opensdk run --chain sdk.json --target api-cli >/dev/null
  if [ -n "$(git status --porcelain -- cli/)" ]; then
    echo "error: regenerating changed cli/ — the committed tree is STALE." >&2
    git --no-pager diff --stat -- cli/ >&2
    echo >&2
    echo "       Commit the regeneration first, then release:" >&2
    echo "         git add cli/ && git commit -m 'chore: regenerate cli'" >&2
    git checkout -- cli/
    exit 1
  fi
  echo "  cli/ is current"
fi

current=$(grep -m1 '^version = ' cli/Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
if [ "$current" = "$version" ]; then
  # Already bumped — a re-tag of an existing bump commit, or a re-run after the
  # tag step failed. Nothing to change; the tag below still needs creating.
  echo "cli/Cargo.toml already reads $version — tagging the existing commit"
else
  echo "cli/Cargo.toml: $current -> $version"
  # Only the [package] version, which is the first `version = ` in the file —
  # the dependency pins further down must not be touched.
  perl -0pi -e "s/^version = \"\Q$current\E\"\$/version = \"$version\"/m" cli/Cargo.toml

  # Refresh Cargo.lock's entry for this package. `cargo check`, NOT
  # `generate-lockfile`: the latter re-resolves every transitive dependency, and
  # several gates in this repo are byte comparisons an unrelated bump flips.
  cargo check -q -p api

  changed=$(git diff --numstat Cargo.lock | awk '{print $1+$2}')
  if [ "${changed:-0}" -gt 2 ]; then
    echo "error: Cargo.lock moved by $changed lines; expected 2 (the version)." >&2
    echo "       A transitive dependency re-resolved — inspect before releasing:" >&2
    git --no-pager diff --stat Cargo.lock >&2
    exit 1
  fi
fi

# The same comparison release.yml runs, run here where it costs seconds instead
# of a CI round trip. Package name `api`, not the repo name.
manifest=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c "import json,sys;print(next(p['version'] for p in json.load(sys.stdin)['packages'] if p['name']=='api'))")
if [ "$manifest" != "$version" ]; then
  echo "error: manifest reads $manifest after the bump, expected $version" >&2
  exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
  git add cli/Cargo.toml Cargo.lock
  git commit -q -m "chore(release): $tag"
fi
git tag -a "$tag" -m "$tag"

echo
echo "committed and tagged $tag at $(git rev-parse --short HEAD). Nothing pushed yet:"
echo
echo "    git push origin master"
if [ "$tag_on_remote" = 1 ]; then
  echo "    git push --force origin $tag     # the tag is published and is moving"
else
  echo "    git push origin $tag"
fi
echo
case "$version" in
  *-*) echo "note: '$version' is a PRERELEASE. GitHub's /releases/latest/ skips"
       echo "      prereleases, so anything resolving 'latest' will not see it." ;;
esac
