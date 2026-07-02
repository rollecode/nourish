//! Applies forwarded settings-window messages (drained from the surface pump).
//! Preferences are mutated on the live `inner.preference` object (so the change
//! takes effect immediately) and then persisted to preferences.json. Environment
//! edits write settings.json (the UI already flagged the reboot banner); output
//! modes go via the OUTPUT_MODE_REQUEST channel (apply / confirm+persist / revert).
use compositor_developer_environment_preference_base::base as pref;
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_driver_output_base::base::{OutputModeRequest, OutputSwitchRequest, OUTPUT_MODES_SNAPSHOT, OUTPUT_MODE_REQUEST_MUT, OUTPUT_SWITCH_REQUEST_MUT};
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
        SettingsMessage::Ime(ime) => {
            // Persist the input-method launch command live to preferences.json. Applied
            // on the next compositor start (the IME is spawned once at boot).
            state.inner.preference.ime = Some(ime);
            let _ = pref::save(&state.inner.preference);
        }
        SettingsMessage::Keyboard(kl) => {
            // Persist AND apply the keyboard layout live: mutate the preference, save,
            // then recompile the keymap on the seat's keyboard. `get_keyboard()` hands
            // back an owned handle, so `&mut state.state` can be borrowed alongside the
            // `&state.inner.preference` read (disjoint fields of `Loop`).
            state.inner.preference.keyboard = kl;
            let _ = pref::save(&state.inner.preference);
            if let Some(keyboard) = state.state.seat.seat.get_keyboard() {
                compositor_support_smithay_state_seat_xkb::xkb::apply(
                    &keyboard,
                    &mut state.state,
                    &state.inner.preference.keyboard,
                );
            }
        }
        SettingsMessage::Apply(a) => {
            if a.switch {
                *state.inner.kernel.get_mut(&OUTPUT_SWITCH_REQUEST_MUT) = Some(OutputSwitchRequest::Apply {
                    edid_key: a.edid_key,
                    mode: Some(a.mode),
                });
            } else {
                *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Apply {
                    width: a.mode.width,
                    height: a.mode.height,
                    refresh_mhz: a.mode.refresh_mhz,
                });
            }
            state.inner.ping_control();
        }
        SettingsMessage::Keep(a) => {
            if a.switch {
                // Confirm the switch + persist the preferred monitor as the default
                // output: set its mode, then move it to the front of `outputs`
                // (`display.base` drives `profiles.first()` at startup).
                *state.inner.kernel.get_mut(&OUTPUT_SWITCH_REQUEST_MUT) = Some(OutputSwitchRequest::Confirm);
                pref::upsert_output(&mut state.inner.preference.outputs, &a.edid_key, pref::ModeRequest::Advertised {
                    width: a.mode.width,
                    height: a.mode.height,
                    refresh_mhz: a.mode.refresh_mhz,
                });
                pref::set_default(&mut state.inner.preference.outputs, &a.edid_key);
            } else {
                *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Confirm);
                let edid = state.inner.kernel.get(&OUTPUT_MODES_SNAPSHOT).edid_key.clone();
                pref::upsert_output(&mut state.inner.preference.outputs, &edid, pref::ModeRequest::Advertised {
                    width: a.mode.width,
                    height: a.mode.height,
                    refresh_mhz: a.mode.refresh_mhz,
                });
            }
            let _ = pref::save(&state.inner.preference);
            state.inner.ping_control();
        }
        SettingsMessage::Revert => {
            // Revert whichever gate is armed (both are no-ops if nothing pending).
            *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Revert);
            *state.inner.kernel.get_mut(&OUTPUT_SWITCH_REQUEST_MUT) = Some(OutputSwitchRequest::Revert);
            state.inner.ping_control();
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
            let st = state.inner.kernel.get_mut(&SETTINGS_MUT);
            st.fps_wanted = matches!(t, Tab::Performance);
            st.tab = t.to_index(); // remember the module for the session (restored on reopen)
            // Leaving Display abandons any provisional mode/switch change → revert
            // it (both are no-ops in the kernel if nothing is pending).
            if !matches!(t, Tab::Display) {
                *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Revert);
                *state.inner.kernel.get_mut(&OUTPUT_SWITCH_REQUEST_MUT) = Some(OutputSwitchRequest::Revert);
                state.inner.ping_control();
            }
        }
        SettingsMessage::Fps(_)
        | SettingsMessage::ModeResult(_)
        | SettingsMessage::SyncSystem(..)
        | SettingsMessage::SyncDisplays(_)
        | SettingsMessage::SelectDisplay(_)
        | SettingsMessage::SelectMode(_)
        | SettingsMessage::WifiSelect(_)
        | SettingsMessage::WifiPassword(_) => {}
    }
}
