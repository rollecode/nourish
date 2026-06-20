[![CI](https://github.com/y5-snowies/nourish/actions/workflows/ci.yml/badge.svg)](https://github.com/y5-snowies/nourish/actions/workflows/ci.yml)
[![Coverage](https://nourish.snowies.com/docs/coverage.svg)](https://nourish.snowies.com/docs/coverage/)
[![Release](https://img.shields.io/github/v/release/y5-snowies/nourish?include_prereleases&display_name=tag)](https://github.com/y5-snowies/nourish/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

# ✻ Nourish

### A calmer way to use your computer.

Nourish is a Linux desktop where your screen is a window onto **one endless canvas**.
Spread your work out, zoom in to focus, zoom out to see everything — nothing gets buried,
nothing gets lost. Close the lid, come back tomorrow, and find it just as you left it.

It's free, open source, and runs on Fedora 44 today.

**[nourish.snowies.com](https://nourish.snowies.com)**  ·  [Guide](https://nourish.snowies.com/guide)

---

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

## Install

On Fedora 44, it's one command. You get a prebuilt build, so there's no toolchain to set up:

```bash
curl -fsSL https://nourish.snowies.com/release/latest/fedora44/package.tar.gz | tar -xz && y5-install/install.sh
```

The installer is interactive and safe to re-run. For the full walkthrough see
[`compositor.installer/INSTALL.md`](compositor.installer/INSTALL.md).

> Nourish is **early — v0.0.1, early but real**. Expect a few rough edges, and please tell
> us what you find.

## Made in the open

Anyone can read exactly how Nourish is made. Under the hood the engine is called **`y5`** — a
Wayland compositor written in Rust, standing on patched forks of
[smithay](https://github.com/Smithay/smithay) (Wayland), [bevy](https://bevyengine.org) +
[wgpu](https://wgpu.rs) (rendering), and [iced](https://iced.rs) (interface), all kept
in-tree under `vendor/`.

### For developers

| Start here | What it covers |
| --- | --- |
| [`CLAUDE.md`](CLAUDE.md) | how the codebase is organised + the conventions — the best entry point |
| [`environment/README.md`](environment/README.md) | build, run, and deploy on your machine |
| [`compositor.installer/`](compositor.installer/) | the installer, the shipped binaries, and the bundled components |
| [`ci/README.md`](ci/README.md) | the CI/CD pipeline (one set of portable scripts, two platforms) |

Each `compositor*/` folder is its own Cargo workspace — there's no root `Cargo.toml`. The
quickest way in is `environment/README.md`.

## License

Dual-licensed, at your option, under either:

- **Apache License 2.0** — [LICENSE-APACHE](LICENSE-APACHE)
- **MIT** — [LICENSE-MIT](LICENSE-MIT)

Copyright © 2026 Yarden Apelker. Unless you state otherwise, any contribution you submit for
inclusion is dual-licensed as above, with no additional terms.
