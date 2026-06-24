//! The selection toolbar (align / distribute / stack / scale-to-fit) as an
//! in-process iced surface, reconciled each frame against the live selection.
//!
//! Two placements, chosen by [`SELECTION_OVERLAY_PLACEMENT`]:
//! - `ScreenBottomCenter`: a fixed screen-space bar at the bottom-center.
//! - `WorldAtCursor`: a world-space bar centered just below the cursor and
//!   drawn above all windows. Being world-space, it scales with camera zoom;
//!   its content fills the surface (logical size == dmabuf size).
//!
//! This module owns the surface LIFECYCLE (create/destroy/count) because that
//! needs the `GlesRenderer` (dmabuf alloc), which only the render path has.
//! Re-anchoring on selection change is event-driven and lives in a system —
//! see `compositor_y5_select_overlay_system`.
//!
//! The surface exists only while the selection is non-empty (create/destroy),
//! so when nothing is selected it captures no pointer/keyboard and draws no
//! cursor — there is simply no surface.

use std::sync::Once;

use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Physical, Point, Rectangle, Size};

use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_core_state_base::state::CoordinateTrait;
use compositor_orchestration_driver_selection_base::base::{
    BAR_H, BAR_W, Placement, SCREEN_BOTTOM_MARGIN, SELECTION_OVERLAY, SELECTION_OVERLAY_MUT,
    SELECTION_OVERLAY_PLACEMENT, SELECTION_REANCHOR_MUT, world_loc_under_cursor,
};
use compositor_orchestration_draw_layer_base::base::Layer;
use compositor_support_world_order_track_base::base::DrawLayer;
use compositor_monitor_compositor_iced_base::{HandleId, IcedHandle, IcedSpace};
use compositor_monitor_selection_scene_base::selection::SelectionAction;
use compositor_monitor_selection_scene_base::ui::{Message, Overlay};
use compositor_y5_surface_draw_handle::handle::load;
use compositor_y5_surface_protocol_base::protocol::{
    SelectionForward, SurfaceMessage, SurfaceMessageType,
};
use compositor_remote_message_client_base::bind::selection;

/// Per-frame reconciler (runs from the scene `hooks`, which has the renderer).
/// Creates the toolbar when the selection becomes non-empty (positioned under
/// the cursor for the world placement), destroys it when it empties, and pushes
/// count changes. Reposition-on-change is handled by the system.
pub fn per_frame(state: &mut Loop, renderer: &mut GlesRenderer, size: Size<i32, Physical>) {
    let count = state.inner.select().Selection.len() as i32;
    let handle = state.inner.kernel.get(&SELECTION_OVERLAY).handle;

    match (handle, count) {
        (None, n) if n > 0 => create(state, renderer, size, n),
        (Some(id), 0) => destroy(state, id),
        (Some(id), n) => update(state, id, n),
        (None, _) => {}
    }

    // Apply a re-anchor requested by the selection-change event (system-driven).
    reanchor_if_pending(state);
}

/// Consume the one-shot `SELECTION_REANCHOR` flag (raised by the overlay system
/// on a selection-change event) and move the world toolbar under the live
/// cursor. Done here, not in the system, because the cursor comes from the seat.
fn reanchor_if_pending(state: &mut Loop) {
    if SELECTION_OVERLAY_PLACEMENT != Placement::WorldAtCursor {
        return;
    }
    let target = state.inner.worlds.spawn_target();
    let pending = state
        .inner
        .worlds
        .get_mut(target)
        .storage_mut()
        .try_get_mut(&SELECTION_REANCHOR_MUT)
        .map(|flag| std::mem::replace(flag, false))
        .unwrap_or(false);
    if !pending {
        return;
    }
    let Some(id) = state.inner.kernel.get(&SELECTION_OVERLAY).handle else {
        return;
    };
    let loc = world_loc(state);
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        reg.set_location_by_id(id, loc);
    }
}

fn create(state: &mut Loop, renderer: &mut GlesRenderer, size: Size<i32, Physical>, count: i32) {
    ensure_font();

    let (loc, sz, space) = placement(state, size);
    let handle = load(
        state,
        renderer,
        Overlay::with_count(count),
        Rectangle::new(loc, sz),
        space,
        Layer::SCENE.bits(),
    );

    // World-space: lift above every window (load registered it at CONTENT).
    if let IcedSpace::World = space {
        state
            .inner
            .register_drawable(uuid::Uuid::from_u128(handle.id.0 as u128), DrawLayer::OVERLAY);
    }

    install_handler(state, handle);

    let untyped = handle.untyped();
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        // Give the toolbar iced keyboard focus so Shift/Alt modifiers reach it.
        reg.set_keyboard_focus(Some(untyped));
    }
    // ...and drop the wayland keyboard focus. The keyboard handler routes to iced
    // ONLY when no wayland client is focused (`wayland_handle` short-circuits
    // otherwise), so a focused window would otherwise swallow the modifiers. This
    // mirrors the old layer-shell overlay's on-demand keyboard grab.
    grab_keyboard_to_overlay(state);

    let st = state.inner.kernel.get_mut(&SELECTION_OVERLAY_MUT);
    st.handle = Some(untyped);
    st.count = count;
}

/// Clear the wayland keyboard focus so keys/modifiers flow to the iced registry
/// (and thus the focused toolbar) instead of a focused window.
fn grab_keyboard_to_overlay(state: &mut Loop) {
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    if let Some(keyboard) = state.state.seat.seat.get_keyboard() {
        keyboard.set_focus(
            &mut state.state,
            Option::<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>::None,
            serial,
        );
    }
}

fn update(state: &mut Loop, id: HandleId, count: i32) {
    if state.inner.kernel.get(&SELECTION_OVERLAY).count != count {
        if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
            reg.dispatch_message(IcedHandle::<Overlay>::from_id(id), Message::SelectNotify(count));
        }
        state.inner.kernel.get_mut(&SELECTION_OVERLAY_MUT).count = count;
    }
}

fn destroy(state: &mut Loop, id: HandleId) {
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        reg.destroy_by_id(id); // also clears keyboard focus / pointer / grab
    }
    let st = state.inner.kernel.get_mut(&SELECTION_OVERLAY_MUT);
    st.handle = None;
    st.count = 0;
}

/// Drained from the surface message pump (`SurfaceMessageType::Selection`):
/// execute a toolbar action in-process against the canvas.
pub fn handle(state: &mut Loop, _renderer: &mut GlesRenderer, forward: SelectionForward) {
    match forward {
        SelectionForward::Execute(actions, alt) => {
            let request = selection::Layout { action: to_actions(&actions, alt) };
            compositor_remote_client_handle_selection::layout(request, state);
        }
        SelectionForward::ScaleToFit(opt) => {
            compositor_remote_client_handle_aspect::fit_aspect(
                selection::FitAspect {
                    perceived: opt.perceived,
                    max: opt.max,
                    horizontal: opt.horizontal,
                    vertical: opt.vertical,
                },
                state,
            );
        }
    }
}

// --- placement ------------------------------------------------------------

fn placement(
    state: &Loop,
    size: Size<i32, Physical>,
) -> (Point<i32, Physical>, Size<i32, Physical>, IcedSpace) {
    match SELECTION_OVERLAY_PLACEMENT {
        Placement::ScreenBottomCenter => {
            let x = ((size.w - BAR_W) / 2).max(0);
            let y = (size.h - BAR_H - SCREEN_BOTTOM_MARGIN).max(0);
            (Point::from((x, y)), Size::from((BAR_W, BAR_H)), IcedSpace::Screen)
        }
        Placement::WorldAtCursor => {
            // Fixed native size: world-space items scale with zoom on their own,
            // and a 1:1 logical/dmabuf size lets the content fill the surface.
            (world_loc(state), Size::from((BAR_W, BAR_H)), IcedSpace::World)
        }
    }
}

/// World-physical top-left so the (BAR_W×BAR_H) toolbar is centered horizontally
/// on the cursor and sits just below it. World iced stores location in
/// logical×scale units; the cursor (`pointer.motion`) is logical.
fn world_loc(state: &Loop) -> Point<i32, Physical> {
    // The live cursor in y5-world logical space comes from the seat (the y5
    // PointerState token is not the live value). World iced location is
    // logical × scale.
    let cursor = state
        .state
        .seat
        .seat
        .get_pointer()
        .map(|p| p.current_location())
        .unwrap_or_default();
    world_loc_under_cursor((cursor.x, cursor.y), state.size_context().scale)
}

// --- font / registry plumbing ---------------------------------------------

/// Register the Material Symbols icon font into iced's global font DB once.
fn ensure_font() {
    static ONCE: Once = Once::new();
    ONCE.call_once(compositor_monitor_selection_font_base::font::load);
}

fn install_handler(state: &mut Loop, handle: IcedHandle<Overlay>) {
    let tx = state.inner.surface_mut().surface_message_buffer_channel.0.clone();
    if let Some(reg) = state.inner.surface_mut().registry.as_mut() {
        if let Some(inst) = reg.instance_mut(handle) {
            inst.runtime_mut().set_message_handler(move |m: &Message| {
                let forward = match m {
                    Message::ExecuteSelection(actions, alt) => {
                        Some(SelectionForward::Execute(actions.clone(), *alt))
                    }
                    Message::ExecuteScaleToFit(opt) => Some(SelectionForward::ScaleToFit(*opt)),
                    _ => None,
                };
                if let Some(forward) = forward {
                    let _ = tx.send(SurfaceMessage {
                        message: SurfaceMessageType::Selection(forward),
                    });
                }
            });
        }
    }
}

// --- UI action -> proto layout (mirrors the former gRPC client path) ------

fn to_actions(actions: &[SelectionAction], alternative: bool) -> Vec<selection::Action> {
    actions
        .iter()
        .filter_map(|a| to_action(a, alternative))
        .map(|action| selection::Action { action: Some(action) })
        .collect()
}

fn to_action(a: &SelectionAction, alternative: bool) -> Option<selection::action::Action> {
    let modifier = selection::align::Modifier { stretch: alternative };
    let action = match a {
        SelectionAction::ScaleToFit(_) => return None, // routed via ExecuteScaleToFit
        SelectionAction::AlignTop => selection::action::Action::Align(selection::Align {
            action: Some(selection::align::Action::Top(modifier)),
        }),
        SelectionAction::AlignBottom => selection::action::Action::Align(selection::Align {
            action: Some(selection::align::Action::Bottom(modifier)),
        }),
        SelectionAction::AlignLeft => selection::action::Action::Align(selection::Align {
            action: Some(selection::align::Action::Left(modifier)),
        }),
        SelectionAction::AlignVerticalCenter => {
            selection::action::Action::Align(selection::Align {
                action: Some(selection::align::Action::CenterVertical(modifier)),
            })
        }
        SelectionAction::AlignHorizontalCenter => {
            selection::action::Action::Align(selection::Align {
                action: Some(selection::align::Action::CenterHorizontal(modifier)),
            })
        }
        SelectionAction::AlignRight => selection::action::Action::Align(selection::Align {
            action: Some(selection::align::Action::Right(modifier)),
        }),
        SelectionAction::DistributeHorizontal => {
            selection::action::Action::Distribute(selection::Distribute {
                action: Some(selection::distribute::Action::Horizontal(
                    selection::distribute::Modifier { start: alternative },
                )),
            })
        }
        SelectionAction::DistributeVertical => {
            selection::action::Action::Distribute(selection::Distribute {
                action: Some(selection::distribute::Action::Vertical(
                    selection::distribute::Modifier { start: alternative },
                )),
            })
        }
        SelectionAction::StackHorizontal => selection::action::Action::Stack(selection::Stack {
            action: Some(selection::stack::Action::Horizontal(true)),
        }),
        SelectionAction::StackVertical => selection::action::Action::Stack(selection::Stack {
            action: Some(selection::stack::Action::Vertical(true)),
        }),
    };
    Some(action)
}
