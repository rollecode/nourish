# y5.compositor.settings

Interactive setup tool for the compositor. A normal run shows a menu:

1. **Settings** — author the settings file, `~/.config/y5.compositor/settings.json`
   (override the location with `--config-file=<path>`). The render-device (GPU) field
   is a list of detected GPUs with estimated card names — the same enumeration the
   in-compositor settings window uses, shared by path via `gpu.base`.
2. **Set preferences** — per-monitor settings, written to
   `~/.config/y5.compositor/preferences.json`. For a monitor you can set its preferred
   mode or make it the **default output**. The compositor uses the first entry in the
   `outputs` array as the default, so "set as default" moves that monitor to the front;
   the monitor list marks the current default with `*`. Monitors and their advertised
   modes are probed directly over DRM, so this works standalone (e.g. during install,
   before any compositor session). The per-monitor identity key is computed to match
   the compositor's exactly, so a saved preference actually applies at runtime.

Escape navigates back (a real raw-mode key; on non-TTY stdin the lists degrade to a
single line read). Escape at the menu exits.

The compositor reads **all** of its configuration from settings.json and has no
defaults — it panics on a missing or partial file. This tool always writes a
**complete** file (every field), so a generated file is always loadable.

## Schema, without drift

This is a standalone workspace (like the other `developer.tool` tools — not in
`link.all.sh`), but it **path-depends on the exact same `config.base` crate** the
compositor parses. There is one `Environment` struct, compiled twice; the tool and
the compositor can never disagree on the schema. The GPU naming (`gpu.base`) and the
preferences schema + I/O (`preference.base`) are shared the same way. Only the EDID
identity parse is mirrored locally (its source crate pulls in smithay) — see the
parity note in `src/drm_probe.rs`.

## Usage

```
cargo run -- [OPTIONS]

  --config-file=<PATH>   Use PATH for settings.json instead of the default location.
  --installer            Installer setup: go straight to Settings (no menu, no
                         preferences, Escape disabled).
  --write-default        Non-interactive: write the canonical default settings.
  -h, --help             Show help.
```

Cargo target names can't contain `.`, so the binary builds as
`y5-compositor-settings`. Installers/run-scripts expose it under the command name
`y5.compositor.settings` (a symlink). For first-run/unattended provisioning, call
`y5-compositor-settings --write-default` to drop a valid file.
