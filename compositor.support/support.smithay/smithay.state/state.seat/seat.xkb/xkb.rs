//! Keyboard layout (xkb) for the seat — the single home for the mapping from the
//! persisted `KeyboardLayout` preference to smithay's `XkbConfig`, plus loading the
//! preference and applying a config to a live keyboard.
//!
//! The seat factory calls [`load`] + [`config`] so the keyboard comes up with the
//! user's layout at startup; the settings window calls [`apply`] to hot-reload it.
//! `Env` leaves the config empty so libxkbcommon reads the `XKB_DEFAULT_RULES`,
//! `XKB_DEFAULT_MODEL`, `XKB_DEFAULT_LAYOUT`, `XKB_DEFAULT_VARIANT` and
//! `XKB_DEFAULT_OPTIONS` environment variables (the historical default); `Manual`
//! uses the explicit layout/variant/options (rules/model left to the xkb default).
use compositor_developer_environment_preference_base::base::{self as pref, KeyboardLayout, LayoutSource};
use smithay::input::SeatHandler;
use smithay::input::keyboard::{KeyboardHandle, XkbConfig};

/// Load the keyboard-layout preference fresh from preferences.json. A missing key
/// yields the default (`Env`), so behaviour is unchanged for existing configs.
pub fn load() -> KeyboardLayout {
    pref::load().keyboard
}

/// Map a layout preference to an [`XkbConfig`]. Borrows `k`; valid for the immediate
/// `add_keyboard`/`set_xkb_config` compile call.
pub fn config(k: &KeyboardLayout) -> XkbConfig<'_> {
    match k.source {
        LayoutSource::Env => XkbConfig::default(),
        LayoutSource::Manual => XkbConfig {
            layout: &k.layout,
            variant: &k.variant,
            options: if k.options.is_empty() { None } else { Some(k.options.clone()) },
            ..Default::default()
        },
    }
}

/// Apply a layout preference to a live keyboard: recompiles the keymap and
/// rebroadcasts it to the focused client. A malformed `Manual` config compiles to
/// an error, which is ignored — the keyboard keeps its previous (valid) keymap
/// rather than losing input. `keyboard` must be an OWNED handle (clone it via
/// `get_keyboard()`) so `data` can be borrowed mutably alongside it.
pub fn apply<D: SeatHandler + 'static>(keyboard: &KeyboardHandle<D>, data: &mut D, k: &KeyboardLayout) {
    let _ = keyboard.set_xkb_config(data, config(k));
}
