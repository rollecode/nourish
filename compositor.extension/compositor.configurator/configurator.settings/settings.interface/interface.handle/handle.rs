//! Applies forwarded settings-window messages (drained from the surface pump).
//! Preferences are mutated on the live `inner.preference` object (so the change
//! takes effect immediately) and then persisted to preferences.json. Environment
//! edits write settings.json (the UI already flagged the reboot banner); output
//! modes go via the OUTPUT_MODE_REQUEST channel (apply / confirm+persist / revert).
use compositor_developer_environment_preference_base::base as pref;
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_driver_output_base::base::{OutputModeRequest, OUTPUT_MODES_SNAPSHOT, OUTPUT_MODE_REQUEST_MUT};
use compositor_orchestration_driver_settings_base::base::SETTINGS_MUT;
use compositor_configurator_settings_surface_message::message::{SettingsMessage, Tab};
use compositor_configurator_network_backend_base::base::{self as wifi, WifiCmd};
use compositor_configurator_bluetooth_backend_base::base::{self as bt, BtCmd};
use compositor_orchestration_driver_audio_base::base::AUDIO;
use smithay::backend::renderer::gles::GlesRenderer;

pub fn handle(state: &mut Loop, _renderer: &mut GlesRenderer, m: SettingsMessage) {
    match m {
        SettingsMessage::Cursor(v) => {
            state.inner.preference.cursor_sensitivity = v as f64;
            let _ = pref::save(&state.inner.preference);
        }
        SettingsMessage::NaturalScroll(b) => {
            state.inner.preference.input_natural_scroll = b;
            let _ = pref::save(&state.inner.preference);
        }
        SettingsMessage::Env(e) => {
            let _ = compositor_developer_environment_config_base::base::save(&e);
        }
        SettingsMessage::PickMode(info) => {
            *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Apply {
                width: info.width,
                height: info.height,
                refresh_mhz: info.refresh_mhz,
            });
        }
        SettingsMessage::Keep(info) => {
            *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Confirm);
            let edid = state.inner.kernel.get(&OUTPUT_MODES_SNAPSHOT).edid_key.clone();
            pref::upsert_output(&mut state.inner.preference.outputs, &edid, pref::ModeRequest::Advertised {
                width: info.width,
                height: info.height,
                refresh_mhz: info.refresh_mhz,
            });
            let _ = pref::save(&state.inner.preference);
        }
        SettingsMessage::Revert => {
            *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Revert);
        }
        SettingsMessage::Rebind(id, combo) => {
            state.inner.keybinding.set(&id, combo);
            let _ = compositor_developer_environment_keybinding_base::base::save(&state.inner.keybinding);
        }
        SettingsMessage::ResetBind(id) => {
            state.inner.keybinding.clear(&id);
            let _ = compositor_developer_environment_keybinding_base::base::save(&state.inner.keybinding);
        }
        SettingsMessage::SetDefaultSink(name) => {
            if let Some(a) = state.inner.kernel.get(&AUDIO) {
                let _ = a.set_default_sink(&name);
            }
        }
        SettingsMessage::SetSinkVolume(name, v) => {
            if let Some(a) = state.inner.kernel.get(&AUDIO) {
                let _ = a.set_sink_volume(&name, v as f64);
            }
        }
        SettingsMessage::WifiEnable(b) => wifi::command(WifiCmd::SetEnabled(b)),
        SettingsMessage::WifiScan => wifi::command(WifiCmd::Scan),
        SettingsMessage::WifiConnect(ssid, pw) => wifi::command(WifiCmd::Connect(ssid, pw)),
        SettingsMessage::BtPower(b) => bt::command(BtCmd::SetPowered(b)),
        SettingsMessage::BtScan(b) => bt::command(BtCmd::Scan(b)),
        SettingsMessage::BtPair(p) => bt::command(BtCmd::Pair(p)),
        SettingsMessage::BtConnect(p) => bt::command(BtCmd::Connect(p)),
        SettingsMessage::Close => {
            state.inner.kernel.get_mut(&SETTINGS_MUT).open = false;
        }
        // Inbound / UI-local — never forwarded to the handler.
        // Tab IS forwarded (so the compositor knows the visible module): gate the
        // live-FPS push on the Performance tab being open.
        SettingsMessage::Tab(t) => {
            state.inner.kernel.get_mut(&SETTINGS_MUT).fps_wanted = matches!(t, Tab::Performance);
            // Leaving Display abandons any provisional mode change → revert it
            // (no-op in the kernel if nothing is pending).
            if !matches!(t, Tab::Display) {
                *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Revert);
            }
        }
        SettingsMessage::Fps(_)
        | SettingsMessage::ModeResult(_)
        | SettingsMessage::SyncSystem(..)
        | SettingsMessage::WifiSelect(_)
        | SettingsMessage::WifiPassword(_) => {}
    }
}
