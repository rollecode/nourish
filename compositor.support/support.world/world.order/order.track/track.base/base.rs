use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use uuid::Uuid;

/// Opaque, RENDERER-AGNOSTIC identity of a draw component. DrawOrder tracks only
/// this id + position; it deliberately does NOT know whether the component is a
/// window, an iced surface, or a bevy view — that mapping lives in the owning
/// system/driver (iced may itself become a driver). A drawable's owner resolves
/// its own ids; ids foreign to an owner are simply skipped in its draw pass.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ComponentId(pub Uuid);

/// Fractional z-position within a layer; lower = further back. Gaps let a
/// component be restacked between two others without renumbering the rest.
pub type OrderKey = f64;

/// Coarse stacking tier. LOWER draws further back (and hit-tests last); within a
/// tier, `OrderKey` (spawn/raise) decides. This is the ONE place a drawable's
/// layer affects z-order — both the draw pass and input hit-test read it through
/// `ordered()`, so they can never disagree (no per-renderer/registry layering).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct DrawLayer(pub i32);

impl DrawLayer {
    /// Default tier: windows + ordinary world iced (placeholders, launcher).
    pub const CONTENT: DrawLayer = DrawLayer(0);
    /// Group frames sit beneath the windows they contain.
    pub const GROUP: DrawLayer = DrawLayer(-100);
    /// World-space overlays that sit ABOVE all windows (e.g. the selection
    /// toolbar in its world-at-cursor placement).
    pub const OVERLAY: DrawLayer = DrawLayer(100);
}

/// The spatial world's NON-DESTRUCTIVE draw-order registry: it stores only
/// identity + tier + position (never the component data, which lives in its
/// owning system) and is the single z-order authority, superseding smithay
/// `Space` stacking. Spawn appends to the top of its tier; raise restacks within
/// the tier; close removes.
#[derive(Default)]
pub struct DrawOrder {
    entries: Vec<(ComponentId, DrawLayer, OrderKey)>,
    next: OrderKey,
}

impl DrawOrder {
    pub fn new() -> Self {
        Self { entries: Vec::new(), next: 1.0 }
    }

    /// Insert `id` at the top of `layer` (re-insert re-raises AND updates the
    /// tier). Returns the assigned key.
    pub fn insert_top(&mut self, id: ComponentId, layer: DrawLayer) -> OrderKey {
        let key = self.next;
        self.next += 1.0;
        match self.entries.iter_mut().find(|(c, _, _)| *c == id) {
            Some(entry) => {
                entry.1 = layer;
                entry.2 = key;
            }
            None => self.entries.push((id, layer, key)),
        }
        key
    }

    pub fn remove(&mut self, id: ComponentId) {
        self.entries.retain(|(c, _, _)| *c != id);
    }

    /// Move `id` above every component in its OWN tier (keeps its tier). Lazily
    /// registers an unknown id at the CONTENT tier (raise is a window/content op).
    pub fn raise(&mut self, id: ComponentId) {
        if self.entries.iter().any(|(c, _, _)| *c == id) {
            let key = self.next;
            self.next += 1.0;
            if let Some(entry) = self.entries.iter_mut().find(|(c, _, _)| *c == id) {
                entry.2 = key;
            }
        } else {
            self.insert_top(id, DrawLayer::CONTENT);
        }
    }

    pub fn key(&self, id: ComponentId) -> Option<OrderKey> {
        self.entries.iter().find(|(c, _, _)| *c == id).map(|(_, _, k)| *k)
    }

    /// Components back-to-front: by tier (ascending = further back first), then
    /// `OrderKey` within a tier. Reverse for front-to-back input hit-testing.
    pub fn ordered(&self) -> Vec<(ComponentId, OrderKey)> {
        let mut v = self.entries.clone();
        v.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)));
        v.into_iter().map(|(c, _, k)| (c, k)).collect()
    }
}

pub static DRAW_ORDER: Token<DrawOrder> = Token::new();
pub static DRAW_ORDER_MUT: TokenMut<DrawOrder> = TokenMut::new(&DRAW_ORDER);
