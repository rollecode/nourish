#!/usr/bin/env bash
# Install the host build dependencies for a bare-metal (non-container) build on Fedora.
# Usage: ./install-deps.sh
set -e

# Wayland / smithay core devel
sudo dnf install -y \
    libinput-devel libseat-devel libxkbcommon-devel \
    pixman-devel clang-devel wayland-devel \
    wayland-protocols-devel mesa-libgbm-devel \
    libdisplay-info-devel systemd-devel \
    dbus-devel pam-devel

# Build tools (protoc for prost-build)
sudo dnf install -y \
    protobuf-compiler


# Graphics stack (Intel/VA-API for bare-metal) + ffmpeg 8.x (screen capture / video encode)
sudo dnf install -y \
    mesa-libEGL-devel \
    mesa-libGL-devel \
    mesa-libgbm-devel \
    libglvnd-devel \
    ffmpeg-free-devel \
    pulseaudio-libs-devel

# Compiler toolchain (default system linker; mold no longer used)
sudo dnf install -y clang

# Diagnostics
sudo dnf install -y egl-utils mesa-demos

# Install-bundle components (match ci/Containerfile):
# Developer-tool window (Tauri 2) GUI devel
sudo dnf install -y \
    webkit2gtk4.1-devel libsoup3-devel gtk3-devel \
    librsvg2-devel libappindicator-gtk3-devel

# xwayland-satellite (X11/XCB)
sudo dnf install -y \
    libxcb-devel xcb-util-cursor-devel

