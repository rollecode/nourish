//! Pointer MOTION transforms, migrated from the rim
//! (`canvas.input/input.motion/motion.rs`) into `CanvasSystem::input`.
//!
//! The owner-system split (document/SMITHAY_DECOUPLING.md P3.4h): the canvas PAN
//! lives on `CameraSystem` (it owns the camera, must accumulate). THIS handles
//! the grab transforms CanvasSystem owns:
//!   - MOVE / SCALE of windows (reform reimplemented Loop-free via `cx.platform`)
//!   - MOVE / SCALE of placeholders (slot + iced-registry geometry via the
//!     placeholder system channel)
//!   - SELECTBOX cursor tracking (canvas grab state, via this system's buffer)
//! plus the wayland `pointer.motion`/`frame` + constraint-abandon via `cx.seat`.
//!
//! Consume is ALL-OR-NOTHING: returns `InputFlow::Consume` exactly when the rim
//! handler returned `true` (an active non-Hand grab), `Pass` otherwise (the rim
//! `native_motion` then runs).

use compositor_support_system_input_event_base::base::InputFlow;
use compositor_support_system_trait_system_base::base::SystemCx;
use compositor_support_smithay_dispatch_state_base::state::Dispatch;
use compositor_y5_canvas_input_state::state::{
    ActiveOption, ActiveTransformCandidate, CanvasGrab,
};
use compositor_y5_camera_transform_translate::slot;
use smithay::desktop::Window;
use smithay::input::pointer::MotionEvent;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Size, SERIAL_COUNTER};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashSet;
use uuid::Uuid;
use compositor_y5_group_state_base::state::GROUP;
use compositor_y5_group_protocol_base::protocol::{GroupBufferMessage, GroupBufferMessageBBOX};
use compositor_y5_surface_system_base::base::SURFACE;
use compositor_y5_surface_protocol_base::protocol::{SurfaceMessage, SurfaceMessageType};
use compositor_y5_window_interface_record::window::LoopWindow;
use crate::base::{CanvasCmd, CANVAS, CANVAS_BUF};

/// A pending window/placeholder geometry change (mirrors the rim `TransformUpdate`).
#[derive(Clone, Copy)]
struct Update {
    position: Option<Point<i32, Logical>>,
    size: Option<Size<i32, Logical>>,
}

/// Pointer MOTION. `x`/`y` are the world point (== the rim's `current_location()`
/// inside the transform arms and the `pointer.motion` location); `screen` is
/// unused here (the PAN delta lives on CameraSystem).
pub(crate) fn motion(cx: &mut SystemCx, x: f64, y: f64, _screen_x: f64, _screen_y: f64) -> InputFlow {
    // The transform arms used the wayland pointer's current location (world
    // space), which equals the normalized world point the rim built for this
    // event; the start_cursor anchors are in the same space.
    let cursor = Point::<f64, Logical>::from((x, y));

    // Only an ACTIVE non-Hand grab is a transform. Hand / no-grab -> Pass (the
    // rim returned `false`; PAN is handled by CameraSystem, native_motion by the
    // rim).
    let active = matches!(
        cx.storage.get(&CANVAS).Grab,
        CanvasGrab::Active(ActiveOption::Moving { .. })
            | CanvasGrab::Active(ActiveOption::Scaling { .. })
            | CanvasGrab::Active(ActiveOption::SelectBox { .. })
    );
    if !active {
        return InputFlow::Pass;
    }

    let mut window_updates: Vec<(Window, Update)> = vec![];
    let mut placeholder_updates: Vec<(Uuid, Update)> = vec![];
    let mut select_box_cursor: Option<Point<f64, Logical>> = None;

    match &cx.storage.get(&CANVAS).Grab {
        CanvasGrab::Active(ActiveOption::Moving { candidates, start_cursor, .. }) => {
            let total_dx = cursor.x - start_cursor.x;
            let total_dy = cursor.y - start_cursor.y;
            match candidates {
                ActiveTransformCandidate::Window(list) => {
                    for (window, start_geo) in list {
                        let mut loc = start_geo.loc;
                        loc.x = (start_geo.loc.x as f64 + total_dx).floor() as i32;
                        loc.y = (start_geo.loc.y as f64 + total_dy).floor() as i32;
                        window_updates.push((window.clone(), Update { position: Some(loc), size: None }));
                    }
                }
                ActiveTransformCandidate::Placeholder(uuid, start_geo) => {
                    let mut loc = start_geo.loc;
                    loc.x = (start_geo.loc.x as f64 + total_dx).floor() as i32;
                    loc.y = (start_geo.loc.y as f64 + total_dy).floor() as i32;
                    placeholder_updates.push((*uuid, Update { position: Some(loc), size: None }));
                }
            }
        }
        CanvasGrab::Active(ActiveOption::Scaling { candidates, start_cursor, Anchor, .. }) => {
            let dx = cursor.x.round() - start_cursor.x.round();
            let dy = cursor.y.round() - start_cursor.y.round();
            let horizontal = Anchor.Horizontal;
            let vertical = Anchor.Vertical;
            match candidates {
                ActiveTransformCandidate::Window(list) => {
                    for (window, start_geo) in list {
                        let mut new_geo = *start_geo;
                        // --- X-AXIS ---
                        if horizontal {
                            let new_w = (start_geo.size.w as f64 + dx).round() as i32;
                            new_geo.size.w = new_w.max(300);
                        } else {
                            let start_left = start_geo.loc.x as f64;
                            let start_right = start_geo.loc.x as f64 + start_geo.size.w as f64;
                            let new_left_f = start_left + dx;
                            let min_left = start_right - 300.0;
                            let new_left = new_left_f.min(min_left).round() as i32;
                            let right_i = start_right.round() as i32;
                            new_geo.loc.x = new_left;
                            new_geo.size.w = right_i - new_left;
                        }
                        // --- Y-AXIS ---
                        if vertical {
                            let new_h = (start_geo.size.h as f64 + dy).round() as i32;
                            new_geo.size.h = new_h.max(300);
                        } else {
                            let start_top = start_geo.loc.y as f64;
                            let start_bottom = start_geo.loc.y as f64 + start_geo.size.h as f64;
                            let new_top_f = start_top + dy;
                            let min_top = start_bottom - 300.0;
                            let new_top = new_top_f.min(min_top).round() as i32;
                            let bottom_i = start_bottom.round() as i32;
                            new_geo.loc.y = new_top;
                            new_geo.size.h = bottom_i - new_top;
                        }
                        let mut update = Update { position: None, size: Some(new_geo.size) };
                        if !horizontal || !vertical {
                            update.position = Some(new_geo.loc);
                        }
                        window_updates.push((window.clone(), update));
                    }
                }
                ActiveTransformCandidate::Placeholder(uuid, start_geo) => {
                    let mut new_geo = *start_geo;
                    // --- X-AXIS ---
                    if horizontal {
                        let new_w = (start_geo.size.w as f64 + dx).floor() as i32;
                        new_geo.size.w = new_w.max(1);
                    } else {
                        let max_negative_dx = -(start_geo.size.w as f64) + 1.0;
                        let bounded_dx = dx.max(max_negative_dx);
                        new_geo.size.w = (start_geo.size.w as f64 - bounded_dx).floor() as i32;
                        new_geo.loc.x = (start_geo.loc.x as f64 + bounded_dx).floor() as i32;
                    }
                    // --- Y-AXIS ---
                    if vertical {
                        let new_h = (start_geo.size.h as f64 + dy).floor() as i32;
                        new_geo.size.h = new_h.max(1);
                    } else {
                        let max_negative_dy = -(start_geo.size.h as f64) + 1.0;
                        let bounded_dy = dy.max(max_negative_dy);
                        new_geo.size.h = (start_geo.size.h as f64 - bounded_dy).floor() as i32;
                        new_geo.loc.y = (start_geo.loc.y as f64 + bounded_dy).floor() as i32;
                    }
                    if new_geo.size.w < 300 {
                        new_geo.size.w = 300;
                    }
                    if new_geo.size.h < 300 {
                        new_geo.size.h = 300;
                    }
                    let mut update = Update { position: None, size: Some(new_geo.size) };
                    if !horizontal || !vertical {
                        update.position = Some(new_geo.loc);
                    }
                    placeholder_updates.push((*uuid, update));
                }
            }
        }
        CanvasGrab::Active(ActiveOption::SelectBox { .. }) => {
            // Track the current cursor (the rim wrote screen_pos = current_location).
            select_box_cursor = Some(cursor);
        }
        _ => {}
    }

    // Groups whose member windows are moving: the group SURFACE is a SEPARATE
    // iced surface, positioned only by `handle_bbox` off a `Group(BBOX)` surface
    // message (the rim's `_reform` sent it via `invalidate_bbox`, which this
    // Loop-free reform dropped). Collect the affected groups BEFORE reforming, so
    // afterward we re-send it — else the member windows move but the group surface
    // + bbox stay put. (document/DECOUPLE_REMAIN.md / SMITHAY_DECOUPLING.md P3.4h.)
    let affected_groups: Vec<Uuid> = {
        let group_state = cx.storage.get(&GROUP);
        let mut seen = HashSet::new();
        window_updates
            .iter()
            .filter_map(|(w, _)| w.uuid())
            .filter_map(|u| group_state.window.get(&u).map(|g| **g))
            .filter(|g| seen.insert(*g))
            .collect()
    };

    // Apply window reforms (force = interactive drag) reading `cx.platform.space()`.
    // Each window owns a live `map` placeholder keyed by its UUID; keep its geometry
    // in sync with the drag (rim `_reform` did this via `placeholder.interface::set`)
    // so the tile spawns at the dragged-to geometry when the window later closes.
    for (window, update) in window_updates {
        if let Some(uuid) = window.uuid() {
            let position = update.position.map(|p| (p.x, p.y));
            let size = update.size.map(|s| (s.w, s.h));
            compositor_y5_placeholder_system_base::base::announce_placeholder_geometry(
                cx.channels, uuid, position, size,
            );
        }
        reform_force(cx, window, update);
    }

    // Re-position each affected group surface: `handle_bbox` recomputes the bbox
    // from the windows' NEW geometry on the next surface drain (a frame's lag,
    // consistent with the group's own BBOX invalidations).
    if !affected_groups.is_empty() {
        let tx = cx.storage.get(&SURFACE).surface_message_buffer_channel.0.clone();
        for group_id in affected_groups {
            let _ = tx.send(SurfaceMessage {
                message: SurfaceMessageType::Group(GroupBufferMessage::BBOX(
                    GroupBufferMessageBBOX { group_id },
                )),
            });
        }
    }

    // Apply placeholder geometry (slot + iced registry) via the placeholder
    // system channel, mirroring the rim `set_visible_geometry`.
    for (uuid, update) in placeholder_updates {
        let position = update.position.map(|p| (p.x, p.y));
        let size = update.size.map(|s| (s.w, s.h));
        compositor_y5_placeholder_system_base::base::announce_placeholder_geometry(
            cx.channels, uuid, position, size,
        );
    }

    // SelectBox cursor lives in CANVAS (our slot) -> own buffer.
    if let Some(c) = select_box_cursor {
        cx.write(&CANVAS_BUF, CanvasCmd::SetSelectBoxCursor(c));
    }

    // Forward the wayland pointer motion + abandon any active constraint (the rim
    // tail), via `cx.seat`.
    if let Some(dispatch) = cx.seat.as_deref_mut().and_then(|s| s.downcast_mut::<Dispatch>()) {
        if let Some(pointer) = dispatch.seat.seat.get_pointer() {
            let serial = SERIAL_COUNTER.next_serial();
            let time = now_msec();
            if pointer.current_focus().is_some() {
                dispatch.seat.abandon_active_constraint(&pointer);
            }
            pointer.motion(
                dispatch,
                None,
                &MotionEvent { location: cursor, serial, time },
            );
            pointer.frame(dispatch);
        }
    }

    InputFlow::Consume
}

/// Loop-free reimplementation of `window.lifecycle.interface::reform_force`
/// (force = true): the interactive resize/move path. The Loop-coupled crate
/// can't be called from a system (CYCLE via the orchestration focus accessors),
/// so the smithay + `slot` core is replicated here, reading the live `Space`
/// through `cx.platform`. The rim's `placeholder::set` side effect is reproduced by
/// the caller (announced per window UUID before this runs); the remaining ones
/// (group bbox invalidate, capture-region refresh) are NOT reproduced — they are
/// settled on release / the next frame; see the migration report.
fn reform_force(cx: &mut SystemCx, window: Window, update: Update) {
    let Some(platform) = cx
        .platform
        .as_deref_mut()
        .and_then(|p| p.downcast_mut::<compositor_orchestration_draw_platform_base::platform::Platform>())
    else {
        return;
    };

    if let Some(position) = update.position {
        platform.space().map_element(window.clone(), position, false);
    }

    if let Some(size) = update.size {
        let Some(toplevel) = window.toplevel() else { return };
        toplevel.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
            state.size = Some(size);
        });
        // The compositor's new decided size (render/input fit authority).
        slot::set_expected_size(&window, size);
        // Interactive drag: throttle the configure (one client commit per motion
        // stutters) and arm the stretch so the window follows between commits.
        // The final size + settle happen on release (`finish_resize`).
        if slot::note_resize(&window, size) {
            toplevel.send_configure();
        }
    }
}

fn now_msec() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(0)
}
