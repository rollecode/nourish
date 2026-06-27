//! Overview World-tab render: the picker globe, embedded.
//!
//! Reuses the picker world's own systems — `embed_open` sets up a picker session
//! without switching the active world, `scene.create` builds the sphere in the
//! picker world's bevy registry, and `render_all` draws it. A minimal local tick
//! advances the sphere's orientation (the picker's own tick is skipped because it
//! drains the shared surface channel and the parallax, which the overview owns).

use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Physical, Point, Size};
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_draw_layer_base::base::Layer;
use compositor_support_bevy_core_compositor_base::{BevyRenderElement, Transform};
use compositor_y5_picker_system_base::base::{PICKER_MUT, PICKER_WORLD};

/// Ensure the embedded picker session + sphere exist, advance the orientation,
/// and render the picker world's bevy registry. Returns the bevy elements.
pub fn prepare_world(
    state: &mut Loop,
    gles: &mut GlesRenderer,
    size: Size<i32, Physical>,
) -> Vec<BevyRenderElement> {
    compositor_y5_picker_interface_embed::embed::embed_open(state);

    // Build the sphere scene once (the picker world's own registry).
    let has_bevy = state
        .inner
        .worlds
        .get_mut(PICKER_WORLD)
        .storage_mut()
        .get_mut(&PICKER_MUT)
        .active
        .as_ref()
        .map(|a| a.bevy.is_some())
        .unwrap_or(false);
    if !has_bevy {
        compositor_y5_picker_scene_create::create::create(state, gles, size);
    }

    tick(state);

    let gpu = state.inner.environment.GPU.clone();
    let reg = state
        .inner
        .worlds
        .get_mut(PICKER_WORLD)
        .storage_mut()
        .try_get_mut(&compositor_background_three_system_base::base::BG_THREE_MUT)
        .and_then(|b| b.registry.as_mut());
    let Some(reg) = reg else { return Vec::new() };
    let transform = Transform { zoom: 1.0, position: Point::new(0.0, 0.0) };
    reg.render_all(&gpu, gles, transform, size.to_f64(), Layer::PICKER_SCENE.bits())
        .unwrap_or_default()
}

/// Minimal orientation advance (momentum / approach) + push to the bevy scene.
fn tick(state: &mut Loop) {
    use compositor_y5_picker_three_constant as c;
    use compositor_y5_picker_three_orient::orient;
    if let Some(a) = state
        .inner
        .worlds
        .get_mut(PICKER_WORLD)
        .storage_mut()
        .get_mut(&PICKER_MUT)
        .active
        .as_mut()
        && a.drag.is_none()
    {
        if orient::spinning(a.spin) {
            let (o, s) = orient::momentum(a.orientation, a.spin, c::SPIN_DECAY);
            (a.orientation, a.spin, a.target) = (o, s, o);
        } else {
            a.orientation = orient::approach(a.orientation, a.target, c::APPROACH_RATE);
        }
    }
    compositor_y5_picker_command_base::base::push_transform(state);
    state.schedule_redraw_post_vblank();
}
