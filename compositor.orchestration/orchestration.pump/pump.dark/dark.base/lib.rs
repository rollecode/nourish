//! Control-plane pump for when the compositor is DARK (no output → the per-frame
//! draw pipeline halts). The kernel arms a loop-handle timer that calls [`dark::pump`]
//! so the IMPORTANT renderer-free control-plane still advances: deliver queued
//! channel events to the active world's systems (e.g. the output-presence event →
//! capture stop) and drain the RPC remote-control buffer. Animations/rendering are
//! intentionally NOT pumped here.

pub mod dark;
