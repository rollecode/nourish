//! Overview-mode state slot (the Super+Tab overlay): a flag-gated presentational
//! overlay on the active world — window grid (Layout) / picker globe (World)
//! beneath a top menu bar, over a frozen blurred snapshot of the desktop.

use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_monitor_compositor_iced_base::HandleId;
use compositor_support_bevy_core_alloc_base::AllocatedDmabuf;
use compositor_y5_graphic_capture_registry::{CaptureHandle, SnapshotHandle};
use smithay::utils::{Physical, Rectangle};
use uuid::Uuid;

/// Height of the top menu bar (physical px). The bar is created at this height
/// (full width); the grid reserves the same top inset.
pub const MENU_BAR_HEIGHT: i32 = 48;

/// The three menu sections. `World`/`Settings` are placeholders; `Layout` is the
/// window grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    World,
    Layout,
    Settings,
}

/// The resolved freeze backdrop: a full-res blurred copy of the desktop, or the
/// sharp snapshot if the blur passes failed.
pub enum Backdrop {
    Blur(AllocatedDmabuf),
    Sharp(SnapshotHandle),
}

/// Freeze-backdrop lifecycle: capture the desktop over a couple of frames (the
/// overlay isn't drawn until it resolves, so the capture grabs the desktop, not
/// itself), then hold the blurred/sharp result. `Ready(None)` = no-capture dim.
pub enum Phase {
    Closed,
    Arming { entry: CaptureHandle, countdown: u8 },
    Ready(Option<Backdrop>),
}

/// Overview-mode slot (per-world; lives on the spawn-target / active world).
pub struct Overview {
    pub visible: bool,
    pub tab: Tab,
    /// The screen-space iced menu-bar surface (raw id; created/destroyed by the
    /// interface), so this slot stays free of the iced UI type.
    pub menu: Option<HandleId>,
    /// Freeze-backdrop capture lifecycle.
    pub phase: Phase,
    /// Vertical grid scroll offset (physical px), clamped at render.
    pub scroll: f64,
    /// Last-rendered cell rects (uuid → screen rect) for click hit-testing.
    pub cells: Vec<(Uuid, Rectangle<i32, Physical>)>,
}

impl Overview {
    pub fn new() -> Self {
        Self {
            visible: false,
            tab: Tab::Layout,
            menu: None,
            phase: Phase::Closed,
            scroll: 0.0,
            cells: Vec::new(),
        }
    }

    /// True once the freeze backdrop is resolved (snapshot taken, or the
    /// no-capture fallback) — i.e. the overlay (grid + menu) may be shown.
    pub fn overlay_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready(_))
    }

    pub fn is_world(&self) -> bool {
        matches!(self.tab, Tab::World)
    }

    pub fn is_settings(&self) -> bool {
        matches!(self.tab, Tab::Settings)
    }
}

/// The overview slot token (read via the core `overview()` focus accessor).
pub static OVERVIEW: Token<Overview> = Token::new();
pub static OVERVIEW_MUT: TokenMut<Overview> = TokenMut::new(&OVERVIEW);

/// Deferred overview action handled by the surface pump (holds the renderer).
#[derive(Debug, Clone)]
pub enum OverviewSurfaceMessage {
    /// Reconcile the menu-bar surface to match `visible`.
    Reconcile,
    /// Select a tab (from the menu bar's iced message handler).
    SetTab(Tab),
}
