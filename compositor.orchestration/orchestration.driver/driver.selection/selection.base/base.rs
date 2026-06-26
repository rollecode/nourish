use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_monitor_compositor_iced_base::HandleId;
use smithay::utils::{Physical, Point, Size};

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
    /// Last camera zoom the world toolbar was counter-scaled for (NaN = unset).
    pub prev_zoom: f64,
}

impl Default for SelectionOverlayState {
    fn default() -> Self {
        Self { handle: None, count: 0, prev_zoom: f64::NAN }
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

/// On-screen toolbar size in physical pixels — the size it keeps on screen at
/// ANY zoom (WorldAtCursor counter-scales the world surface to hold this).
pub const BAR_W: i32 = 440;
pub const BAR_H: i32 = 120;
/// Gap below the screen bottom (ScreenBottomCenter).
pub const SCREEN_BOTTOM_MARGIN: i32 = 100;
/// On-screen gap below the cursor, in physical px (WorldAtCursor).
pub const CURSOR_DY: f64 = 12.0;
/// Lower bound on the zoom used for counter-scaling, so the world dmabuf
/// (`BAR / zoom`) can't explode past GPU limits when zoomed far out. Below this
/// the toolbar stops growing (and so begins to shrink on screen).
pub const MIN_ZOOM: f64 = 0.15;

/// World footprint that renders to ~`BAR_W`×`BAR_H` ON SCREEN at the given zoom
/// (a World item's screen size = world size × zoom, so world size = base/zoom).
pub fn world_size(zoom: f64) -> Size<i32, Physical> {
    let z = zoom.max(MIN_ZOOM);
    Size::from((
        ((BAR_W as f64) / z).round().max(1.0) as i32,
        ((BAR_H as f64) / z).round().max(1.0) as i32,
    ))
}

/// iced scale factor for the counter-scaled surface so the content lays out at
/// the native `BAR` logical size and fills the (larger, when zoomed out) dmabuf.
pub fn world_scale_factor(zoom: f64) -> f32 {
    (1.0 / zoom.max(MIN_ZOOM)) as f32
}

/// World-physical top-left so the (on-screen constant `BAR_W`-wide) toolbar is
/// centered horizontally on, and just below, the cursor. `cursor` is the live
/// world-logical cursor (seat `current_location`); `scale` the output scale;
/// world iced stores location in `logical × scale` units (like placeholders).
pub fn world_loc_under_cursor(cursor: (f64, f64), scale: f64, zoom: f64) -> Point<i32, Physical> {
    let size = world_size(zoom);
    let z = zoom.max(MIN_ZOOM);
    Point::from((
        (cursor.0 * scale - (size.w as f64) / 2.0).round() as i32,
        (cursor.1 * scale + CURSOR_DY / z).round() as i32,
    ))
}
