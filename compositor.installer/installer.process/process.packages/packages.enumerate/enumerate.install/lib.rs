//! The `dnf install` runner + optional RPM Fusion (free) enablement. Pure std.

use std::process::Command;

/// Install the given packages with dnf. With `dry_run`, the command is printed only.
///
/// Strict: a non-zero dnf exit (including an unavailable package) is returned as an
/// error so the caller ABORTS rather than continuing with a half-installed system.
/// The default package set is therefore restricted to what the enabled repos actually
/// carry; anything RPM-Fusion-only is installed only after `enable_rpmfusion_free`.
pub fn dnf_install(packages: &[String], dry_run: bool) -> Result<(), String> {
    if packages.is_empty() {
        return Ok(());
    }
    let mut argv: Vec<String> = vec!["dnf".into(), "install".into(), "-y".into()];
    argv.extend(packages.iter().cloned());
    run_sudo(&argv, dry_run)
}

/// Enable the RPM Fusion **free** repo (its `-release` rpm), so `mesa-va-drivers-freeworld`
/// (hardware VA-API video) becomes installable. Opt-in only.
pub fn enable_rpmfusion_free(dry_run: bool) -> Result<(), String> {
    enable_rpmfusion("free", dry_run)
}

/// Enable the RPM Fusion **nonfree** repo, so `intel-media-driver` (the iHD VA-API driver
/// for Gen8+ Intel iGPUs, e.g. Kaby Lake) becomes installable. The nonfree `-release` rpm
/// depends on the free one, so enable free first. Opt-in only.
pub fn enable_rpmfusion_nonfree(dry_run: bool) -> Result<(), String> {
    enable_rpmfusion("nonfree", dry_run)
}

/// Swap Fedora's codec-stripped `ffmpeg-free` for RPM Fusion's full `ffmpeg`
/// (`--allowerasing`, since the full libs replace the `-free` ones). Needed for screen
/// capture on every machine — NVENC and VAAPI both encode through FFmpeg. Needs RPM
/// Fusion (free) enabled first. Opt-in.
pub fn swap_ffmpeg_full(dry_run: bool) -> Result<(), String> {
    run_sudo(
        &["dnf".into(), "swap".into(), "-y".into(), "ffmpeg-free".into(), "ffmpeg".into(), "--allowerasing".into()],
        dry_run,
    )
}

/// Install an RPM Fusion `<kind>-release` rpm (`kind` = "free" | "nonfree").
fn enable_rpmfusion(kind: &str, dry_run: bool) -> Result<(), String> {
    let rel = fedora_release();
    let url = format!(
        "https://mirrors.rpmfusion.org/{kind}/fedora/rpmfusion-{kind}-release-{rel}.noarch.rpm"
    );
    println!("Enabling RPM Fusion ({kind}) for Fedora {rel}...");
    run_sudo(&["dnf".into(), "install".into(), "-y".into(), url], dry_run)
}

/// `rpm -E %fedora` — the running Fedora release number; falls back to the bundle's
/// target (44) if rpm can't be queried.
fn fedora_release() -> String {
    Command::new("rpm")
        .args(["-E", "%fedora"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_else(|| "44".to_string())
}

/// Run `sudo <argv>`, or just print it under `dry_run`. Non-zero exit -> Err.
fn run_sudo(argv: &[String], dry_run: bool) -> Result<(), String> {
    if dry_run {
        println!("  [dry-run] sudo {}", argv.join(" "));
        return Ok(());
    }
    let status = Command::new("sudo")
        .args(argv)
        .status()
        .map_err(|e| format!("failed to run sudo {}: {e}", argv.first().map_or("", |s| s)))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{}` exited with {status}", argv.join(" ")))
    }
}
