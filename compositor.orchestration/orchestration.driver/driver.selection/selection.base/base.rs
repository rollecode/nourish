use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_monitor_compositor_iced_base::HandleId;
use smithay::utils::{Physical, Point};

/// Selection-overlay driver data: the live iced toolbar instance (created when
/// the selection becomes non-empty, destroyed when it empties) plus the
/// last-seen selection count used to gate redundant UI dispatches. The handle
/// is shared between the render-path reconciler (create/destroy/count) and the
/// `SelectSystem`-driven reposition (see `compositor_y5_select_overlay_system`).
pub struct SelectionOverlayState {
    /// The live toolbar instance, if one is currently shown.
    pub handle: Option<HandleId>,
    /// Last selection size pushed to the UI (avoids redundant dispatches).
    pub count: i32,
}

impl Default for SelectionOverlayState {
    fn default() -> Self {
        Self { handle: None, count: 0 }
    }
}

pub static SELECTION_OVERLAY: Token<SelectionOverlayState> = Token::new();
pub static SELECTION_OVERLAY_MUT: TokenMut<SelectionOverlayState> =
    TokenMut::new(&SELECTION_OVERLAY);

/// One-shot "re-anchor the toolbar to the cursor" flag. Set by
/// `compositor_y5_select_overlay_system` when it receives a selection-change
/// event; consumed (read + cleared) by the render-path reconciler, which has
/// the seat to read the live cursor. Lives in the spatial world's storage
/// (registered by the overlay system, resolved via the spawn-target accessor).
pub static SELECTION_REANCHOR: Token<bool> = Token::new();
pub static SELECTION_REANCHOR_MUT: TokenMut<bool> = TokenMut::new(&SELECTION_REANCHOR);

/// Where the selection toolbar is placed. Compile-time knob, shared by the
/// reconciler (placement at create) and the reposition system.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Fixed at the bottom-center of the screen (zoom-independent).
    ScreenBottomCenter,
    /// World-space, centered just below the cursor, above all windows. Scales
    /// with camera zoom and re-anchors to the cursor on each selection change.
    WorldAtCursor,
}

/// The active placement. Flip this constant to switch modes.
pub const SELECTION_OVERLAY_PLACEMENT: Placement = Placement::WorldAtCursor;

/// Toolbar size in physical pixels (its native, unzoomed size).
pub const BAR_W: i32 = 440;
pub const BAR_H: i32 = 120;
/// Gap below the screen bottom (ScreenBottomCenter).
pub const SCREEN_BOTTOM_MARGIN: i32 = 100;
/// Gap below the cursor, in world-physical px (WorldAtCursor).
pub const CURSOR_DY: f64 = 12.0;

/// World-physical top-left for the toolbar so a `BAR_W`-wide surface is centered
/// horizontally on, and just below, the cursor. `cursor` is `PointerState::motion`
/// and `scale` the output scale; world iced stores its location in `logical ×
/// scale` units (matching placeholder surfaces).
pub fn world_loc_under_cursor(cursor: (f64, f64), scale: f64) -> Point<i32, Physical> {
    Point::from((
        (cursor.0 * scale - (BAR_W as f64) / 2.0).round() as i32,
        (cursor.1 * scale + CURSOR_DY).round() as i32,
    ))
}
