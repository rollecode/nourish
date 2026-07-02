//! The settings window `IcedUi`: owns the live UI state; rendering lives in
//! `surface.chrome`. All edit messages forward to the surface handler; `Tab`, the
//! display selection, and the optimistic confirm state are handled locally.
use compositor_developer_environment_config_base::base::Environment;
use compositor_developer_environment_keybinding_base::base::KeyRow;
use compositor_developer_environment_preference_base::base::LayoutPlacement;
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
    /// The cursor-teleport layout squares (Display tab canvas, multi-monitor). Each
    /// is a monitor (`identity`) placed at an abstract `(x,y)` with side `size`.
    pub layout: Vec<LayoutPlacement>,
    /// Selected placement on the canvas (highlights it + selects its monitor).
    pub selected_placement: Option<u64>,
    /// Next placement id to hand out (monotonic within the session).
    pub next_placement_id: u64,
}

/// A new square's side length in abstract layout units.
const PLACE_SIZE: f32 = 120.0;
/// Minimum square side.
const MIN_SIZE: f32 = 60.0;
/// Edge-snap radius (abstract units).
const SNAP: f32 = 12.0;

fn rects_overlap(a: &LayoutPlacement, b: &LayoutPlacement) -> bool {
    a.x < b.x + b.size && b.x < a.x + a.size && a.y < b.y + b.size && b.y < a.y + a.size
}

/// Snap `p`'s edges to any other placement's edges within [`SNAP`].
fn snap_edges(p: &mut LayoutPlacement, others: &[LayoutPlacement]) {
    for o in others {
        if o.id == p.id {
            continue;
        }
        for (pe, oe) in [(p.x, o.x + o.size), (p.x, o.x), (p.x + p.size, o.x), (p.x + p.size, o.x + o.size)] {
            if (pe - oe).abs() <= SNAP {
                p.x += oe - pe;
            }
        }
        for (pe, oe) in [(p.y, o.y + o.size), (p.y, o.y), (p.y + p.size, o.y), (p.y + p.size, o.y + o.size)] {
            if (pe - oe).abs() <= SNAP {
                p.y += oe - pe;
            }
        }
    }
}

/// The mode to seed the picker selection with for a display: its current mode if
/// active, else its first advertised mode.
fn default_mode(d: &DisplayInfo) -> Option<ModeInfo> {
    // Active monitor: its live mode. Otherwise the mode SAVED IN PREFERENCES for
    // this monitor, then the recommended (first advertised) as a last resort.
    d.current.or(d.preferred).or_else(|| d.available.first().copied())
}

impl Settings {
    pub fn new(env: Environment, cursor: f32, natural: bool, snap: OutputsSnapshot, keys: Vec<KeyRow>, tab: Tab, layout: Vec<LayoutPlacement>) -> Self {
        let active = snap.displays.iter().find(|d| d.active).cloned();
        let active_edid = active.as_ref().map(|d| d.edid_key.clone()).unwrap_or_default();
        let selected_mode = active.as_ref().and_then(default_mode);
        let next_placement_id = layout.iter().map(|p| p.id + 1).max().unwrap_or(0);
        Self {
            tab,
            cursor_sensitivity: cursor,
            natural_scroll: natural,
            env,
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
            layout,
            selected_placement: None,
            next_placement_id,
        }
    }

    /// The layout as forwarded to the handler (persist + rebuild teleport).
    fn layout_commit(&self) -> SettingsMessage {
        SettingsMessage::LayoutCommit(self.layout.clone())
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
            // --- Teleport-layout canvas edits (UI-local; APPLY LAYOUT forwards) ---
            SettingsMessage::LayoutPlace(edid, x, y) => {
                let id = self.next_placement_id;
                self.next_placement_id += 1;
                let mut p = LayoutPlacement { id, identity: edid, x, y, size: PLACE_SIZE };
                // Nudge right until it doesn't overlap an existing square.
                while self.layout.iter().any(|o| rects_overlap(&p, o)) {
                    p.x += PLACE_SIZE + SNAP;
                }
                self.selected_display = p.identity.clone();
                self.selected_mode = self.display(&self.selected_display).and_then(default_mode);
                self.selected_placement = Some(id);
                self.layout.push(p);
            }
            SettingsMessage::LayoutMove(id, x, y) => {
                if let Some(mut moved) = self.layout.iter().find(|p| p.id == id).cloned() {
                    moved.x = x;
                    moved.y = y;
                    let others: Vec<_> = self.layout.iter().filter(|p| p.id != id).cloned().collect();
                    snap_edges(&mut moved, &others);
                    // Reject the move if it would overlap another square.
                    if !others.iter().any(|o| rects_overlap(&moved, o)) {
                        if let Some(slot) = self.layout.iter_mut().find(|p| p.id == id) {
                            slot.x = moved.x;
                            slot.y = moved.y;
                        }
                    }
                }
            }
            SettingsMessage::LayoutResize(id, size) => {
                let size = size.max(MIN_SIZE);
                if let Some(mut resized) = self.layout.iter().find(|p| p.id == id).cloned() {
                    resized.size = size;
                    let others: Vec<_> = self.layout.iter().filter(|p| p.id != id).cloned().collect();
                    if !others.iter().any(|o| rects_overlap(&resized, o)) {
                        if let Some(slot) = self.layout.iter_mut().find(|p| p.id == id) {
                            slot.size = size;
                        }
                    }
                }
            }
            SettingsMessage::LayoutSelect(id) => {
                self.selected_placement = Some(id);
                if let Some(p) = self.layout.iter().find(|p| p.id == id) {
                    self.selected_display = p.identity.clone();
                    self.selected_mode = self.display(&self.selected_display).and_then(default_mode);
                }
            }
            SettingsMessage::LayoutRemove(id) => {
                self.layout.retain(|p| p.id != id);
                if self.selected_placement == Some(id) {
                    self.selected_placement = None;
                }
            }
            // Forwarded (persist + rebuild teleport): the UI already holds the data.
            SettingsMessage::LayoutCommit(_) => {}
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
            &self.layout,
            self.selected_placement,
        )
    }
}
