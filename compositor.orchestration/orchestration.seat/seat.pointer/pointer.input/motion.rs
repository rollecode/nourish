use crate::constraint::apply_pointer_constraint;
use crate::native_motion;
use smithay::backend::input::{AbsolutePositionEvent, Event, InputBackend, PointerMotionEvent};
use smithay::utils::{Logical, Physical, Point};
use std::ops::Deref;
use compositor_y5_camera_transform_translate::transform::Transform;
use compositor_y5_camera_transform_translate::translate;
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_core_state_base::state::CoordinateTrait;

pub fn absolute<I: InputBackend>(
    event: &<I as InputBackend>::PointerMotionAbsoluteEvent,
    _loop: &mut Loop,
) {
    // Compute the physical screen position (`raw_pos`) and the normalized world
    // point BEFORE the bus so the motion systems receive precisely what the rim
    // computed (the pan delta uses the physical screen, transforms +
    // pointer.motion use the world point).
    let ctx = _loop.size_context();

    // position_transformed wants Size<_, Logical>. Pass the panel's
    // physical size wrapped as Logical — the values are physical even
    // if the marker says otherwise. We immediately re-tag the result
    // as Physical (which is what it really is).
    let physical_size_as_logical = smithay::utils::Size::<i32, Logical>::from((
        ctx.screen_size_physical.0.round() as i32,
        ctx.screen_size_physical.1.round() as i32,
    ));
    let raw_pos: Point<f64, Logical> = event.position_transformed(physical_size_as_logical);

    // The numbers are in physical units; re-tag.
    let position_screen = Point::<f64, Physical>::from((raw_pos.x, raw_pos.y));

    let t: Transform = (position_screen, ctx).into();
    let position_normalized = &t.into_storage_point_f64();

    {
        // World input bus first (Pass-1): CameraSystem handles the canvas PAN
        // (Hand / position_updating), CanvasSystem the MOVE/SCALE/SELECTBOX
        // transforms. `Pass` falls through to legacy `native_motion` routing.
        let ev = compositor_support_system_input_event_base::base::InputEvent::PointerMotion {
            x: position_normalized.x,
            y: position_normalized.y,
            screen_x: raw_pos.x,
            screen_y: raw_pos.y,
            delta_x: 0.0,
            delta_y: 0.0,
        };
        if compositor_orchestration_input_drive_base::drive::route(_loop, ev)
            == compositor_support_system_input_event_base::base::InputFlow::Consume
        {
            return;
        }
    }

    // let ctx = _loop.size_context();
    // let output = _loop.inner.space_state().state.outputs().next().unwrap();
    // let logical_geom = _loop.inner.space_state().state.output_geometry(output).unwrap();

    // let position_screen: Point<f64, Logical> = event.position_transformed(logical_geom.size);

    // let t: Transform = (position_screen, ctx).into();
    // let position_normalized = &t.into_storage_point_f64();

    // Extract geometry from compositor space
    // let compositor_output = _loop.inner.space_state().state.outputs().next().unwrap();
    // let compositor_output_geometry = _loop
    //     .state
    //     .space
    //     .state
    //     .output_geometry(compositor_output)
    //     .unwrap();

    // // Get the position of cursor on screen
    // let position_screen = event.position_transformed(compositor_output_geometry.size)
    //     + compositor_output_geometry.loc.to_f64();

    // let ctx = _loop.size_context();
    // let cursor_phys = Point::<f64, Physical>::from((position_screen.x, position_screen.y));

    // let t: Transform = (cursor_phys, ctx).into();

    // // Extract as raw y5-world, since smithay clients work in world coords.
    // let position_normalized = &t.into_storage_point_f64();

    // // Normalize the cursor position based on camera state
    // // THis is actually screen to world which is also space to world since they share coords.
    // let position_normalized = &translate::space_to_world(
    //     &_loop.inner.camera_mut().transform,
    //     compositor_output_geometry.size.to_f64(),
    //     position_screen,
    //     _loop.inner.space_state().default_scale()
    // );

    // Bus returned Pass (no canvas pan/transform consumed it) — route native
    // client pointer motion.
    native_motion::absolute::input_received_normalized::<I>(
        event,
        _loop,
        position_normalized,
        &raw_pos,
    );
}

pub fn relative<I: InputBackend>(
    event: &<I as InputBackend>::PointerMotionEvent,
    _loop: &mut Loop,
) {
    let ctx = _loop.size_context();

    let ctx = _loop.size_context();

    let dt = event.delta();
    let dt_unaccelerated = event.delta_unaccel();

    // Live cursor-speed multiplier (settings window). Read before the
    // `pointer_mut()` borrows below so the borrows stay disjoint. Applied ONLY to
    // the on-screen cursor accumulator — NOT to `dt`/`dt_unaccelerated`, which are
    // forwarded raw to clients via the relative-pointer protocol (games apply
    // their own acceleration and expect unscaled hardware deltas).
    let sensitivity = _loop.inner.preference.cursor_sensitivity;

    // Snapshot previous position in both spaces.
    let previous_phys = _loop.inner.pointer_mut().motion.clone();
    let previous_world: Point<f64, Logical> = {
        let pt = Point::<f64, Physical>::from((previous_phys.x, previous_phys.y));
        let t: Transform = (pt, ctx).into();
        t.into_storage_point_f64()
    };

    // Accumulate the physical delta, scaled by the live cursor sensitivity.
    _loop.inner.pointer_mut().motion.x += dt.x * sensitivity;
    _loop.inner.pointer_mut().motion.y += dt.y * sensitivity;

    // Candidate in world space.
    let candidate_world: Point<f64, Logical> = {
        let pt = Point::<f64, Physical>::from((
            _loop.inner.pointer_mut().motion.x,
            _loop.inner.pointer_mut().motion.y,
        ));
        let t: Transform = (pt, ctx).into();
        t.into_storage_point_f64()
    };

    // CHECK: Constraint is not applied for absolute events. Relevant for
    // tablets and winit testing; otherwise redundant.
    let (constrained_world, was_constrained) =
        apply_pointer_constraint(_loop, previous_world, candidate_world);

    // Reconcile final world position and update the physical accumulator
    // so future deltas accumulate from the right place.
    let final_world: Point<f64, Logical> = if was_constrained {
        // Reverse-project constrained world back to physical, write to accumulator.
        let final_phys: Point<f64, Physical> = {
            let t: Transform = (constrained_world, ctx).into();
            t.into()
        };
        _loop.inner.pointer_mut().motion.x = final_phys.x;
        _loop.inner.pointer_mut().motion.y = final_phys.y;
        constrained_world
    } else {
        // No constraint: clamp physical to panel, re-derive world.
        let (pw, ph) = ctx.screen_size_physical;
        _loop.inner.pointer_mut().motion.x = _loop.inner.pointer_mut().motion.x.clamp(0.0, pw);
        _loop.inner.pointer_mut().motion.y = _loop.inner.pointer_mut().motion.y.clamp(0.0, ph);

        let pt = Point::<f64, Physical>::from((
            _loop.inner.pointer_mut().motion.x,
            _loop.inner.pointer_mut().motion.y,
        ));
        let t: Transform = (pt, ctx).into();
        t.into_storage_point_f64()
    };

    let position_screen = _loop.inner.pointer_mut().motion;
    let position_normalized = final_world;

    // Apply motion only if this is equals false
    // !was_constrained || final_world != previous_world a
    
    let was_constrained_locked = !(!was_constrained || final_world != previous_world);

    // Previous code start(working pre motion relative and constraint impl.)

    // Accumulate cursor in physical pixels (libinput's natural unit).
    // let dt = event.delta();
    // let dt_unaccelerated = event.delta_unaccel();
    // let time = event.time_msec();

    // let previous_location = _loop.inner.pointer_mut().motion.clone();

    // _loop.inner.pointer_mut().motion.x += dt.x;
    // _loop.inner.pointer_mut().motion.y += dt.y;

    // // Clamp to physical panel bounds.
    // let (pw, ph) = ctx.screen_size_physical;

    // // CHECK: Constraint is not applied for absolute events. This is especially relevant for tablets and testing within winit. but otherwise redaundant.
    // let (constrained, was_constrained) =
    //     apply_pointer_constraint(_loop, previous_location, _loop.inner.pointer_mut().motion);

    // let new_location = if was_constrained {
    //     constrained
    // } else {
    //     Point::new(
    //         _loop.inner.pointer_mut().motion.x.clamp(0.0, pw),
    //         _loop.inner.pointer_mut().motion.y.clamp(0.0, ph),
    //     )
    // };

    // _loop.inner.pointer_mut().motion = new_location;

    // // Additionally clamp by constraint.

    // // Build a Transform from the physical cursor position. Transform
    // // reverse-projects through scale + camera, storing as y5-world.
    // let cursor_phys =
    //     Point::<f64, Physical>::from((_loop.inner.pointer_mut().motion.x, _loop.inner.pointer_mut().motion.y));

    // let t: Transform = (cursor_phys, ctx).into();

    // let position_screen = _loop.inner.pointer_mut().motion; // < -- NOTE: This variable is being used for calculating deltas for panning
    // // Extract as raw y5-world, since smithay clients work in world coords.
    // let position_normalized = t.into_storage_point_f64(); // < -- NOTE: This variable is currently the variable sent to pointer motion

    // Previous code end //

    // 1. Extract geometry from compositor space (Exactly like your absolute arm)
    // let compositor_output = _loop.inner.space_state().state.outputs().next().unwrap();
    // let compositor_output_geometry = _loop
    //     .state
    //     .space
    //     .state
    //     .output_geometry(compositor_output)
    //     .unwrap();

    // let compositor_output_geometry_physical = _loop.inner.space_state().default_physical_precise();
    // let compositor_output_geometry_logical = _loop.inner.space_state().default_logical();

    // let compositor_output_geometry = _loop.inner.space_state().state.outputs().next().unwrap();
    // let compositor_output_geometry_logical = _loop.inner.space_state().state.output_geometry(compositor_output_geometry).unwrap();

    // 2. Calculate the new screen position using the hardware delta
    // Note: You must add `pub pointer_location: smithay::utils::Point<f64, Logical>`
    // to your `Loop` struct to accumulate this movement!
    // let dt = event.delta();
    // let mut position_screen = _loop.inner.pointer_mut().motion + dt;

    // 3. Clamp to screen bounds so the cursor cannot leave the monitor
    // let min_x = compositor_output_geometry_logical.loc.x as f64;
    // let max_x = (compositor_output_geometry_logical.loc.x + compositor_output_geometry_logical.size.w) as f64;
    // let min_y = compositor_output_geometry_logical.loc.y as f64;
    // let max_y = (compositor_output_geometry_logical.loc.y + compositor_output_geometry_logical.size.h) as f64;

    // It is better that this works logically like now.
    // But, position_normalized performs space_to_world which is in turn:
    // 1. does space_to_world
    // position_screen.x = position_screen.x.clamp(min_x, max_x);
    // position_screen.y = position_screen.y.clamp(min_y, max_y);

    // Save the clamped position for the next frame's relative calculation
    // _loop.inner.pointer_mut().motion = position_screen;

    // 5. Normalize the cursor position based on camera state
    // let position_normalized = translate::space_to_world(
    //     &_loop.inner.camera_mut().transform,
    //     compositor_output_geometry_logical.size.to_f64(),
    //     position_screen,
    //     _loop.inner.space_state().default_scale()
    // );
    // 4. Notify canvas of input to handle camera movement

    // World input bus first (Pass-1), AFTER position_normalized + the
    // pointer-constraint reconciliation: the systems receive the post-constraint
    // world point (`position_normalized`) and the physical accumulator
    // (`position_screen`) — exactly what the rim's canvas motion handler used.
    {
        let ev = compositor_support_system_input_event_base::base::InputEvent::PointerMotion {
            x: position_normalized.x,
            y: position_normalized.y,
            screen_x: position_screen.x,
            screen_y: position_screen.y,
            delta_x: dt.x,
            delta_y: dt.y,
        };
        if compositor_orchestration_input_drive_base::drive::route(_loop, ev)
            == compositor_support_system_input_event_base::base::InputFlow::Consume
        {
            return;
        }
    }

    let position_normalized = position_normalized;
    // 6. Dispatch normalize event to your native pointer handler
    // (Using the equivalent handler for PointerMotionEvent as you mentioned)
    native_motion::relative::input_received_normalized::<I>(
        event,
        _loop,
        position_normalized,
        &position_screen,
        (dt, dt_unaccelerated),
        was_constrained_locked,
    );
}
