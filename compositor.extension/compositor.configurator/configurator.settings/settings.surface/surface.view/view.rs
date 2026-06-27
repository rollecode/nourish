//! The settings window `IcedUi`: owns the live UI state; rendering lives in
//! `surface.chrome`. All edit messages forward to the surface handler; `Tab` and
//! the optimistic `current`/confirm state are handled locally.
use compositor_developer_environment_config_base::base::Environment;
use compositor_developer_environment_keybinding_base::base::KeyRow;
use compositor_orchestration_driver_output_base::base::{ModeInfo, OutputModesSnapshot};
use compositor_support_iced_core_engine_base::{IcedUi, Renderer};
use compositor_y5_audio_controller_interface::interface::AudioState;
use compositor_configurator_network_backend_base::base::WifiSnapshot;
use compositor_configurator_bluetooth_backend_base::base::BtSnapshot;
use compositor_configurator_hardware_gpu_base::base::{render_devices, RenderDevice};
use compositor_configurator_settings_surface_chrome::chrome;
use compositor_configurator_settings_surface_message::message::{SettingsMessage, Tab};
use iced_core::{Element, Theme};

pub struct Settings {
    pub tab: Tab,
    pub cursor_sensitivity: f32,
    pub natural_scroll: bool,
    pub env: Environment,
    pub dirty: bool,
    pub modes: Vec<ModeInfo>,
    /// The mode shown as active. Updated optimistically when a mode is picked,
    /// restored to `baseline` on revert.
    pub current: Option<ModeInfo>,
    /// The last confirmed (kept) mode — the revert target for `current`.
    pub baseline: Option<ModeInfo>,
    pub picked: Option<ModeInfo>,
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

impl Settings {
    pub fn new(env: Environment, cursor: f32, natural: bool, snap: OutputModesSnapshot, keys: Vec<KeyRow>) -> Self {
        Self {
            tab: Tab::Display,
            cursor_sensitivity: cursor,
            natural_scroll: natural,
            env,
            dirty: false,
            modes: snap.available,
            current: snap.current,
            baseline: snap.current,
            picked: None,
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
}

impl IcedUi for Settings {
    type Message = SettingsMessage;

    fn update(&mut self, message: SettingsMessage) {
        match message {
            SettingsMessage::Tab(t) => self.tab = t,
            SettingsMessage::Fps(f) => self.fps = f,
            SettingsMessage::Cursor(v) => self.cursor_sensitivity = v,
            SettingsMessage::NaturalScroll(b) => self.natural_scroll = b,
            SettingsMessage::Env(e) => {
                self.env = e;
                self.dirty = true;
            }
            SettingsMessage::PickMode(info) => {
                self.picked = Some(info);
                self.current = Some(info); // optimistic: mark the picked mode current
                self.confirming = true;
            }
            SettingsMessage::Keep(_) => {
                self.baseline = self.current; // commit the kept mode
                self.confirming = false;
            }
            SettingsMessage::Revert => {
                self.current = self.baseline; // restore the previously-confirmed mode
                self.confirming = false;
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
            &self.modes,
            self.current,
            self.picked,
            self.confirming,
            &self.keys,
            &self.audio,
            &self.wifi,
            &self.bt,
            self.wifi_selected.as_deref(),
            &self.wifi_password,
            &self.render_devices,
            self.fps,
        )
    }
}
