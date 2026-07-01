//! The settings window `IcedUi`: owns the live UI state; rendering lives in
//! `surface.chrome`. All edit messages forward to the surface handler; `Tab`, the
//! display selection, and the optimistic confirm state are handled locally.
use compositor_developer_environment_config_base::base::Environment;
use compositor_developer_environment_preference_base::base::Ime;
use compositor_developer_environment_keybinding_base::base::KeyRow;
use compositor_orchestration_driver_output_base::base::{ApplyResult, DisplayInfo, ModeInfo, OutputsSnapshot};
use compositor_support_iced_core_engine_base::{IcedUi, Renderer};
use compositor_y5_audio_controller_interface::interface::AudioState;
use compositor_configurator_network_backend_base::base::WifiSnapshot;
use compositor_configurator_bluetooth_backend_base::base::BtSnapshot;
use compositor_configurator_hardware_gpu_base::base::{render_devices, RenderDevice};
use compositor_configurator_settings_surface_chrome::chrome;
use compositor_configurator_settings_surface_message::message::{Applied, SettingsMessage, Tab};
use iced_core::{Element, Theme};

pub struct Settings {
    pub tab: Tab,
    pub cursor_sensitivity: f32,
    pub natural_scroll: bool,
    pub env: Environment,
    /// Input-method launch command (Misc tab), persisted to preferences.json.
    pub ime: Ime,
    pub dirty: bool,
    /// Every connected monitor (active + connected-but-inactive), for the picker.
    pub displays: Vec<DisplayInfo>,
    /// EDID key of the monitor currently driving the compositor.
    pub active_edid: String,
    /// EDID key of the monitor selected in the picker (defaults to active).
    pub selected_display: String,
    /// Mode selected for the selected monitor (defaults to its current/first).
    pub selected_mode: Option<ModeInfo>,
    /// The change awaiting Keep/Revert (drives the confirm bar + commit on result).
    pub pending: Option<Applied>,
    pub confirming: bool,
    /// Shortcut rows for the Keys tab (id, label, default, current combo).
    pub keys: Vec<KeyRow>,
    /// Live system snapshots (pushed in per-frame via `SyncSystem`).
    pub audio: AudioState,
    pub wifi: WifiSnapshot,
    pub bt: BtSnapshot,
    /// Wi-Fi UI state: the secured network awaiting a password + the typed value.
    pub wifi_selected: Option<String>,
    pub wifi_password: String,
    /// Available render nodes with estimated GPU names (System tab picker).
    pub render_devices: Vec<RenderDevice>,
    /// Live frames-per-second (pushed by the embed), shown on the Display panel.
    pub fps: u32,
}

/// The mode to seed the picker selection with for a display: its current mode if
/// active, else its first advertised mode.
fn default_mode(d: &DisplayInfo) -> Option<ModeInfo> {
    // Active monitor: its live mode. Otherwise the mode SAVED IN PREFERENCES for
    // this monitor, then the recommended (first advertised) as a last resort.
    d.current.or(d.preferred).or_else(|| d.available.first().copied())
}

impl Settings {
    pub fn new(env: Environment, cursor: f32, natural: bool, snap: OutputsSnapshot, keys: Vec<KeyRow>, tab: Tab, ime: Ime) -> Self {
        let active = snap.displays.iter().find(|d| d.active).cloned();
        let active_edid = active.as_ref().map(|d| d.edid_key.clone()).unwrap_or_default();
        let selected_mode = active.as_ref().and_then(default_mode);
        Self {
            tab,
            cursor_sensitivity: cursor,
            natural_scroll: natural,
            env,
            ime,
            dirty: false,
            displays: snap.displays,
            active_edid: active_edid.clone(),
            selected_display: active_edid,
            selected_mode,
            pending: None,
            confirming: false,
            keys,
            audio: AudioState::default(),
            wifi: WifiSnapshot::default(),
            bt: BtSnapshot::default(),
            wifi_selected: None,
            wifi_password: String::new(),
            render_devices: render_devices(),
            fps: 0,
        }
    }

    fn display(&self, key: &str) -> Option<&DisplayInfo> {
        self.displays.iter().find(|d| d.edid_key == key)
    }

    /// Reset the picker selection back to the active monitor + its current mode.
    fn reset_selection(&mut self) {
        self.selected_display = self.active_edid.clone();
        self.selected_mode = self.display(&self.active_edid).and_then(default_mode);
    }
}

impl IcedUi for Settings {
    type Message = SettingsMessage;

    fn update(&mut self, message: SettingsMessage) {
        match message {
            SettingsMessage::Tab(t) => self.tab = t,
            SettingsMessage::Fps(f) => self.fps = f,
            // Kernel outcome of the provisional apply: commit on Confirmed (a
            // switch makes the target the active monitor), reset otherwise.
            SettingsMessage::ModeResult(r) => match r {
                ApplyResult::Confirmed => {
                    if let Some(p) = self.pending.take() {
                        // Update the in-memory snapshot so the active monitor's
                        // `current` reflects the just-applied mode (and, for a real
                        // switch, the active flag moves). Without this the snapshot
                        // is stale: re-selecting the previous mode would read as "no
                        // change", greying out CHECK, so you couldn't revert/redo in
                        // the same session.
                        for d in &mut self.displays {
                            if d.edid_key == p.edid_key {
                                d.active = true;
                                d.current = Some(p.mode);
                            } else if p.switch {
                                d.active = false;
                                d.current = None;
                            }
                        }
                        self.active_edid = p.edid_key;
                        self.selected_display = self.active_edid.clone();
                        self.selected_mode = Some(p.mode);
                    }
                    self.confirming = false;
                }
                ApplyResult::Reverted | ApplyResult::Failed => {
                    self.pending = None;
                    self.confirming = false;
                    self.reset_selection();
                }
                ApplyResult::Provisional => {}
            },
            SettingsMessage::Cursor(v) => self.cursor_sensitivity = v,
            SettingsMessage::NaturalScroll(b) => self.natural_scroll = b,
            SettingsMessage::Env(e) => {
                self.env = e;
                self.dirty = true;
            }
            SettingsMessage::Ime(i) => self.ime = i,
            SettingsMessage::SelectDisplay(key) => {
                self.selected_display = key.clone();
                self.selected_mode = self.display(&key).and_then(default_mode);
            }
            SettingsMessage::SelectMode(m) => self.selected_mode = Some(m),
            SettingsMessage::Apply(a) => {
                self.pending = Some(a);
                self.confirming = true;
            }
            SettingsMessage::Keep(_) => self.confirming = false,
            SettingsMessage::Revert => {
                self.pending = None;
                self.confirming = false;
                self.reset_selection();
            }
            SettingsMessage::Rebind(id, combo) => {
                if let Some(r) = self.keys.iter_mut().find(|r| r.id == id) { r.combo = combo; }
            }
            SettingsMessage::ResetBind(id) => {
                if let Some(r) = self.keys.iter_mut().find(|r| r.id == id) { r.combo = r.default.clone(); }
            }
            SettingsMessage::SyncSystem(a, w, b) => {
                self.audio = a;
                self.wifi = w;
                self.bt = b;
            }
            // Live hotplug refresh of the monitor list. Skip while a provisional
            // change is being confirmed (the snapshot is mid-transition then), and
            // preserve the user's picker selection when that monitor still exists.
            SettingsMessage::SyncDisplays(displays) => {
                if !self.confirming {
                    self.active_edid = displays
                        .iter()
                        .find(|d| d.active)
                        .map(|d| d.edid_key.clone())
                        .unwrap_or_default();
                    if !displays.iter().any(|d| d.edid_key == self.selected_display) {
                        self.selected_display = self.active_edid.clone();
                        self.selected_mode =
                            displays.iter().find(|d| d.edid_key == self.selected_display).and_then(default_mode);
                    }
                    self.displays = displays;
                }
            }
            SettingsMessage::WifiSelect(ssid) => {
                self.wifi_selected = Some(ssid);
                self.wifi_password.clear();
            }
            SettingsMessage::WifiPassword(p) => self.wifi_password = p,
            SettingsMessage::WifiConnect(..) => {
                self.wifi_selected = None;
                self.wifi_password.clear();
            }
            // Forwarded-only actions: no local UI state change.
            SettingsMessage::SetDefaultSink(_)
            | SettingsMessage::SetSinkVolume(_, _)
            | SettingsMessage::WifiEnable(_)
            | SettingsMessage::WifiScan
            | SettingsMessage::BtPower(_)
            | SettingsMessage::BtScan(_)
            | SettingsMessage::BtPair(_)
            | SettingsMessage::BtConnect(_)
            | SettingsMessage::Close => {}
        }
    }

    fn view(&self) -> Element<'_, SettingsMessage, Theme, Renderer> {
        chrome::render(
            self.tab,
            self.dirty,
            self.cursor_sensitivity,
            self.natural_scroll,
            &self.env,
            &self.displays,
            &self.active_edid,
            &self.selected_display,
            self.selected_mode,
            self.pending.as_ref(),
            self.confirming,
            &self.keys,
            &self.audio,
            &self.wifi,
            &self.bt,
            self.wifi_selected.as_deref(),
            &self.wifi_password,
            &self.render_devices,
            self.fps,
            &self.ime,
        )
    }
}
