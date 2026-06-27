#!/usr/bin/env bash
# Build the public/ site for Pages (landing page + docs). Used by the docs/pages job.
#
# The former per-entry coverage pass (one `cargo llvm-cov` build per workspace +
# merged report) was removed along with the per-workspace CI build: the site no
# longer carries the self-hosted coverage badge/report. build-docs.sh still folds
# .ci-coverage/ into public/ if a report is present, so this stays forward-compatible
# if coverage is ever reintroduced as a root-level run.

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$REPO_ROOT"
here="$(dirname "${BASH_SOURCE[0]}")"

"$here/build-docs.sh"   # folds .ci-coverage/ into public/ when present
