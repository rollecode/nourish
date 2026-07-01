//! Viewport tree: a root `Viewport` of `Slot`s (+ `floating` panes); each slot owns a `Camera`.
use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_y5_camera_state_base::state::Camera;
use smithay::utils::{Physical, Rectangle};

/// Stable per-world slot identity (the shortcut target; also seeds damage Ids).
pub type SlotId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// An array of slots, or a floating pane wrapping a (splittable) viewport.
pub enum Viewport {
    Slots { axis: Axis, slots: Vec<Slot> },
    Floating { rect: Rectangle<i32, Physical>, inner: Box<Viewport> },
}

/// A drawing cell. `camera` is live for a leaf (`content` None); `weight` is its
/// size share within the parent `Slots` array (separator-drag adjusts it).
pub struct Slot {
    pub id: SlotId,
    pub camera: Camera,
    pub content: Option<Box<Viewport>>,
    pub weight: f64,
}

pub struct Viewports {
    pub root: Viewport,
    /// Detached panes overlaid on `root` (drawn on top); each a `Floating`.
    pub floating: Vec<Viewport>,
    /// Keyboard-shortcut target (split/detach) — set by clicking a pane.
    pub active: SlotId,
    /// Pane under the cursor — operative for all pointer input.
    pub pointer: SlotId,
    pub next_id: SlotId,
    /// Windows visible per leaf slot (refreshed each render, transient) — drives per-window fractional scale.
    pub visible: std::collections::HashMap<SlotId, Vec<uuid::Uuid>>,
}

pub static VIEWPORTS: Token<Viewports> = Token::new();
pub static VIEWPORTS_MUT: TokenMut<Viewports> = TokenMut::new(&VIEWPORTS);

impl Default for Viewports {
    fn default() -> Self {
        let slot = Slot { id: 0, camera: Camera::default(), content: None, weight: 1.0 };
        let root = Viewport::Slots { axis: Axis::Vertical, slots: vec![slot] };
        Viewports { root, floating: Vec::new(), active: 0, pointer: 0, next_id: 1, visible: std::collections::HashMap::new() }
    }
}

impl Viewport {
    /// Depth-first search for slot `id` (matches container slots too).
    pub fn find(&self, id: SlotId) -> Option<&Slot> {
        match self {
            Viewport::Slots { slots, .. } => slots.iter().find_map(|s| if s.id == id { Some(s) } else { s.content.as_ref().and_then(|v| v.find(id)) }),
            Viewport::Floating { inner, .. } => inner.find(id),
        }
    }
    pub fn find_mut(&mut self, id: SlotId) -> Option<&mut Slot> {
        match self {
            Viewport::Slots { slots, .. } => slots.iter_mut().find_map(|s| if s.id == id { Some(s) } else { s.content.as_mut().and_then(|v| v.find_mut(id)) }),
            Viewport::Floating { inner, .. } => inner.find_mut(id),
        }
    }
    /// First leaf in document order (the always-present fallback).
    pub fn first_leaf(&self) -> &Slot {
        match self {
            Viewport::Slots { slots, .. } => match &slots[0].content { Some(inner) => inner.first_leaf(), None => &slots[0] },
            Viewport::Floating { inner, .. } => inner.first_leaf(),
        }
    }
}

impl Viewports {
    fn panes(&self) -> impl Iterator<Item = &Viewport> {
        std::iter::once(&self.root).chain(self.floating.iter())
    }
    /// Camera of the slot with `id`, searched across the root AND floating panes.
    pub fn camera_of(&self, id: SlotId) -> Option<&Camera> {
        self.panes().find_map(|v| v.find(id)).map(|s| &s.camera)
    }
    pub fn camera_of_mut(&mut self, id: SlotId) -> Option<&mut Camera> {
        if self.root.find(id).is_some() {
            return self.root.find_mut(id).map(|s| &mut s.camera);
        }
        self.floating.iter_mut().find_map(|v| v.find_mut(id)).map(|s| &mut s.camera)
    }
    /// Operative camera (pane under the cursor, `pointer`; first leaf if stale).
    pub fn focus_camera(&self) -> &Camera {
        self.camera_of(self.pointer).unwrap_or_else(|| &self.root.first_leaf().camera)
    }
    pub fn focus_camera_mut(&mut self) -> &mut Camera {
        let id = if self.camera_of(self.pointer).is_some() { self.pointer } else { self.root.first_leaf().id };
        self.camera_of_mut(id).expect("first_leaf always resolves")
    }
}
