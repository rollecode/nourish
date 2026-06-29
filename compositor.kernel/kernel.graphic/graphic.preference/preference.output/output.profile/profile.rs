//! Typed per-monitor preference keyed by EDID identity. Populated from the live
//! preferences document (`preferences.json`) via [`get`]; the kernel keeps its own
//! self-contained value type so the hardware path never depends on the on-disk serde
//! shape directly — [`get`] maps the developer-side schema onto it. Order is
//! preserved: the FIRST profile is the default output (see `display.base`'s
//! `profiles.first()`), matching how the settings UI orders them.

#[derive(Debug, Clone)]
pub enum ModeRequest {
    /// Pick from advertised modes (current default policy applies when None).
    Advertised { width: u16, height: u16, refresh_mhz: u32 },
    /// Synthesize via CVT (requires the mode-synthesis safety enable).
    Cvt { width: u16, height: u16, refresh: f64 },
    /// Raw modeline string (requires the mode-synthesis safety enable).
    Modeline(String),
}

#[derive(Debug, Clone, Default)]
pub struct OutputProfile {
    /// EDID identity string this profile applies to ("make model serial").
    /// `None` = applies to any output (single-output era default).
    pub identity: Option<String>,
    pub mode: Option<ModeRequest>,
    pub position: (i32, i32),
    pub scale: Option<f64>,
}

/// Load the per-monitor profiles from `preferences.json`, mapped onto the kernel's
/// value type. A missing/invalid file yields an empty vec (default policy), so this
/// is behavior-neutral when the user has set no output preferences.
pub fn get() -> Vec<OutputProfile> {
    compositor_developer_environment_preference_base::base::load()
        .outputs
        .into_iter()
        .map(map_profile)
        .collect()
}

fn map_profile(p: compositor_developer_environment_preference_base::base::OutputProfile) -> OutputProfile {
    OutputProfile {
        identity: p.identity,
        mode: p.mode.map(map_mode),
        position: p.position,
        scale: p.scale,
    }
}

fn map_mode(m: compositor_developer_environment_preference_base::base::ModeRequest) -> ModeRequest {
    use compositor_developer_environment_preference_base::base::ModeRequest as Src;
    match m {
        Src::Advertised { width, height, refresh_mhz } => ModeRequest::Advertised { width, height, refresh_mhz },
        Src::Cvt { width, height, refresh } => ModeRequest::Cvt { width, height, refresh },
        Src::Modeline(s) => ModeRequest::Modeline(s),
    }
}
