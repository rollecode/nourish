//! Overview rim interface: toggle the overlay and reconcile the menu-bar
//! surface.
//!
//! `toggle`/`request_close` flip `Overview::visible` synchronously (so input
//! gating and the scene react this frame), then defer the menu-bar surface
//! create/destroy to the surface pump via `OverviewSurfaceMessage::Reconcile` —
//! the pump holds the GLES renderer `interface.surface::open` needs.

use smithay::backend::renderer::gles::GlesRenderer;
use compositor_orchestration_core_state_base::Loop;
use compositor_y5_overview_interface_surface::surface;
use compositor_y5_overview_state_base::base::{OverviewSurfaceMessage, Tab};
use compositor_y5_surface_protocol_base::protocol::{SurfaceMessage, SurfaceMessageType};

/// Super+Tab: flip the overlay on/off. Defers the menu-bar surface op.
pub fn toggle(state: &mut Loop) {
    let now = !state.inner.overview().visible;
    state.inner.overview_mut().visible = now;
    if now {
        // Always open on the Layout tab, freshly scrolled.
        let ov = state.inner.overview_mut();
        ov.tab = Tab::Layout;
        ov.scroll = 0.0;
    }
    defer_reconcile(state);
}

/// Escape: close the overlay if it is open (no-op otherwise).
pub fn request_close(state: &mut Loop) -> bool {
    if !state.inner.overview().visible {
        return false;
    }
    state.inner.overview_mut().visible = false;
    defer_reconcile(state);
    true
}

fn defer_reconcile(state: &mut Loop) {
    let _ = state
        .inner
        .surface_mut()
        .surface_message_buffer_channel
        .0
        .send(SurfaceMessage {
            message: SurfaceMessageType::Overview(OverviewSurfaceMessage::Reconcile),
        });
}

/// Surface-pump entry: apply a deferred overview action (has the GLES renderer).
pub fn handle(state: &mut Loop, renderer: &mut GlesRenderer, message: OverviewSurfaceMessage) {
    match message {
        OverviewSurfaceMessage::Reconcile => {
            if state.inner.overview().visible {
                surface::open(state, renderer);
            } else {
                surface::close(state);
            }
        }
        OverviewSurfaceMessage::SetTab(tab) => {
            state.inner.overview_mut().tab = tab;
        }
    }
}
