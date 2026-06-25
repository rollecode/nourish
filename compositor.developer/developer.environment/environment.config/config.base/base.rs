use std::sync::OnceLock;

/// The compositor's complete runtime configuration, read from a JSON file
/// (`~/.config/y5.compositor/settings.json`, override with `--config-file=<path>`).
/// Every field is REQUIRED — no optionals, no defaults; startup panics otherwise.
/// This is the ONE place the compositor reads its own configuration.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    /// Renderer backend: `"vulkan"` or `"gles"`.
    pub renderer: String,
    /// Fall back to GLES if Vulkan initialization fails.
    pub renderer_fallback: bool,
    /// Frame-sync: `""` (off), `"infence"` (KMS IN_FENCE), or `"kms"`.
    pub renderer_sync: String,
    /// Enable HDR output (Vulkan only).
    pub hdr: bool,
    /// Scanout bit depth: `8` (SDR) or `10` (deep color).
    pub depth: u8,
    /// Enable adaptive sync / VRR.
    pub vrr: bool,
    /// DRM render node path, e.g. `/dev/dri/renderD128`.
    pub render_node: String,
    /// XDG desktop name advertised to clients, e.g. `Y5Compositor`.
    pub desktop_name: String,
    /// Developer-log level spec, e.g. `"info,warn,error"`.
    pub log_level: String,
    /// Vulkan diagnostics overlay: `""`, `"vk"`, or `"blit"`.
    pub vk_diag: String,
    /// Capture encoder: `"mesa"`/`"vaapi"` for VAAPI, else NVENC.
    pub capture_encoder: String,
    /// Live-capture video codec: `"av1"` | `"h265"` | `"h264"`. Falls back to
    /// the first available NVENC encoder along av1 → h265 → h264 (VP9 has no
    /// NVENC; reachable only via the optimized software re-encode).
    pub capture_codec: String,
    /// Live-capture quality: `"lossless"` (near-lossless, CQ 19) or
    /// `"optimized"` (smaller, higher CQ — still real-time hardware). Sets the
    /// live NVENC CQ; independent of the optional software re-encode below.
    pub capture_quality: String,
    /// Max live-capture frame rate, clamped to `30..=120`.
    pub capture_refresh_rate_max: u32,
    /// Optional post-capture **software** re-encode (much smaller): `""` = off
    /// (offer it as an "Optimized encoding" checkbox in the save dialog);
    /// `"ffmpeg"` = run it automatically in the background after every recording
    /// (no checkbox; writes a `.y5-encoding` file renamed to the target on done).
    pub capture_background_encoder: String,
    /// `false` (default) = a failed NVENC zero-copy start aborts the capture with
    /// an error dialog. `true` = fall back to the slower GPU→CPU readback encoder
    /// instead. (The readback path also flips correctly on winit, unlike
    /// zero-copy — but it's not used unless this is enabled.)
    pub capture_nvenc_allow_readback_fallback: bool,
    /// `true` = keep the capture's natural variable frame rate (exact timing,
    /// smallest). `false` = produce a constant frame rate, snapped to a standard
    /// rate (else nearest 5), for editors/players that reject VFR. CFR is applied
    /// during the re-encode pass (it can't be done without re-timing frames), so
    /// `false` forces a re-encode even for an otherwise plain save.
    pub capture_variable_frame_rate: bool,
    /// `false` = compositor-tracked window sizing; `true` = client xdg geometry.
    pub window_client_size_fallback: bool,
    /// `false` = fit only the root toplevel; `true` = fit the whole surface tree.
    pub window_subsurface_shrinks: bool,
    /// `true` (default) = natural scrolling: invert the touchpad finger-axis
    /// direction for canvas pan, window scroll, and multi-finger swipe navigation
    /// (a discrete mouse wheel is unaffected). `false` = use the raw direction.
    pub input_natural_scroll: bool,
}

static ENV: OnceLock<Environment> = OnceLock::new();

/// Resolve the settings-file path: `--config-file=<path>`/`--config-file <path>`
/// from process args if present, else `$XDG_CONFIG_HOME/y5.compositor/settings.json`,
/// else `$HOME/.config/y5.compositor/settings.json`. Shared with the companion tool.
pub fn resolve_path() -> std::path::PathBuf {
    if let Some(p) = config_file_arg(std::env::args()) {
        return std::path::PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .expect("neither XDG_CONFIG_HOME nor HOME set; cannot locate settings.json");
            home.join(".config")
        });
    base.join("y5.compositor").join("settings.json")
}

/// Extract a `--config-file` value from an argument iterator (split out for testing).
pub fn config_file_arg(args: impl Iterator<Item = String>) -> Option<String> {
    let mut it = args;
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--config-file=") {
            return Some(v.to_string());
        }
        if a == "--config-file" {
            return it.next();
        }
    }
    None
}

/// Read and parse the settings file exactly once, as the very first thing in
/// `main()` (before logging, which reads `log_level`). Panics with a clear message
/// if the file is unavailable or any required field is missing/invalid. This crate
/// has no logging dep, so it uses `panic!` rather than `abort!`.
pub fn init() {
    let path = resolve_path();
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read settings file {}: {e}. Create it with `y5.compositor.settings`, \
             or pass --config-file=<path>.",
            path.display()
        )
    });
    let parsed: Environment = serde_json::from_str(&raw).unwrap_or_else(|e| {
        panic!("settings file {} is invalid: {e}. Every field is required.", path.display())
    });
    if ENV.set(parsed).is_err() {
        panic!("environment already initialized");
    }
}

/// The parsed environment. Panics if called before [`init`].
pub fn get() -> &'static Environment {
    ENV.get().expect("environment not initialized; call init() first in main()")
}

/// Canonical complete starting settings — the single source of default values shared by
/// the configuration TOOLS: the `y5.compositor.settings` editor and the installer's seed.
/// NOT used by the compositor at runtime — [`init`] still requires a fully-populated file
/// and never falls back to these, so a real config can't be silently half-default. Living
/// here (with the struct) means the editor and the installer agree on one set of values
/// across the full 19-field schema, so any seeded file is always complete and valid.
pub fn default_settings() -> Environment {
    Environment {
        renderer: "vulkan".to_string(),
        renderer_fallback: true,
        renderer_sync: String::new(),
        hdr: false,
        depth: 8,
        vrr: false,
        render_node: "/dev/dri/renderD128".to_string(),
        desktop_name: "Y5Compositor".to_string(),
        log_level: "info,warn,error".to_string(),
        vk_diag: String::new(),
        capture_encoder: "nvenc".to_string(),
        capture_codec: "av1".to_string(),
        capture_quality: "optimized".to_string(),
        capture_refresh_rate_max: 120,
        capture_background_encoder: "ffmpeg".to_string(),
        capture_nvenc_allow_readback_fallback: false,
        capture_variable_frame_rate: false,
        window_client_size_fallback: false,
        window_subsurface_shrinks: false,
        input_natural_scroll: true,
    }
}
