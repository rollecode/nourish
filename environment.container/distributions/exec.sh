#!/usr/bin/env bash
# Launch the winit y5_compositor from inside a distro container (started by run.sh).
# run.sh drops you into a shell first so you can poke at the distro (check libs, GPU, env,
# `ldd /usr/local/bin/y5_compositor`, `vulkaninfo`, ...); run this when you're ready to start
# the compositor. It writes the required settings.json from the COMPOSITOR_* env, then execs.
#
# Usage (inside the container shell):  exec.sh
set -euo pipefail

# Ensure the NVIDIA gbm backend (nvidia-drm_gbm.so) resolves in this distro's gbm dir — CDI
# injects the lib but not always the symlink. Without it, GBM_BACKEND=nvidia-drm fails and mesa
# falls back to DRM_IOCTL_MODE_CREATE_DUMB on a render node → EACCES panic. (Idempotent.)
/usr/local/bin/gpu-setup.sh || true

# Turn the COMPOSITOR_* env knobs into the settings.json the binary requires (incl. every
# required field). run.sh mounts the live environment/compositor-env.sh over the image's copy,
# so settings-writer fixes take effect without rebuilding.
# shellcheck disable=SC1091
. /working.directory/environment/compositor-env.sh
compositor_write_settings

exec y5_compositor "$@"
