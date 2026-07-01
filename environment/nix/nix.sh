#!/usr/bin/env bash
# Enter a Nix dev shell with all host build dependencies for y5.
# Distro-agnostic alternative to environment/install-deps.sh — no sudo, no global
# system mutation, deps pinned via flake.lock.
#
# Usage:
#   ./environment/nix/nix.sh            # drop into an interactive build shell
#   ./environment/nix/nix.sh <cmd...>   # run a command inside the shell, then exit
#
# Requires Nix with flakes enabled. Covers BUILDING the compositor; to RUN it on the
# GPU, prefix the run command with nixGLNvidia (provided in the shell), e.g.
#   nixGLNvidia ./environment/run-host.sh winit debug
set -e

if ! command -v nix >/dev/null 2>&1; then
    echo "error: 'nix' is not installed or not on PATH." >&2
    echo "Install Nix first: https://nixos.org/download (multi-user recommended)," >&2
    echo "then enable flakes (experimental-features = nix-command flakes)." >&2
    exit 1
fi

FLAKE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Use the `path:` prefix so Nix reads this directory literally instead of via the
# enclosing git repo. Without it, when environment/nix/ is untracked/dirty, flake eval
# is keyed off the git tree and silently uses a STALE copy that ignores live edits.
FLAKE_REF="path:$FLAKE_DIR"

# --impure: nixGLNvidia auto-detects the host NVIDIA driver version at eval time,
# which needs access to the host outside the pure sandbox.
NIX_ARGS=(--extra-experimental-features 'nix-command flakes' --impure)

if [ "$#" -eq 0 ]; then
    exec nix develop "${NIX_ARGS[@]}" "$FLAKE_REF"
else
    exec nix develop "${NIX_ARGS[@]}" "$FLAKE_REF" --command "$@"
fi
