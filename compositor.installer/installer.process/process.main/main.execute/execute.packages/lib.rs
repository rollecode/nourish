//! Installer step 1: detect the GPU, prompt the dnf package groups + the RPM Fusion
//! VA-API drivers, install the selection, and — on NVIDIA — check the bound driver and
//! warn (Nourish never installs the proprietary NVIDIA driver).

use compositor_installer_process_config_parse_base::prompt;
use compositor_installer_process_packages_enumerate_base as pkg;
use compositor_installer_process_packages_enumerate_base::NvidiaDriver;

/// Mesa Gallium VAAPI driver (RPM Fusion free).
const MESA_VA_FREEWORLD: &str = "mesa-va-drivers-freeworld";
/// Intel iHD VA-API driver (RPM Fusion nonfree) — for Gen8+ Intel iGPUs (e.g. Kaby Lake).
const INTEL_MEDIA_DRIVER: &str = "intel-media-driver";
/// Fedora's codec-stripped FFmpeg runtime libs carried by the `runtime` group. When the
/// user opts into full FFmpeg these are dropped from the base install: the swap below
/// pulls the full libav stack and erases Fedora's preinstalled `-free` packages in ONE
/// pass, so installing them here first would only get them immediately erased again.
const FFMPEG_FREE_LIBS: &[&str] = &[
    "libavutil-free", "libavcodec-free", "libavformat-free", "libavfilter-free", "libswscale-free",
];

/// Detect the GPU vendor, prompt every package group AND the RPM-Fusion capture/VA-API
/// options, then run dnf over the resolved selection. All prompts run BEFORE the first
/// dnf, so the base transaction already targets the final package set — when full FFmpeg
/// is chosen the `-free` libs are never installed-then-swapped. STRICT: a real install
/// failure (incl. a package the enabled repos lack) returns `Err` so the caller aborts.
pub fn select_and_install(dry_run: bool) -> Result<(), String> {
    let gpu = pkg::detect_gpu();
    println!("\nDetected GPU vendor: {gpu:?}");

    // 1) Package groups.
    let mut selected_packages: Vec<String> = Vec::new();
    println!("\n-- Package groups (Enter keeps the [default]) --");
    for group in pkg::groups() {
        let want = prompt::yes_no(
            group.key,
            &format!("{} — {}", group.title, group.description),
            group.default_on,
        );
        if want {
            selected_packages.extend(group.packages.iter().map(|s| s.to_string()));
        }
    }

    // 2) RPM-Fusion capture / VA-API choices — prompted up front (before any dnf) so the
    // base install targets the final set. These are RPM-Fusion-only (the base group set
    // never references them), enabling the repo(s) on demand below.
    println!("\n-- Screen capture / VA-API (RPM Fusion) --");
    let intel = gpu == pkg::Gpu::Intel;
    let want_ffmpeg = prompt::yes_no("Full FFmpeg (RPM Fusion)", "Swap Fedora's codec-stripped ffmpeg-free for the full ffmpeg — REQUIRED for screen capture on every machine (NVENC + VAAPI both encode through it). Enables RPM Fusion (free).", true);
    let want_mesa_va = prompt::yes_no("mesa-va-drivers-freeworld (RPM Fusion)", "Mesa VAAPI driver — REQUIRED by the compositor's Vulkan renderer + VA-API capture. Enables RPM Fusion (free). Recommended.", true);
    let want_intel_media = prompt::yes_no("intel-media-driver (RPM Fusion)", "Intel iHD VA-API driver — Gen8+ Intel iGPUs (e.g. Kaby Lake) need it for the Vulkan renderer + capture. Enables RPM Fusion (nonfree). Recommended on Intel only.", intel);

    // 3) When swapping to full FFmpeg, drop the `-free` libs from the base install so they
    // are never installed only to be erased by the swap (no double-install of ffmpeg).
    if want_ffmpeg {
        selected_packages.retain(|p| !FFMPEG_FREE_LIBS.contains(&p.as_str()));
    }

    // 4) Base install.
    if selected_packages.is_empty() {
        println!("No package groups selected — skipping dnf.");
    } else {
        println!("\nInstalling {} packages via dnf...", selected_packages.len());
        pkg::dnf_install(&selected_packages, dry_run)
            .map_err(|e| format!("package install failed: {e}"))?;
    }

    // 5) Apply the RPM-Fusion choices collected in step 2.
    if want_ffmpeg {
        pkg::enable_rpmfusion_free(dry_run).map_err(|e| format!("RPM Fusion (free) failed: {e}"))?;
        pkg::swap_ffmpeg_full(dry_run).map_err(|e| format!("ffmpeg swap failed: {e}"))?;
    }
    if want_mesa_va {
        install_va(&[MESA_VA_FREEWORLD.to_string()], false, dry_run)?;
    }
    if want_intel_media {
        install_va(&[INTEL_MEDIA_DRIVER.to_string()], true, dry_run)?;
    }

    // Nourish ships no NVIDIA driver; if the hardware is here, just report its state.
    if gpu == pkg::Gpu::Nvidia {
        warn_nvidia_driver(pkg::nvidia_driver_status());
    }
    Ok(())
}

/// Enable RPM Fusion (free, plus nonfree when `nonfree`) and install `pkgs`. The nonfree
/// repo builds on free, so free is enabled either way; both enables are idempotent.
fn install_va(pkgs: &[String], nonfree: bool, dry_run: bool) -> Result<(), String> {
    pkg::enable_rpmfusion_free(dry_run).map_err(|e| format!("RPM Fusion (free) failed: {e}"))?;
    if nonfree {
        pkg::enable_rpmfusion_nonfree(dry_run).map_err(|e| format!("RPM Fusion (nonfree) failed: {e}"))?;
    }
    pkg::dnf_install(pkgs, dry_run).map_err(|e| format!("VA-API driver install failed: {e}"))
}

/// Print a prominent warning when an NVIDIA GPU is present without the proprietary
/// driver bound. No-op when the proprietary stack is already active.
fn warn_nvidia_driver(status: NvidiaDriver) {
    let reason = match status {
        NvidiaDriver::Proprietary => return,
        NvidiaDriver::Nouveau => "the open-source 'nouveau' driver is bound to it",
        NvidiaDriver::Missing => "no NVIDIA kernel driver is bound (the proprietary stack is missing)",
    };
    // Bold-yellow full-width rule banner (no right border, so the double-width ⚠ can't break it).
    const Y: &str = "\x1b[1;33m";
    const R: &str = "\x1b[0m";
    const RULE: &str = "════════════════════════════════════════════════════════════════════════";
    eprintln!("\n{Y}{RULE}{R}");
    eprintln!("{Y}  ⚠  NVIDIA GPU detected — the proprietary driver is NOT active{R}");
    eprintln!("{Y}{RULE}{R}");
    eprintln!("An NVIDIA GPU is present, but {reason}. Nourish renders on Vulkan and does NOT");
    eprintln!("install the NVIDIA driver for you — on nouveau it's slow/unstable. Install it");
    eprintln!("yourself, reboot, then re-run. RPM Fusion akmod (recommended):");
    eprintln!("  sudo dnf install rpmfusion-free-release rpmfusion-nonfree-release   # if not enabled");
    eprintln!("  sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda   # then wait for akmods + reboot");
    eprintln!("Or NVIDIA's .run installer (nvidia.com). Verify with `nvidia-smi`.");
    eprintln!("{Y}────────────────────────────────────────────────────────────────────────{R}");
}
