//! The settings-window message type, shared by the view + tab builders + the
//! surface protocol/handler. iced-free so the protocol crate can name it.
use compositor_developer_environment_config_base::base::Environment;
use compositor_developer_environment_preference_base::base::{Ime, KeyboardLayout};
use compositor_orchestration_driver_output_base::base::{ApplyResult, DisplayInfo, ModeInfo};

/// A provisional display change the user can Keep/Revert: a target monitor
/// (by EDID identity key), the mode to drive it at, and whether this switches
/// the ACTIVE output (different monitor → output-switch gate) or just changes
/// the mode on the active monitor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub edid_key: String,
    pub mode: ModeInfo,
    pub switch: bool,
}
use compositor_y5_audio_controller_interface::interface::AudioState;
use compositor_configurator_network_backend_base::base::WifiSnapshot;
use compositor_configurator_bluetooth_backend_base::base::BtSnapshot;

/// The settings modules shown in the sidebar (design: SYSTEM CONFIGURATION).
/// `Input` merges the former Cursor + Keys; `System` is the Environment editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Display,
    Audio,
    Input,
    Network,
    Bluetooth,
    Performance,
    System,
    Misc,
}

impl Tab {
    /// Stable index for session persistence in the driver `SettingsState`
    /// (orchestration can't name `Tab`, so the selected module round-trips as a `u8`).
    pub fn to_index(self) -> u8 {
        match self {
            Tab::Display => 0, Tab::Audio => 1, Tab::Input => 2, Tab::Network => 3,
            Tab::Bluetooth => 4, Tab::Performance => 5, Tab::System => 6, Tab::Misc => 7,
        }
    }
    pub fn from_index(i: u8) -> Self {
        match i {
            1 => Tab::Audio, 2 => Tab::Input, 3 => Tab::Network, 4 => Tab::Bluetooth,
            5 => Tab::Performance, 6 => Tab::System, 7 => Tab::Misc, _ => Tab::Display,
        }
    }
}

#[derive(Clone, Debug)]
pub enum SettingsMessage {
    /// Switch the visible tab (UI-only, not forwarded).
    Tab(Tab),
    /// Live frames-per-second, pushed by the embed (UI-only, not forwarded).
    Fps(u32),
    /// Kernel result of the last mode Apply, pushed by the embed (UI-only): drops
    /// the confirm bar + restores the shown mode when a mode wasn't kept.
    ModeResult(ApplyResult),
    /// Live cursor speed multiplier (forwarded: applied + persisted at once).
    Cursor(f32),
    /// Live touchpad natural-scroll (forwarded).
    NaturalScroll(bool),
    /// A full edited Environment to write back to settings.json (forwarded;
    /// sets the reboot-dirty banner). Carrying the whole struct keeps one
    /// message variant instead of 19 field-specific ones.
    Env(Environment),
    /// A full edited input-method command (Misc tab) to persist to preferences.json
    /// (forwarded). Carries the whole `Ime` like `Env`, so exec + args edits share one
    /// variant. Applied on the next compositor start.
    Ime(Ime),
    /// A full edited keyboard-layout preference (Misc tab) to persist to
    /// preferences.json AND apply live (forwarded). Carries the whole
    /// `KeyboardLayout` like `Ime`/`Env`, so the source toggle + layout/variant/
    /// options edits share one variant.
    Keyboard(KeyboardLayout),
    /// Select a monitor in the Display picker (UI-local: syncs the mode list).
    SelectDisplay(String),
    /// Select a mode for the selected monitor (UI-local).
    SelectMode(ModeInfo),
    /// Provisionally apply the selected monitor + mode (forwarded): a mode change
    /// on the active monitor, or an active-output switch to another monitor.
    Apply(Applied),
    /// Keep the provisional change (forwarded: confirm + persist preferred
    /// monitor and/or per-EDID mode).
    Keep(Applied),
    /// Revert the provisional change (forwarded: reverts whichever gate is armed).
    Revert,
    /// Rebind a shortcut: `(action_id, combo_string)` (forwarded: parsed +
    /// persisted to keybinding.json).
    Rebind(String, String),
    /// Reset a shortcut to its default (forwarded: clears the override).
    ResetBind(String),
    /// Live system snapshots pushed in by the per-frame reconciler (NOT forwarded).
    SyncSystem(AudioState, WifiSnapshot, BtSnapshot),
    /// Live connected-monitor list pushed in on hotplug (NOT forwarded): refreshes
    /// the Display picker for the open session.
    SyncDisplays(Vec<DisplayInfo>),
    /// Audio (forwarded): make a sink default / set a sink's volume.
    SetDefaultSink(String),
    SetSinkVolume(String, f32),
    /// Wi-Fi: enable/scan/connect are forwarded; Select/Password are UI-local.
    WifiEnable(bool),
    WifiScan,
    WifiSelect(String),
    WifiPassword(String),
    WifiConnect(String, String),
    /// Bluetooth (forwarded): power, scan, pair/connect by device path.
    BtPower(bool),
    BtScan(bool),
    BtPair(String),
    BtConnect(String),
    /// Close the settings window (forwarded).
    Close,
}
