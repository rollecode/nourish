<div align="center">

<img src="compositor.website/assets/favicon.svg" alt="Nourish" width="84" height="84">

# Nourish

<img src="compositor.website/assets/hero-sphere.png" alt="The Nourish world — one endless canvas" width="300">

<br><br>

[![CI](https://github.com/y5-snowies/nourish/actions/workflows/ci.yml/badge.svg)](https://github.com/y5-snowies/nourish/actions/workflows/ci.yml)
[![Coverage](https://nourish.snowies.com/docs/coverage.svg)](https://nourish.snowies.com/docs/coverage/)
[![Release](https://img.shields.io/github/v/release/y5-snowies/nourish?sort=semver)](https://github.com/y5-snowies/nourish/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

### A simple next-gen desktop for work.

<video src="https://nourish.snowies.com/assets/hero.mp4" poster="compositor.website/assets/hero-sphere.png" controls muted loop width="720">
  <a href="https://nourish.snowies.com">▶ Watch the intro at nourish.snowies.com</a>
</video>

**[nourish.snowies.com](https://nourish.snowies.com)**  ·  [Guide](https://nourish.snowies.com/guide)  ·  [Discord](https://discord.gg/kasec5bYb)

</div>

---

Nourish is a Linux desktop where your screen is a window onto **one endless canvas**.
Spread your work out, zoom in to focus, zoom out to see everything — nothing gets buried,
nothing gets lost. Close the lid, come back tomorrow, and find it just as you left it.

It's free, open source, and stable to be used as daily driver.
It's performant, and renders using Vulkan. optionally, you can set automatic fallback or explicitly select Gles on systems where Vulkan is not supported.

## Glossary

The whole desktop is a handful of ideas. Everything else is built from these:

```
Nourish — one endless desktop
│
├─ Canvas        — one endless, zoomable surface; windows sit side by side, never stacked
│  ├─ Zoom       — step in to focus or out to see everything; text and video stay crisp
│  └─ Navigation — fly to a neighbouring window with a keystroke; hands stay on the keys
├─ Worlds        — many independent canvases; one key switches, each remembers itself
│  └─ Picker     — a gentle 3D screen showing every world as a tile to fly into
├─ Groups        — a named bundle of windows you line up, space out, or collapse to its name
├─ Placeholders  — the outline a closed or crashed app leaves behind; one tap restores it in place
├─ Capture       — screenshot or hardware-encoded H.264 video of a window, a region, or the screen
├─ Launcher      — a keyboard finder that starts any app
├─ Backdrop      — a living parallax scene that drifts behind your work
└─ Lock screen   — a clean lock screen with a real password prompt behind it
```

## What makes it different

🗺️ &nbsp;**One endless canvas.** Your windows live side by side instead of stacked in a pile.
The screen simply glides and zooms across them, and everything stays crisp at any zoom.

📌 &nbsp;**Things stay where you put them.** Finding your work feels like glancing across a
desk, not digging through a stack.

🪟 &nbsp;**Groups.** Gather a handful of windows, line them up, and give the bunch a name —
then collapse it down to just that name when you need it out of the way.

🌱 &nbsp;**A desktop that heals itself.** When an app closes — or crashes — Nourish leaves a
quiet outline where it was. One tap brings it back: same spot, same size. Your layout is
saved to disk, so it survives a full reboot too.

🪐 &nbsp;**Many canvases, one keystroke.** Want a clean slate? Flick to a whole separate
canvas. Keep "work" and "play" apart — each one remembers itself between sessions.

🎥 &nbsp;**Capture anything.** Screenshot or record a window, a region that follows your pan
and zoom, or the whole screen — even with a transparent background for clean overlays. You can capture 4K@120 smoothly without any performance drawbacks.

🖥️ &nbsp;**Runs on your hardware.** NVIDIA, Intel, or AMD; a stable Vulkan renderer with a
GLES fallback for older cards. Built natively on Wayland, and older X11 apps run too.

## Install

On Fedora 44, it's one command. You get a prebuilt build, so there's no toolchain to set up:

```bash
curl -fsSL https://nourish.snowies.com/release/latest/fedora44/package.tar.gz | tar -xz && y5-install/install.sh
```

The installer is interactive and safe to re-run. For the full walkthrough see
[`compositor.installer/INSTALL.md`](compositor.installer/INSTALL.md).

Prefer a pinned build? Every release is also published immutably under its version —
`https://nourish.snowies.com/release/v1.0.0/fedora44/package.tar.gz` — while `latest`
always points at the newest. Browse them on the
[releases page](https://github.com/y5-snowies/nourish/releases).

## Made in the open

Anyone can read exactly how Nourish is made. Under the hood the engine is called **`y5`** — a
Wayland compositor written in Rust, standing on patched forks of
[smithay](https://github.com/Smithay/smithay) (Wayland), [bevy](https://bevyengine.org) +
[wgpu](https://wgpu.rs) (rendering), and [iced](https://iced.rs) (interface), all kept
in-tree under `vendor/`.

```bash
# Build & run nested in your current Wayland session
environment/run-host.sh winit debug

# Build a release install bundle (compositor + components + installer)
ci/scripts/package-installer.sh
```

The conventions, architecture, and discovery commands live in [`CLAUDE.md`](CLAUDE.md) and
[`document/`](document/).

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
