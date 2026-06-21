//! Native Vulkan parallax background: the SPIR-V shader modules (WGSL → SPIR-V
//! via naga at build time), push-constant packing, and the `NativeShaderPass`
//! builder handed through the dispatch seam.
//!
//! This is the Vulkan counterpart of the GLES pixel-shader parallax
//! (`draw.program` / `spacev3.frag`). It lives in the background layer — not in
//! the Vulkan kernel — so the kernel keeps only the generic fullscreen-shader
//! machinery (`FullscreenPass`) and learns nothing parallax-specific. The
//! shaders flow down to the renderer as bytes through the renderer-agnostic
//! dispatch seam.

pub mod vulkan;
