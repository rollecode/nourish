#!/usr/bin/env bash
# (B) Compile the y5_compositor binary on a target distro and extract it to the host.
# Builds the per-distro image (which clones the local repo and compiles), then copies the
# binary out of it — no live container needs to keep running.
#
# Usage: ./build.sh <distro> [debug|release] [out-dir]
#   out-dir default: ./out/<distro>/   (gitignored)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
. "$HERE/common.sh"

DISTRO="${1:?usage: build.sh <distro> [debug|release] [out-dir]  (have: $(distro_list | tr '\n' ' ')) }"
PROFILE="${2:-debug}"
OUT="${3:-$HERE/out/$DISTRO}"
distro_validate "$DISTRO"

IMAGE="$(distro_image "$DISTRO" "$PROFILE")"

# Always (re)build so the extracted binary reflects the current committed source.
"$HERE/image.sh" "$DISTRO" "$PROFILE"

mkdir -p "$OUT"
cid="$(podman create "$IMAGE")"
trap 'podman rm -f "$cid" >/dev/null 2>&1 || true' EXIT
podman cp "$cid:/usr/local/bin/y5_compositor" "$OUT/y5_compositor"

echo ">> extracted $DISTRO $PROFILE binary -> $OUT/y5_compositor" >&2
