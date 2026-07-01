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
        }
        SettingsMessage::Revert => {
            // Revert whichever gate is armed (both are no-ops if nothing pending).
            *state.inner.kernel.get_mut(&OUTPUT_MODE_REQUEST_MUT) = Some(OutputModeRequest::Revert);
            *state.inner.kernel.get_mut(&OUTPUT_SWITCH_REQUEST_MUT) = Some(OutputSwitchRequest::Revert);
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
        // Set the CURRENT world's background-shader override: write it into the
        // world's own `Two` slot (persisted by `BackgroundDoc` on mark), and clear
        // the instance so `TwoSystem::update` rebuilds next frame. Empty = default.
        SettingsMessage::SetWorldShader(name) => {
            let world = state.inner.worlds.active_id();
            if let Some(two) = state
                .inner
                .worlds
                .active_mut()
                .storage_mut()
                .try_get_mut(&compositor_background_two_system_base::base::BG_TWO_MUT)
            {
                two.background_shader = if name.is_empty() { None } else { Some(name) };
                two.instance = None;
                compositor_support_system_persist_mark_base::base::mark_world(world, true);
            }
        }
        // Set the current world's shader params: store the full vector on the
        // world's `Two` slot (persisted, debounced — drags fire fast) and update
        // the live instance in place so the background reacts without a rebuild.
        SettingsMessage::SetWorldShaderParams(values) => {
            let world = state.inner.worlds.active_id();
            if let Some(two) = state
                .inner
                .worlds
                .active_mut()
                .storage_mut()
                .try_get_mut(&compositor_background_two_system_base::base::BG_TWO_MUT)
            {
                two.params = values.clone();
                // Map the name-keyed overrides onto the live instance's param
                // slots (slot = the prop's index in the selected shader's props).
                let selection = two.background_shader.clone().or_else(
                    compositor_developer_stats_registry_base::base::background_shader_default,
                );
                let props = match &selection {
                    Some(sel) => compositor_background_two_shader_load::properties_for(sel),
                    None => compositor_background_two_shader_builtin::builtin_props(),
                };
                if let Some(inst) = two.instance.as_mut() {
                    for (name, val) in &values {
                        if let Some(slot) = props.iter().position(|p| &p.name == name) {
                            if slot < 8 {
                                inst.params[slot] = *val;
                            }
                        }
                    }
                }
                compositor_support_system_persist_mark_base::base::mark_world(world, false);
            }
        }
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
            }
        }
        SettingsMessage::Fps(_)
        | SettingsMessage::Tick
        | SettingsMessage::ModeResult(_)
        | SettingsMessage::SyncSystem(..)
        | SettingsMessage::SyncDisplays(_)
        | SettingsMessage::SyncShaders(..)
        | SettingsMessage::SyncShaderProps(..)
        | SettingsMessage::SyncShaderPreview(..)
        | SettingsMessage::SyncShaderStatus(..)
        | SettingsMessage::SelectDisplay(_)
        | SettingsMessage::SelectMode(_)
        | SettingsMessage::WifiSelect(_)
        | SettingsMessage::WifiPassword(_) => {}
    }
}
