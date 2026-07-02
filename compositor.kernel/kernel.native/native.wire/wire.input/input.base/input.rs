//! Input wiring: the libinput source -> compositor lifecycle + redraw
//! scheduling. (Ex wire.rs `start()` libinput closure.)

use compositor_kernel_input_loop_libinput_base::libinput::LibinputSource;
use compositor_kernel_native_context_render_base::render::NativeRenderContext;
use smithay::backend::input::InputEvent;
use smithay::reexports::input::DeviceCapability;
use smithay::reexports::calloop::EventLoop;
use std::cell::RefCell;
use std::rc::Rc;
use compositor_orchestration_core_state_base::state::StatusSession;
use compositor_orchestration_core_state_base::Loop;

pub fn register(
    event_loop: &mut EventLoop<Loop>,
    libinput_source: LibinputSource,
    ctx_rc: Rc<RefCell<NativeRenderContext>>,
) {
    event_loop
        .handle()
        .insert_source(libinput_source, move |event, _, state| {
            if let StatusSession::Paused = state.inner.status_session {
                return;
            }
            // Track physical keyboards so `led_state_changed` can drive their
            // LEDs, and seed each newly added keyboard with the current LED
            // state (e.g. the NumLock-on-by-default set at seat creation).
            match &event {
                InputEvent::DeviceAdded { device }
                    if device.has_capability(DeviceCapability::Keyboard) =>
                {
                    let mut device = device.clone();
                    if let Some(keyboard) = state.state.seat.seat.get_keyboard() {
                        device.led_update(keyboard.led_state().into());
                    }
                    state.state.seat.keyboards.push(device);
                }
                InputEvent::DeviceRemoved { device } => {
                    state.state.seat.keyboards.retain(|d| d != device);
                }
                _ => {}
            }
            // Any input event potentially changes what should be on screen
            // (cursor position, focus, key feedback). Request a redraw.
            // When DARK (no output), only keyboard events run the full pipeline so
            // the always-on fixed shortcuts (VT switch, volume, media) still work;
            // pointer/touch/gesture processing is dropped (no display to interact
            // with — avoids warping the cursor / refocusing against a dead space).
            let dark = *state.inner.kernel.get(
                &compositor_orchestration_driver_lid_base::base::DISPLAY_OFF,
            ) || ctx_rc.borrow().pipe().drm_output.is_none();
            if !dark || matches!(event, InputEvent::Keyboard { .. }) {
                compositor_orchestration_draw_state_lifecycle::lifecycle::input(state, &event);
            }
            // A lid switch (or any input that drove the lid policy) may have
            // queued a display request; perform it now — here so it runs even
            // when the render loop is gated (DPMS-off) or about to be.
            compositor_kernel_native_context_display_apply::apply::drain(state, &ctx_rc);
            // Live output-mode change requests from the settings window (apply /
            // confirm / revert), drained here for the same reason as the lid.
            compositor_kernel_native_context_display_mode::mode::drain(state, &ctx_rc);
            // Live active-output (preferred-monitor) switch requests from the
            // settings window (apply / confirm / revert), same gate as the mode.
            compositor_kernel_native_context_display_switch::switch::drain(state, &ctx_rc);
            // Lock engage: the keybinding set `Status::Locked` synchronously + flagged
            // this; run the renderer-free engage off-frame on a one-shot idle (the
            // keyboard crates can't call `lock.interface` — it depends back on them).
            if state.inner.lock_engage {
                state.inner.lock_engage = false;
                state.loop_handle.insert_idle(|state| {
                    compositor_y5_lock_interface_base::interface::lock_logical(state);
                });
            }
            state.schedule_redraw();
        })
        .unwrap();
}
