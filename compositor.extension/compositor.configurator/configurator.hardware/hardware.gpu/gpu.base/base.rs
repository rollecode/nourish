//! Enumerate DRM **render nodes** (`/dev/dri/renderD*` only — never the primary
//! `card*` nodes) and estimate each GPU's name from sysfs PCI ids, upgraded to a
//! real model name via `/usr/share/hwdata/pci.ids` when present. Pure std::fs.
use std::fs;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderDevice {
    /// e.g. `/dev/dri/renderD128`.
    pub node: String,
    /// Estimated GPU name, e.g. "NVIDIA Corporation GA102 [GeForce RTX 4090]".
    pub name: String,
}

/// All render nodes, sorted, each with an estimated GPU name.
pub fn render_devices() -> Vec<RenderDevice> {
    let mut names: Vec<String> = match fs::read_dir("/dev/dri") {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with("renderD"))
            .collect(),
        Err(_) => return Vec::new(),
    };
    names.sort();
    names
        .into_iter()
        .map(|n| RenderDevice { name: estimate(&n), node: format!("/dev/dri/{n}") })
        .collect()
}

fn read_trim(p: &str) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn estimate(render: &str) -> String {
    let base = format!("/sys/class/drm/{render}/device");
    let vendor = read_trim(&format!("{base}/vendor")).map(|s| s.trim_start_matches("0x").to_lowercase());
    let device = read_trim(&format!("{base}/device")).map(|s| s.trim_start_matches("0x").to_lowercase());
    let driver = fs::read_link(format!("{base}/driver"))
        .ok()
        .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()));
    if let (Some(v), Some(d)) = (&vendor, &device) {
        if let Some(name) = pci_lookup(v, d) {
            return name;
        }
        let vname = vendor_name(v);
        return match &driver {
            Some(drv) => format!("{vname} [{v}:{d}] ({drv})"),
            None => format!("{vname} [{v}:{d}]"),
        };
    }
    driver.unwrap_or_else(|| "Unknown GPU".to_string())
}

fn vendor_name(v: &str) -> &'static str {
    match v {
        "10de" => "NVIDIA",
        "1002" | "1022" => "AMD",
        "8086" => "Intel",
        _ => "GPU",
    }
}

/// Resolve "vendor model" from the system pci.ids database (if installed).
fn pci_lookup(vendor: &str, device: &str) -> Option<String> {
    let data = fs::read_to_string("/usr/share/hwdata/pci.ids")
        .or_else(|_| fs::read_to_string("/usr/share/misc/pci.ids"))
        .ok()?;
    let mut vname: Option<String> = None;
    for line in data.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            if line.len() >= 6 && line.is_char_boundary(4) && line[..4].eq_ignore_ascii_case(vendor) {
                vname = Some(line[4..].trim().to_string());
            } else if vname.is_some() {
                break; // moved past the target vendor's block
            }
        } else if vname.is_some() && !line.starts_with("\t\t") {
            let l = &line[1..];
            if l.len() >= 6 && l.is_char_boundary(4) && l[..4].eq_ignore_ascii_case(device) {
                return Some(format!("{} {}", vname.as_deref().unwrap_or(""), l[4..].trim()).trim().to_string());
            }
        }
    }
    None
}
