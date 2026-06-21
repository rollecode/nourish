use compositor_support_system_buffer_token_base::y5_buffer;
use compositor_support_system_channel_token_base::y5_channel;
use compositor_support_system_input_event_base::base::{InputEvent, InputFlow};
use compositor_support_system_input_layer_base::base as input_layer;
use compositor_support_system_storage_slot_base::base::Storage;
use compositor_support_system_trait_system_base::base::{BufferCx, System, SystemCx, WorldBuilder};
use compositor_support_system_world_frame_base::base::FrameTick;
use compositor_y5_camera_state_base::state::{Camera, CAMERA, CAMERA_MUT};
use compositor_y5_canvas_input_state::state::{ActiveOption, CanvasGrab};
use compositor_y5_canvas_system_base::base::CANVAS;
use compositor_y5_group_state_base::state::{GroupVisibility, GROUP};
use compositor_y5_navigator_state_base::state::{NavRequest, State, NAVIGATOR};
use compositor_y5_surface_interface_core::hit::surface_under_filtered_cx;
use compositor_y5_window_interface_record::window::LoopWindow;
use smithay::desktop::Window;
use smithay::utils::{Logical, Point};
use std::any::Any;

/// Zoom guard (see the legacy camera.draw interface for the rationale: the
/// scene can wedge at extreme zoom until the damage interaction is root-caused).
const MIN_ZOOM: f64 = 0.02;
const MAX_ZOOM: f64 = 50.0;

#[derive(Clone, Copy, Debug)]
pub struct CameraMoved {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct CameraZoomed {
    pub zoom: f64,
}
y5_channel!(pub CAMERA_MOVED, CAMERA_MOVED_TX: CameraMoved);
y5_channel!(pub CAMERA_ZOOMED, CAMERA_ZOOMED_TX: CameraZoomed);

enum CamCmd {
    SetPosition(f64, f64),
    SetZoom(f64),
    /// Canvas PAN step (Hand grab / `position_updating`), migrated from the rim
    /// `canvas.input/input.motion`. Carries the CURRENT physical screen cursor;
    /// the handler reads `position_previous` (the previous screen cursor),
    /// computes the zoom-scaled delta, advances the camera position by it, and
    /// stores the new screen cursor in `position_previous`. Done whole in the
    /// buffer so the screen accumulator (`position_previous`) is NOT clobbered by
    /// `SetPosition` (which writes it to the camera's logical position). The bool
    /// gates the actual camera move (the rim advanced position_previous on every
    /// motion event but only panned when `position_updating`).
    Pan(f64, f64, bool),
}
y5_buffer!(CAM_BUF: CamCmd);

/// Owns the camera slot. Pulls the navigator's eased output each tick, applies it
/// via its buffer, announces CAMERA_MOVED / CAMERA_ZOOMED, and — because it OWNS
/// the camera — handles scroll-zoom INPUT directly: a Pass-1 system can only
/// mutate its own slot synchronously, so cursor-anchored zoom (which must
/// accumulate across rapid scroll events) lives here, not in a separate canvas
/// input system that could only defer via a channel.
#[derive(Default)]
pub struct CameraSystem;

impl System for CameraSystem {
    fn name(&self) -> &'static str {
        "camera"
    }

    fn register(&mut self, builder: &mut WorldBuilder) {
        builder.storage.insert(&CAMERA, Camera::default());
        builder.input(input_layer::WORLD);
    }

    fn input(&mut self, cx: &mut SystemCx, event: &InputEvent) -> InputFlow {
        // Canvas PAN: when a pan is in progress (Hand grab / position_updating),
        // advance the camera by the zoom-scaled screen-cursor delta. The pan path
        // is non-intercepting in the rim (returned `false`), so we Pass — the rim
        // still runs native_motion. Synchronous via our own CAM_BUF so rapid
        // motion accumulates (each step reads the just-written position_previous).
        if let InputEvent::PointerMotion { screen_x, screen_y, .. } = event {
            // The rim updated position_previous on EVERY motion event (so the
            // first pan step has a correct delta), but only moved the camera when
            // position_updating. `do_pan` carries that gate; position_previous is
            // always advanced to the current screen cursor.
            let do_pan = cx.storage.get(&CANVAS).position_updating;
            // A manual pan is direct user intent: drop any active navigator travel
            // so the easing doesn't fight (and immediately overwrite) the drag.
            if do_pan {
                cancel_travel(cx);
            }
            cx.write(&CAM_BUF, CamCmd::Pan(*screen_x, *screen_y, do_pan));
            return InputFlow::Pass;
        }

        let InputEvent::PointerAxis { vertical, x, y, .. } = event else { return InputFlow::Pass };
        let cursor = Point::<f64, Logical>::from((*x, *y));

        // Over a visible window (and not a scene-group passthrough ice)? Then it's
        // a window scroll — Pass so the rim's native_axis routes it to the client.
        let over_window = surface_under_filtered_cx(cx.storage, cursor, &|hit| {
            if let Some(window) = hit.window() {
                return window_visible(cx.storage, window);
            }
            if let Some(layer) = hit.iced_layer()
                && (layer & compositor_orchestration_draw_layer_base::base::Layer::SCENE_SURFACE_GROUP.bits()) != 0
            {
                return false;
            }
            true
        })
        .is_some();
        let hand = matches!(cx.storage.get(&CANVAS).Grab, CanvasGrab::Active(ActiveOption::Hand));
        if over_window && !hand {
            return InputFlow::Pass;
        }

        // Canvas zoom, cursor-anchored. Synchronous via our own CAM_BUF (flushed
        // right after this input()), so the next scroll event reads the updated zoom.
        if *vertical != 0.0 {
            // A manual zoom is direct user intent: drop any active navigator travel
            // so the easing doesn't fight (and immediately overwrite) the gesture.
            cancel_travel(cx);
            let (old_zoom, cam_position) = {
                let camera = cx.storage.get(&CAMERA);
                (*camera.transform.zoom(), camera.transform.position())
            };
            let base_step = 0.05 * old_zoom;
            let distance_from_normal = (old_zoom - 1.0).abs();
            let normal_dampener = 0.4 + (0.6 * (distance_from_normal / (distance_from_normal + 1.0)));
            let adjusted_step = base_step * normal_dampener * vertical.abs().min(1.0);
            let new_zoom = if *vertical < 0.0 { old_zoom + adjusted_step } else { (old_zoom - adjusted_step).max(0.01) };

            // Position uses the CLAMPED zoom (the buffer clamps the same way), so the
            // cursor stays pinned to the same logical point.
            let actual_new_zoom = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            let zoom_ratio = old_zoom / actual_new_zoom;
            let new_x = cursor.x - (cursor.x - cam_position.x) * zoom_ratio;
            let new_y = cursor.y - (cursor.y - cam_position.y) * zoom_ratio;

            cx.write(&CAM_BUF, CamCmd::SetZoom(new_zoom));
            cx.write(&CAM_BUF, CamCmd::SetPosition(new_x, new_y));
        }
        InputFlow::Consume
    }

    fn update(&mut self, cx: &mut SystemCx, _tick: &FrameTick) {
        let Some(output) = cx.storage.get(&NAVIGATOR).output else { return };
        if let Some((x, y)) = output.position {
            cx.write(&CAM_BUF, CamCmd::SetPosition(x, y));
        }
        if let Some(zoom) = output.zoom {
            cx.write(&CAM_BUF, CamCmd::SetZoom(zoom));
        }
    }

    fn buffer(&mut self, cx: &mut BufferCx, message: Box<dyn Any>) {
        let camera = cx.storage.get_mut(&CAMERA_MUT);
        match *message.downcast::<CamCmd>().expect("camera buffer type") {
            CamCmd::SetPosition(x, y) => {
                // NOTE: do NOT touch `position_previous` here. It is the canvas-PAN
                // accumulator (the previous physical screen cursor, written by
                // `CamCmd::Pan` + wire.rs on pointer init) and is read ONLY by the
                // Pan arm. SetPosition is driven by zoom + navigator easing; writing
                // the LOGICAL camera position into it clobbered the screen
                // accumulator, so the next pan computed `screen - world_position`
                // and flung the camera to an unknown place (and windows out of view).
                camera.transform.position = Point::from((x, y));
                cx.channels.send(&CAMERA_MOVED_TX, CameraMoved { x, y });
            }
            CamCmd::SetZoom(zoom) => {
                let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
                camera.transform.zoom = zoom;
                cx.channels.send(&CAMERA_ZOOMED_TX, CameraZoomed { zoom });
            }
            CamCmd::Pan(screen_x, screen_y, do_pan) => {
                // Zoom-scaled screen-cursor delta (rim: dx = (screen - prev)/zoom).
                let zoom = *camera.transform.zoom();
                let prev = camera.position_previous;
                let dx = (screen_x - prev.x) / zoom;
                let dy = (screen_y - prev.y) / zoom;
                // Advance position_previous for the next event (rim does this every
                // motion event, whether panning or not).
                camera.position_previous = Point::from((screen_x, screen_y));
                if do_pan {
                    // Rim: cam_position -= (dx, dy). Emit CAMERA_MOVED so the
                    // background parallax follows (the rim's camera_draw::position
                    // updated the background pan inline).
                    let pos = camera.transform.position();
                    let new_x = pos.x - dx;
                    let new_y = pos.y - dy;
                    camera.transform.position = Point::from((new_x, new_y));
                    cx.channels.send(&CAMERA_MOVED_TX, CameraMoved { x: new_x, y: new_y });
                }
            }
        }
    }
}

/// Cancel an in-progress navigator travel when the user pans/zooms by hand.
/// Announced on the focused world's channel (the navigator owns the slot); only
/// fires while a `Travel` is set, so it's a no-op once cancelled. `Machine::set`
/// ignores the request while locked, so this never disturbs the lock state.
fn cancel_travel(cx: &mut SystemCx) {
    if matches!(cx.storage.get(&NAVIGATOR).state(), State::Travel(_)) {
        compositor_y5_navigator_state_base::state::request(cx.channels, NavRequest::Set(State::Idle));
    }
}

/// Window visibility via group state (mirrors `DrawWindow::visible`, reading
/// `cx.storage` instead of `&Loop`): a window in a hidden group is not visible.
fn window_visible(storage: &Storage, window: &Window) -> bool {
    let Some(window_uuid) = window.uuid() else { return true };
    let group_state = storage.get(&GROUP);
    let Some(group_uuid) = group_state.window.get(&window_uuid) else { return true };
    for group in &group_state.group {
        if &group.id != group_uuid.as_ref() {
            continue;
        }
        return matches!(group.Visibility, GroupVisibility::Visible(_));
    }
    false
}
