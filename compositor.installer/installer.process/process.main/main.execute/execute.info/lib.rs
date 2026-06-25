//! Non-interactive installer outputs: `--emit-presets` and `--help`.

use compositor_installer_process_config_parse_base as cfg;
use compositor_installer_process_layout_compute_base as layout;
use compositor_installer_process_packages_enumerate_base as pkg;

/// Non-interactive: print the default preset's identity + complete settings JSON (one
/// per line, `id<TAB>desktop_name<TAB>json`) for the single Y5 Compositor session. The
/// JSON is built exactly as the installer seeds it (shared schema + defaults). Used by CI.
pub fn emit_presets() {
    let base = cfg::BaseConfig::default();
    let encoder = pkg::capture_encoder_for(pkg::detect_gpu());
    for p in &cfg::default_presets(&base, encoder) {
        println!("{}\t{}\t{}", p.id, p.desktop_name, layout::settings_json(p).replace('\n', " "));
    }
}

pub fn print_help() {
    println!(
        "y5-install — interactive y5 compositor installer\n\n\
         USAGE:\n  y5-install [--dry-run] [--help]\n\n\
         Run from the unzipped artifact directory (prebuilt binaries + templates\n\
         next to this executable, or set Y5_INSTALL_STAGE).\n\n\
         OPTIONS:\n  \
         -n, --dry-run   Print the dnf + file actions without changing anything.\n  \
         -h, --help      Show this help.\n"
    );
}
