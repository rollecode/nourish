//! Read laptop battery state from `/sys/class/power_supply/*` (entries whose
//! `type` is `Battery`). `read()` returns `None` on machines with no battery —
//! i.e. desktops — so callers can use presence as a laptop signal. Pure std::fs.
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    /// Charge level, 0-100 percent.
    pub capacity: u8,
    /// True while plugged in and charging (or already full on AC).
    pub charging: bool,
}

fn read_trim(p: &str) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// First `type == Battery` power supply, or `None` when none exists (desktop).
pub fn read() -> Option<Battery> {
    let mut names: Vec<String> = fs::read_dir("/sys/class/power_supply")
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names {
        let base = format!("/sys/class/power_supply/{name}");
        if read_trim(&format!("{base}/type")).as_deref() != Some("Battery") {
            continue;
        }
        let Some(capacity) = read_trim(&format!("{base}/capacity")).and_then(|s| s.parse::<u8>().ok())
        else {
            continue;
        };
        let status = read_trim(&format!("{base}/status")).unwrap_or_default();
        let charging = matches!(status.as_str(), "Charging" | "Full");
        return Some(Battery { capacity: capacity.min(100), charging });
    }
    None
}
