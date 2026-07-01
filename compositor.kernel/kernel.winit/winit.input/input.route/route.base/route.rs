//! Winit event dispatch -> compositor lifecycle. The Focus(false) modifier
//! hack now routes through the shared `compositor_kernel_graphic_seat_modifier_clear` entry
//! (the same problem exists on native TTY switch).

use compositor_kernel_winit_scene_compose_base::compose::WinitRenderContext;
use smithay::backend::winit::WinitEvent;
use compositor_orchestration_core_state_base::Loop;

pub fn route(event: &WinitEvent, state: &mut Loop, context: &mut WinitRenderContext) {
    match event {
        WinitEvent::Resized { size, scale_factor } => {
            info!("winit: resized to {size:?} (scale {scale_factor})");
            compositor_orchestration_draw_state_lifecycle::lifecycle::resize(
                context.output.clone(),
                *size,
                Some(smithay::output::Scale::Fractional(*scale_factor)),
            );
            state.schedule_redraw();
        }
        WinitEvent::Input(input_event) => {
            // Per-event logging omitted: input is a high-frequency path.
            compositor_orchestration_draw_state_lifecycle::lifecycle::input(state, input_event);
            // (Lock engage is drained in the control-plane ping source — the lock
            // keybinding calls `ping_control()` — not here, so it never depends on
            // input arriving and doesn't poll per frame.)
        }
        WinitEvent::Focus(focused) => {
            info!("winit: focus={focused}");
            if !focused {
                info!("winit: focus lost — clearing held modifiers");
                compositor_kernel_graphic_seat_modifier_clear::clear::clear_held_modifiers(state);
            }
        }
        WinitEvent::Redraw => {
            compositor_kernel_winit_scene_compose_base::compose::draw(state, context);
        }
        WinitEvent::CloseRequested => {
            info!("winit: close requested — stopping compositor");
            compositor_orchestration_draw_state_lifecycle::lifecycle::stop(state);
        }
        _ => (),
    }
}
