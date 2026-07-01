#!/usr/bin/env bash
# Compute the effective RELEASE-CANDIDATE version (X.Y.Z-rc.N) for the current commit
# and print it to stdout. This is the rc-channel sibling of version.sh and shares its
# VERSION-file mechanics; it differs only in appending the `-rc.N` pre-release counter.
#
# Numeric base X.Y.Z = the upcoming release number, derived EXACTLY like version.sh:
#   committed VERSION base, with PATCH bumped past the highest published stable
#   `v<MAJOR>.<MINOR>.<patch>` tag (the `v…-rc.*` tags are non-numeric after the series
#   strip, so they are ignored here — stable numbering and rc numbering stay independent).
#
# The -rc.N counter is derived from the existing `v<X.Y.Z>-rc.*` tags published by the
# rc release job (no commit-back, working tree stays clean):
#   - no rc tag for this X.Y.Z yet -> rc.1
#   - rc tags exist                -> highest published N + 1   (rc.1 -> rc.2 -> ...)
#
# Idempotent: if HEAD already carries a `v<X.Y.Z>-rc.*` tag (a re-run of an already-
# published rc commit) that exact version is reused instead of bumping again.
#
# Platform-agnostic: tags are the only state it reads, so GitHub and GitLab agree.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

VERSION_FILE="$REPO_ROOT/VERSION"
[ -r "$VERSION_FILE" ] || die "missing VERSION file at $VERSION_FILE"
base="$(tr -d '[:space:]' < "$VERSION_FILE")"
case "$base" in
    [0-9]*.[0-9]*.[0-9]*) : ;;
    *) die "VERSION must be MAJOR.MINOR.PATCH (got '$base')" ;;
esac
major="${base%%.*}"; rest="${base#*.}"; minor="${rest%%.*}"; basepatch="${rest#*.}"
series="v$major.$minor."

# CI shallow clones frequently omit tags; pull them so the series is complete. Best
# effort — a dev running this offline still gets whatever tags are already local.
git -C "$REPO_ROOT" fetch --tags --quiet 2>/dev/null || true

# --- numeric base: highest published stable patch in this MAJOR.MINOR series (+1) ---
# Identical derivation to version.sh. Only pure-integer patches count, so `v1.1.0-rc.3`
# (patch token "0-rc.3") is skipped and never perturbs the upcoming release number.
maxpatch=-1
while IFS= read -r tag; do
    p="${tag#"$series"}"
    case "$p" in '' | *[!0-9]*) continue ;; esac
    [ "$p" -gt "$maxpatch" ] && maxpatch="$p"
done < <(git -C "$REPO_ROOT" tag --list "$series*" 2>/dev/null)

if [ "$maxpatch" -ge "$basepatch" ]; then
    patch=$((maxpatch + 1))
else
    patch="$basepatch"
fi
numbase="$major.$minor.$patch"
rcseries="v$numbase-rc."

# Re-run guard: an rc version already pinned to this commit wins — don't double-bump.
on_head="$(git -C "$REPO_ROOT" tag --points-at HEAD --list "$rcseries*" 2>/dev/null | sort -V | tail -n1)"
if [ -n "$on_head" ]; then
    printf '%s\n' "${on_head#v}"
    exit 0
fi

# --- rc counter: highest published N for this X.Y.Z (+1), else 1 ---
maxrc=0
while IFS= read -r tag; do
    n="${tag#"$rcseries"}"
    case "$n" in '' | *[!0-9]*) continue ;; esac
    [ "$n" -gt "$maxrc" ] && maxrc="$n"
done < <(git -C "$REPO_ROOT" tag --list "$rcseries*" 2>/dev/null)

printf '%s-rc.%s\n' "$numbase" "$((maxrc + 1))"
