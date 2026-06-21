#!/usr/bin/env bash
# Install the host build dependencies for a bare-metal (non-container) build on Fedora.
# Usage: ./install-deps.sh
set -e

# Wayland / smithay core devel
sudo dnf install -y \
    libinput-devel libseat-devel libxkbcommon-devel \
    pixman-devel clang-devel wayland-devel \
    wayland-protocols-devel mesa-libgbm-devel \
    libdisplay-info-devel systemd-devel


# Graphics stack (Intel/VA-API for bare-metal) + ffmpeg 8.x (screen capture / video encode)
sudo dnf install -y \
    mesa-libEGL-devel \
    mesa-libGL-devel \
    mesa-libgbm-devel \
    libglvnd-devel \
    ffmpeg-free-devel

# Compiler toolchain (default system linker; mold no longer used)
sudo dnf install -y clang

# Diagnostics
sudo dnf install -y egl-utils mesa-demos

