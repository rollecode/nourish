//! Gaussian-ish blur of the frozen desktop, baked into a fresh full-res dmabuf.
//!
//! Chained LINEAR downsample (full → ½ → ¼ → ⅛) — each exact-halving blit is a
//! proper 2×2 box average, so the chain is a real low-pass (no aliasing) — then a
//! single LINEAR upscale back to full. All filtering is explicit `Linear`, so the
//! result is smooth regardless of the draw-time sampler. Cross-renderer safe: the
//! output is a dmabuf the scene imports + draws 1:1. Returns `None` on any
//! allocation/blit failure (the caller falls back to the sharp snapshot).

use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{Bind, Blit, TextureFilter};
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::utils::{Physical, Point, Rectangle, Size};
use compositor_support_bevy_core_alloc_base::{allocate_dmabuf, AllocatedDmabuf};
use compositor_y5_graphic_capture_registry::SnapshotHandle;

fn rect(w: i32, h: i32) -> Rectangle<i32, Physical> {
    Rectangle::new(Point::from((0, 0)), Size::from((w.max(1), h.max(1))))
}

fn blit(gles: &mut GlesRenderer, from: &mut Dmabuf, to: &mut Dmabuf, src: Rectangle<i32, Physical>, dst: Rectangle<i32, Physical>) -> Option<()> {
    let s = gles.bind(from).ok()?;
    let mut t = gles.bind(to).ok()?;
    let sync = gles.blit(&s, &mut t, src, dst, TextureFilter::Linear).ok()?;
    // Block until the GPU has consumed this stage — the intermediate dmabufs are
    // dropped when `blur` returns, so each must be done being read first.
    let _ = sync.wait();
    Some(())
}

/// Produce a full-res blurred copy of `snap`'s desktop frame.
pub fn blur(gles: &mut GlesRenderer, node: &str, snap: &SnapshotHandle) -> Option<AllocatedDmabuf> {
    let full = snap.size();
    let (w, h) = (full.w.max(1), full.h.max(1));
    let half = allocate_dmabuf(node, (w / 2).max(1) as u32, (h / 2).max(1) as u32).ok()?;
    let quarter = allocate_dmabuf(node, (w / 4).max(1) as u32, (h / 4).max(1) as u32).ok()?;
    let eighth = allocate_dmabuf(node, (w / 8).max(1) as u32, (h / 8).max(1) as u32).ok()?;
    let out = allocate_dmabuf(node, w as u32, h as u32).ok()?;

    let mut src = snap.dmabuf().clone();
    let mut d2 = half.dmabuf.clone();
    let mut d4 = quarter.dmabuf.clone();
    let mut d8 = eighth.dmabuf.clone();
    let mut dout = out.dmabuf.clone();

    blit(gles, &mut src, &mut d2, rect(w, h), rect(w / 2, h / 2))?;
    blit(gles, &mut d2, &mut d4, rect(w / 2, h / 2), rect(w / 4, h / 4))?;
    blit(gles, &mut d4, &mut d8, rect(w / 4, h / 4), rect(w / 8, h / 8))?;
    blit(gles, &mut d8, &mut dout, rect(w / 8, h / 8), rect(w, h))?;

    Some(out)
}
