//! dnf package groups, GPU-vendor detection, and the `dnf install` runner.
//!
//! Façade: the implementation lives in the sibling `enumerate.*` crates; this
//! crate re-exports the original public surface unchanged. Pure std.

pub use compositor_installer_process_packages_enumerate_groups::groups;
pub use compositor_installer_process_packages_enumerate_install::{
    dnf_install, enable_rpmfusion_free, enable_rpmfusion_nonfree, swap_ffmpeg_full,
};
pub use compositor_installer_process_packages_enumerate_model::{
    Gpu, NvidiaDriver, PackageGroup, capture_encoder_for, detect_gpu, nvidia_driver_status,
};
