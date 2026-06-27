//! Overview grid geometry (pure): fixed-height, varying-width cells packed into
//! a centered grid, plus the shared cell-height heuristic.

use smithay::utils::{Physical, Point, Rectangle, Size};

/// The shared cell height (px), sized to a roughly constant *physical* height
/// (small on a big low-DPI display, bigger on a high-DPI laptop) via EDID mm when
/// known, else a resolution fraction; floored at `MIN_PX`.
pub fn cell_height(area_h: i32, mode_h_px: i32, physical_mm_h: i32) -> i32 {
    const TARGET_MM: f64 = 50.0;
    const MIN_PX: i32 = 150;
    let by_physical = if physical_mm_h > 0 && mode_h_px > 0 {
        Some((TARGET_MM * mode_h_px as f64 / physical_mm_h as f64).round() as i32)
    } else {
        None
    };
    let px = by_physical.unwrap_or((area_h as f64 * 0.30).round() as i32);
    px.clamp(MIN_PX, (area_h as f64 * 0.6).round() as i32).max(1)
}

/// One placed grid cell, paired with the index of the item that fills it.
pub struct Cell {
    pub index: usize,
    pub rect: Rectangle<i32, Physical>,
}

/// Tunables for [`layout`] (all physical pixels).
pub struct GridParams {
    pub gap: i32,
    pub cell_height: i32,
    pub margin: i32,
}

/// Lay `aspects` (each = width/height) into a centered grid inside `area`.
/// Returns one [`Cell`] per input aspect (input order) plus total content height
/// for scroll clamping. Vertically centered when it fits, else top-aligned (so a
/// caller's scroll works from 0). Wide cells are clamped to the available width.
pub fn layout(
    area: Rectangle<i32, Physical>,
    aspects: &[f64],
    params: GridParams,
) -> (Vec<Cell>, i32) {
    if aspects.is_empty() {
        return (Vec::new(), 0);
    }

    let inner_w = (area.size.w - 2 * params.margin).max(1);
    let inner_h = (area.size.h - 2 * params.margin).max(1);
    let h = params.cell_height.clamp(1, inner_h);

    // Cell widths (same height, varying width), clamped to the available width.
    let widths: Vec<i32> = aspects
        .iter()
        .map(|a| ((h as f64 * a.max(0.05)).round() as i32).clamp(1, inner_w))
        .collect();

    // Greedily pack indices into rows that fit `inner_w` (with gaps).
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut row: Vec<usize> = Vec::new();
    let mut row_w = 0;
    for (i, &w) in widths.iter().enumerate() {
        let extra = if row.is_empty() { w } else { params.gap + w };
        if !row.is_empty() && row_w + extra > inner_w {
            rows.push(std::mem::take(&mut row));
            row_w = 0;
        }
        row_w += if row.is_empty() { w } else { params.gap + w };
        row.push(i);
    }
    if !row.is_empty() {
        rows.push(row);
    }

    // Center the block of rows vertically.
    let rows_n = rows.len() as i32;
    let block_h = rows_n * h + (rows_n - 1).max(0) * params.gap;
    let mut y = area.loc.y + params.margin + ((inner_h - block_h) / 2).max(0);

    let mut cells = Vec::with_capacity(aspects.len());
    for row in &rows {
        let total: i32 = row.iter().map(|&i| widths[i]).sum::<i32>()
            + (row.len() as i32 - 1).max(0) * params.gap;
        let mut x = area.loc.x + params.margin + ((inner_w - total) / 2).max(0);
        for &i in row {
            cells.push(Cell {
                index: i,
                rect: Rectangle::new(Point::from((x, y)), Size::from((widths[i], h))),
            });
            x += widths[i] + params.gap;
        }
        y += h + params.gap;
    }
    (cells, block_h)
}
