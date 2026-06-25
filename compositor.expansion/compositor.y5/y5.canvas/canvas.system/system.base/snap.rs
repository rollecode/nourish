//! Edge snapping for canvas MOVE / SCALE grabs.
//!
//! The grab math produces an UNREALIZED geometry — `start_geo + raw_delta`, the
//! plain unsnapped drag (the historical behavior, and the ONLY thing tracked
//! across frames, so the snap "breaks out" cleanly). This module turns it into the
//! REALIZED geometry that is applied: the same drag with a per-axis correction
//! pulling the nearest moving edge onto the nearest snap line within
//! [`SNAP_RADIUS`]. The correction is a delta added to the raw delta, so the
//! existing per-window apply math (and its min-size clamps) is reused and a
//! multi-window grab keeps its relative offsets.
//!
//! The snap sources (other windows' / visible placeholders' rects + the screen
//! edges) are captured once at grab start into a [`SnapMap`]; this module consumes
//! it per frame, gating each window/placeholder source by the zoom-scaled
//! exclusion range against the (unrealized) moving geom.

use compositor_y5_canvas_input_state::state::SnapMap;
use smithay::utils::{Logical, Point, Rectangle, Size};

// --- KNOBS ---------------------------------------------------------------------

/// Base distance between a moving edge and a snap line for the edge to snap onto
/// it. World-logical units, but the EFFECTIVE radius is divided by the camera zoom
/// (see [`effective_radius`]) so it stays ~constant in SCREEN pixels: zooming in
/// shrinks the world radius, zooming out grows it.
pub const SNAP_RADIUS: f64 = 12.0;

/// Maximum world-space gap between the (unrealized) moving geom and a window /
/// placeholder source for that source's edges to stay snap candidates. The
/// effective range is this value DIVIDED BY the camera zoom, so zooming out (zoom
/// < 1) widens it. Set to `-1.0` for an infinite range (every source always
/// participates). Screen edges are never range-gated.
pub const SNAP_EXCLUSION_RANGE: f64 = 600.0;

/// When true, only windows / placeholders intersecting the current viewport are
/// added to the snap map at grab start (the viewport is derived from the camera
/// transform + screen size; see `build_snap_map`).
pub const SNAP_VISIBLE_ONLY: bool = true;

/// When true, the four edges of the camera VIEWPORT — the on-screen world region
/// (camera-centered, `screen / zoom`), i.e. the visible screen boundary the user
/// actually sees — are added as always-on snap lines. Distinct from the fixed
/// output-geometry edges: the viewport pans and scales with the camera, so this is
/// what lets a window/placeholder snap to the edge of what's currently shown.
pub const SNAP_VIEWPORT_EDGES: bool = true;

/// Margin by which [`SNAP_VISIBLE_ONLY`]'s viewport is inflated, so windows just
/// off the screen can still snap. World-logical units, zoom-scaled the SAME way as
/// [`SNAP_EXCLUSION_RANGE`] (divided by zoom — zooming out widens it). `0.0` keeps
/// the exact viewport; `-1.0` disables the visibility cull entirely (every source
/// participates). Only consulted when `SNAP_VISIBLE_ONLY` is true.
pub const SNAP_VISIBLE_ONLY_EXTEND_RANGE: f64 = 0.0;

/// The zoom-scaled margin to inflate the visible-only viewport by, or `None` for
/// "unbounded" (no visibility cull). Mirrors [`SNAP_EXCLUSION_RANGE`]'s scaling.
pub fn visible_extend(zoom: f64) -> Option<f64> {
    scale_range(SNAP_VISIBLE_ONLY_EXTEND_RANGE, zoom)
}

// --- CORRECTION ----------------------------------------------------------------

/// The correction added to the raw (unrealized) drag delta to obtain the realized
/// (snapped) delta. `dx`/`dy` are `0.0` on an axis with no line in range.
pub struct SnapCorrection {
    pub dx: f64,
    pub dy: f64,
}

/// Snap correction for a MOVE: all four edges of the translated `start` rect are
/// eligible (a move can align any side). `dx`/`dy` are the raw drag delta.
pub fn move_correction(
    snap: &SnapMap,
    start: Rectangle<f64, Logical>,
    dx: f64,
    dy: f64,
    zoom: f64,
) -> SnapCorrection {
    let moving = translate(start, dx, dy);
    let (vlines, hlines) = candidate_lines(snap, moving, zoom);
    let radius = effective_radius(zoom);
    let x_edges = [moving.loc.x, moving.loc.x + moving.size.w];
    let y_edges = [moving.loc.y, moving.loc.y + moving.size.h];
    SnapCorrection {
        dx: axis_correction(&vlines, &x_edges, radius),
        dy: axis_correction(&hlines, &y_edges, radius),
    }
}

/// Snap correction for a SCALE: only the moving edge on each axis is eligible —
/// the anchor pins the opposite one (`horizontal`/`vertical` true = right/bottom
/// moves). `dx`/`dy` are the raw resize delta.
pub fn scale_correction(
    snap: &SnapMap,
    start: Rectangle<f64, Logical>,
    dx: f64,
    dy: f64,
    horizontal: bool,
    vertical: bool,
    zoom: f64,
) -> SnapCorrection {
    let moving = resize_rect(start, dx, dy, horizontal, vertical);
    let (vlines, hlines) = candidate_lines(snap, moving, zoom);
    let radius = effective_radius(zoom);
    let x_edge = if horizontal { moving.loc.x + moving.size.w } else { moving.loc.x };
    let y_edge = if vertical { moving.loc.y + moving.size.h } else { moving.loc.y };
    SnapCorrection {
        dx: axis_correction(&vlines, &[x_edge], radius),
        dy: axis_correction(&hlines, &[y_edge], radius),
    }
}

/// The effective snap lines for this frame: the always-on screen edges plus each
/// source's edges. A non-visible source (offscreen at grab start) participates only
/// while within the (zoom-scaled) exclusion range of `moving`; a visible source
/// always participates (two on-screen windows snap regardless of distance).
fn candidate_lines(snap: &SnapMap, moving: Rectangle<f64, Logical>, zoom: f64) -> (Vec<f64>, Vec<f64>) {
    let mut vertical = snap.vertical.clone();
    let mut horizontal = snap.horizontal.clone();
    let range = effective_range(zoom);
    for source in &snap.sources {
        let s = source.rect.to_f64();
        if !source.visible {
            if let Some(r) = range {
                if rect_gap(moving, s) > r {
                    continue;
                }
            }
        }
        vertical.push(s.loc.x);
        vertical.push(s.loc.x + s.size.w);
        horizontal.push(s.loc.y);
        horizontal.push(s.loc.y + s.size.h);
    }
    (vertical, horizontal)
}

/// Best signed correction for one axis: over every (edge, line) pair the offset
/// `line - edge` whose magnitude is smallest and within `radius`, else `0.0`.
fn axis_correction(lines: &[f64], edges: &[f64], radius: f64) -> f64 {
    let mut best: Option<f64> = None;
    for &edge in edges {
        for &line in lines {
            let offset = line - edge;
            if offset.abs() > radius {
                continue;
            }
            if best.map_or(true, |b: f64| offset.abs() < b.abs()) {
                best = Some(offset);
            }
        }
    }
    best.unwrap_or(0.0)
}

/// The zoom-scaled snap radius: [`SNAP_RADIUS`] divided by the camera zoom, so the
/// snap distance is ~constant in screen pixels — zooming in (zoom > 1) shrinks it,
/// zooming out grows it.
fn effective_radius(zoom: f64) -> f64 {
    if zoom > 0.0 {
        SNAP_RADIUS / zoom
    } else {
        SNAP_RADIUS
    }
}

/// `None` (infinite) when [`SNAP_EXCLUSION_RANGE`] is negative, else the range
/// divided by `zoom` so zooming out widens it.
fn effective_range(zoom: f64) -> Option<f64> {
    scale_range(SNAP_EXCLUSION_RANGE, zoom)
}

/// Shared zoom scaling for the range knobs: `-1` (negative) → `None` (unbounded);
/// otherwise `base / zoom` so zooming out (zoom < 1) widens the range.
fn scale_range(base: f64, zoom: f64) -> Option<f64> {
    if base < 0.0 {
        None
    } else if zoom > 0.0 {
        Some(base / zoom)
    } else {
        Some(base)
    }
}

/// Minimum gap (Euclidean) between two rects; `0.0` if they overlap or touch.
fn rect_gap(a: Rectangle<f64, Logical>, b: Rectangle<f64, Logical>) -> f64 {
    let dx = (b.loc.x - (a.loc.x + a.size.w)).max(a.loc.x - (b.loc.x + b.size.w)).max(0.0);
    let dy = (b.loc.y - (a.loc.y + a.size.h)).max(a.loc.y - (b.loc.y + b.size.h)).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn translate(rect: Rectangle<f64, Logical>, dx: f64, dy: f64) -> Rectangle<f64, Logical> {
    Rectangle::new(Point::from((rect.loc.x + dx, rect.loc.y + dy)), rect.size)
}

/// The unrealized resized rect: the anchor pins one edge, the opposite edge
/// follows the delta (no min-size clamp — only used for snap geometry).
fn resize_rect(start: Rectangle<f64, Logical>, dx: f64, dy: f64, horizontal: bool, vertical: bool) -> Rectangle<f64, Logical> {
    let left = if horizontal { start.loc.x } else { start.loc.x + dx };
    let right = if horizontal { start.loc.x + start.size.w + dx } else { start.loc.x + start.size.w };
    let top = if vertical { start.loc.y } else { start.loc.y + dy };
    let bottom = if vertical { start.loc.y + start.size.h + dy } else { start.loc.y + start.size.h };
    Rectangle::new(Point::from((left, top)), Size::from((right - left, bottom - top)))
}
