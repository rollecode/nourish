#!/usr/bin/env bash
# Materialize the self-contained source tree the distro builds use (default: distributions/.src).
#
# RUN THIS IN AN ENVIRONMENT THAT HAS THE REPO'S FULL GIT DATA (the dev sandbox).
# In a linked git worktree, .git is a file pointing at an external gitdir (e.g.
# /home/y5/nourish/.git). A machine that only has the worktree's files — the host — cannot
# resolve that link, so it cannot clone the worktree. This step produces a NORMAL, self-contained
# clone (committed files only, real .git/, no external link) here, where the data exists.
#
# Afterwards image.sh / build.sh / run.sh build from it ANYWHERE — host included — with no git
# link dependency. Re-run this to refresh the source after committing changes.
#
# Usage: ./prepare-source.sh         (writes distributions/.src)
#   Override the destination with Y5_DIST_SRC=/some/path ./prepare-source.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
. "$HERE/common.sh"

prepare_source
echo ">> prepared source: $DIST_SRC ($(du -sh "$DIST_SRC" 2>/dev/null | cut -f1), branch $(git -C "$DIST_SRC" rev-parse --abbrev-ref HEAD), $(git -C "$DIST_SRC" rev-parse --short HEAD))" >&2
