//! The cursor-teleport layout and its edge-crossing lookup.
//!
//! The settings Display-tab canvas arranges square "placements" in an abstract
//! layout space. Each placement is a teleport zone for one physical monitor
//! (`key` = its EDID "make model serial"); the SAME monitor may have several
//! placements (duplicates = extra zones), so a placement is identified by a stable
//! `id`, not by its key. This space is PURELY about where the cursor crosses
//! between monitors — it never affects scale or resolution.
//!
//! When the pointer leaves the monitor it is currently on, [`TeleportLayout::neighbor`]
//! answers: which placement (if any) abuts the crossed edge at the crossing point,
//! and where along the entered edge (proportionally) the cursor should reappear.
//! Proportional `exit_frac`/`entry_frac` is the invariant that makes crossings
//! correct even when the two monitors differ in size or resolution.

/// The side of a placement the cursor crossed (or entered through).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// The edge on the far side — the side a neighbor is entered through.
    pub fn opposite(self) -> Edge {
        match self {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }
}

/// One placed monitor square in abstract layout space (`size` = side length; the
/// square spans `[x, x+size] × [y, y+size]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub id: u64,
    pub key: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
}

/// The result of a successful crossing: the entered placement plus where along its
/// entry edge the cursor lands (proportional, clamped to `[0, 1]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Neighbor {
    pub id: u64,
    pub key: String,
    /// The edge of the entered placement the cursor comes in through.
    pub entry_edge: Edge,
    /// Position along `entry_edge`, `0.0` = its start (top for L/R, left for T/B).
    pub entry_frac: f32,
}

/// The full arrangement of teleport zones (empty on single-monitor / no layout).
#[derive(Clone, Debug, Default)]
pub struct TeleportLayout {
    pub placements: Vec<Placement>,
}

impl TeleportLayout {
    pub fn new(placements: Vec<Placement>) -> Self {
        TeleportLayout { placements }
    }

    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&Placement> {
        self.placements.iter().find(|p| p.id == id)
    }

    /// The first placement of monitor `key`, if any — the zone the cursor starts in
    /// when it lands on that monitor with no more specific placement known.
    pub fn first_of(&self, key: &str) -> Option<&Placement> {
        self.placements.iter().find(|p| p.key == key)
    }

    /// Given the cursor is leaving placement `from_id` across `edge` at `exit_frac`
    /// (`0.0` = start of that edge: top for Left/Right, left for Top/Bottom), find
    /// the abutting placement to enter, if one covers the crossing point.
    pub fn neighbor(&self, from_id: u64, edge: Edge, exit_frac: f32) -> Option<Neighbor> {
        let from = self.get(from_id)?;
        let f = exit_frac.clamp(0.0, 1.0);
        // The crossing point in abstract space, on `from`'s `edge`.
        let (ex, ey) = match edge {
            Edge::Right => (from.x + from.size, from.y + f * from.size),
            Edge::Left => (from.x, from.y + f * from.size),
            Edge::Bottom => (from.x + f * from.size, from.y + from.size),
            Edge::Top => (from.x + f * from.size, from.y),
        };
        let entry_edge = edge.opposite();

        let mut best: Option<(&Placement, f32)> = None; // (placement, perpendicular gap)
        for q in &self.placements {
            if q.id == from_id {
                continue;
            }
            let tol = adj_tol(from.size, q.size);
            // The perpendicular coordinate where `q` must sit to abut `from`'s edge,
            // and the parallel span of `q`'s facing edge that must cover the crossing.
            let (gap, covers) = match entry_edge {
                // q entered through its LEFT edge → q sits to the right of `from`.
                Edge::Left => ((q.x - ex).abs(), within(ey, q.y, q.y + q.size)),
                Edge::Right => ((q.x + q.size - ex).abs(), within(ey, q.y, q.y + q.size)),
                Edge::Top => ((q.y - ey).abs(), within(ex, q.x, q.x + q.size)),
                Edge::Bottom => ((q.y + q.size - ey).abs(), within(ex, q.x, q.x + q.size)),
            };
            if gap <= tol && covers {
                if best.map_or(true, |(_, g)| gap < g) {
                    best = Some((q, gap));
                }
            }
        }

        let (q, _) = best?;
        // Where along `q`'s entry edge the cursor lands, proportionally.
        let entry_frac = match entry_edge {
            Edge::Left | Edge::Right => (ey - q.y) / q.size,
            Edge::Top | Edge::Bottom => (ex - q.x) / q.size,
        };
        Some(Neighbor {
            id: q.id,
            key: q.key.clone(),
            entry_edge,
            entry_frac: entry_frac.clamp(0.0, 1.0),
        })
    }
}

/// Adjacency tolerance: placements the settings canvas snapped to touch abut within
/// a hair; allow a small perpendicular gap proportional to the squares' size (min
/// 1 unit) so float noise and tiny gaps still cross.
fn adj_tol(a: f32, b: f32) -> f32 {
    (0.02 * (a + b) * 0.5).max(1.0)
}

/// `v` within `[lo, hi]` (inclusive), tolerant of ordering noise.
fn within(v: f32, lo: f32, hi: f32) -> bool {
    v >= lo - f32::EPSILON && v <= hi + f32::EPSILON
}
