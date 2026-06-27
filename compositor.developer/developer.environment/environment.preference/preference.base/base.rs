//! Live user **preferences** — the inline-reloaded counterpart to the read-once
//! `environment.config` (settings.json). Stored in
//! `~/.config/y5.compositor/preferences.json` (same dir). Loaded fresh ([`load`])
//! rather than cached in a startup `OnceLock`: startup seeds the runtime cells,
//! the settings window re-reads on open (so terminal edits show) and writes back
//! with [`save`]; applied live, no reboot.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A per-output mode preference keyed by EDID identity. `Advertised` is the only
/// variant applied by default policy; the synthesis variants require the separate
/// mode-synthesis safety enable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModeRequest {
    /// Pick from the modes the monitor advertises.
    Advertised { width: u16, height: u16, refresh_mhz: u32 },
    /// Synthesize via CVT (requires the mode-synthesis safety enable).
    Cvt { width: u16, height: u16, refresh: f64 },
    /// Raw modeline string (requires the mode-synthesis safety enable).
    Modeline(String),
}

/// Per-monitor output preference. `identity = None` applies to any output
/// (single-output-era default).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutputProfile {
    /// EDID identity string ("make model serial") this profile applies to.
    pub identity: Option<String>,
    pub mode: Option<ModeRequest>,
    #[serde(default)]
    pub position: (i32, i32),
    pub scale: Option<f64>,
}

/// The complete preferences document. `#[serde(default)]` so a partial or older
/// `preferences.json` (or a missing file) still loads with sane per-field values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preference {
    /// Pointer cursor speed multiplier on relative motion (`1.0` = unscaled).
    pub cursor_sensitivity: f64,
    /// Natural scrolling: invert the touchpad finger-axis direction for canvas
    /// pan, window scroll, and multi-finger swipe navigation (wheel unaffected).
    pub input_natural_scroll: bool,
    /// Per-EDID output mode/position/scale preferences.
    pub outputs: Vec<OutputProfile>,
}

impl Default for Preference {
    fn default() -> Self {
        Self { cursor_sensitivity: 1.0, input_natural_scroll: true, outputs: Vec::new() }
    }
}

/// `preferences.json`, in the same config dir as `settings.json` (honoring
/// `--config-file`/`XDG_CONFIG_HOME` via the shared resolver).
fn path() -> PathBuf {
    compositor_developer_environment_config_base::base::resolve_path()
        .with_file_name("preferences.json")
}

/// Load the preferences fresh from disk. A missing or invalid file yields the
/// defaults (so the compositor and the settings window always have sane values).
pub fn load() -> Preference {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Persist `prefs` atomically (write to a sibling `.tmp`, then rename over the
/// target — a partial write can never replace a good file).
pub fn save(prefs: &Preference) -> Result<(), String> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let json =
        serde_json::to_string_pretty(prefs).map_err(|e| format!("serialize preferences: {e}"))?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &p).map_err(|e| format!("rename {}: {e}", p.display()))?;
    Ok(())
}

/// Set the mode for the output profile matching `edid_key`, inserting a new
/// profile if none exists. Other fields of an existing profile are preserved.
pub fn upsert_output(outputs: &mut Vec<OutputProfile>, edid_key: &str, mode: ModeRequest) {
    if let Some(p) = outputs.iter_mut().find(|p| p.identity.as_deref() == Some(edid_key)) {
        p.mode = Some(mode);
    } else {
        outputs.push(OutputProfile {
            identity: Some(edid_key.to_string()),
            mode: Some(mode),
            ..Default::default()
        });
    }
}
