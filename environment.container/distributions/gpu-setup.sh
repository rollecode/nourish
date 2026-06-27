#!/usr/bin/env bash
# Make the NVIDIA gbm backend (nvidia-drm_gbm.so) resolvable in THIS distro's gbm dir.
#
# Why: container.env sets GBM_BACKEND=nvidia-drm, so mesa tries to load `nvidia-drm_gbm.so`.
# CDI (nvidia-ctk) injects the backend lib itself (libnvidia-allocator.so) but creates the gbm
# symlink against the HOST's gbm path — which on a Debian/Ubuntu image (search path
# /usr/lib/x86_64-linux-gnu/gbm) or any non-host distro is the wrong place. Without the symlink:
#   MESA-LOADER: failed to open nvidia-drm: .../gbm/nvidia-drm_gbm.so: cannot open ...
#   KMS: DRM_IOCTL_MODE_CREATE_DUMB failed: Permission denied   (the dumb-buffer fallback on a
#   render node always EACCESes) → the compositor panics in CreateBo.
#
# This links nvidia-drm_gbm.so -> the injected libnvidia-allocator so allocations take the NVIDIA
# path instead of the dumb-buffer fallback. Idempotent; needs container-root (run.sh has it).

# Find the CDI-injected NVIDIA gbm allocator (the real backend behind nvidia-drm_gbm.so).
alloc="$(ldconfig -p 2>/dev/null | grep -oE '/[^ ]*libnvidia-allocator\.so\.[0-9.]+' | head -1)"
[ -n "$alloc" ] || alloc="$(find /usr/lib /usr/lib64 -maxdepth 4 -name 'libnvidia-allocator.so.*' 2>/dev/null | head -1)"

if [ -z "$alloc" ]; then
    echo "gpu-setup: libnvidia-allocator.so not found — is the NVIDIA CDI device attached?" >&2
    echo "  Check on the host:  nvidia-ctk cdi list   (run.sh passes --device \$CDI_DEVICE)" >&2
    return 0 2>/dev/null || exit 0
fi

linked=0
for gbmdir in /usr/lib/x86_64-linux-gnu/gbm /usr/lib64/gbm /usr/lib/gbm /usr/lib/aarch64-linux-gnu/gbm; do
    [ -d "$gbmdir" ] || continue
    if [ ! -e "$gbmdir/nvidia-drm_gbm.so" ]; then
        ln -sf "$alloc" "$gbmdir/nvidia-drm_gbm.so" \
            && echo "gpu-setup: linked $gbmdir/nvidia-drm_gbm.so -> $alloc" >&2 \
            && linked=1
    fi
done
[ "$linked" = 1 ] || echo "gpu-setup: nvidia gbm backend already present (alloc: $alloc)" >&2
return 0 2>/dev/null || exit 0
