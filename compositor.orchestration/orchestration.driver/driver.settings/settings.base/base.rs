use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_monitor_compositor_iced_base::HandleId;

/// Settings-window driver data: the live screen-space iced surface (opened with
/// Super+. , closed from its button) plus UI lifecycle flags. `open` is the
/// desired state the keybinding toggles; the render-path reconciler in
/// `settings.interface` creates/destroys the surface to match (it needs the
/// GlesRenderer, available only on the draw path). `dirty` records that an
/// Environment (settings.json) field changed this session — those are read once
/// at startup, so the window shows a "reboot to apply" banner. Live preferences
/// (cursor speed, natural-scroll, output modes) apply immediately and never set it.
#[derive(Default)]
pub struct SettingsState {
    pub open: bool,
    pub handle: Option<HandleId>,
    pub dirty: bool,
    /// True only while the Performance tab is the visible settings module — the
    /// gate for pushing live FPS (so other tabs don't buffer per-frame updates).
    pub fps_wanted: bool,
}

pub static SETTINGS: Token<SettingsState> = Token::new();
pub static SETTINGS_MUT: TokenMut<SettingsState> = TokenMut::new(&SETTINGS);
