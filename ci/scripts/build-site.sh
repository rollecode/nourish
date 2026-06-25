#!/usr/bin/env bash
# Build the full public/ site for Pages: full coverage (per entry) + merged report +
# landing page. Used by the docs/pages job so the published site carries the
# self-hosted coverage badge and per-crate report — no third-party coverage service.
#
# (PR runs gate coverage in ci.yml without publishing; this is the master/Pages build.)

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$REPO_ROOT"
here="$(dirname "${BASH_SOURCE[0]}")"

while IFS= read -r entry; do
    "$here/coverage-full.sh" "$entry"
done < <("$here/discover-workspaces.sh" --lines)

"$here/merge-coverage.sh"
"$here/build-docs.sh"   # folds .ci-coverage/ into public/ when present
