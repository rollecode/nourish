use crate::layershell::layershell;
use crate::{buffers, hooks};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Id, Kind, RenderElement};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{ImportAll, ImportDma, ImportMem, Renderer, Texture};
use smithay::desktop::space::SpaceRenderElements;
use smithay::desktop::{Window, layer_map_for_output};
use smithay::utils::{Logical, Physical, Point, Scale, Size};
use smithay::wayland::seat::WaylandFocus;
use compositor_orchestration_draw_dispatch_frame::SceneDispatch;
use compositor_orchestration_draw_scene_element::element::{PreImported, SceneElement};
use compositor_orchestration_core_state_base::Loop;
use compositor_orchestration_core_state_base::state::CoordinateTrait;
use compositor_orchestration_draw_node_base::node::{DrawNode, Plan};
use compositor_support_system_world_frame_base::base as layer;

pub struct Scene<R: Renderer> {
    pub Element: Vec<SceneElement<R>>,
    pub visible_window: Vec<Window>,
}

/// The GLES-built elements carried from the `prepare()` phase into the
/// renderer-agnostic `scene()`. iced UI, bevy 3D, and the parallax background
/// each render their content into GLES resources every frame (via the GLES
/// renderer), so they're produced here and handed to `scene()` as plain values
/// — `scene()` only wraps them into `SceneElement`s and draws them through the
/// `SceneDispatch` seam (real on GLES, blank on Vulkan).
pub struct PreparedGles {
    pub surfaces: Vec<compositor_monitor_compositor_iced_base::IcedRenderElement>,
    pub surfaces_screen: Vec<compositor_monitor_compositor_iced_base::IcedRenderElement>,
    /// Capture-dim layer elements, composited between windows and the
    /// world/background layers.
    pub surfaces_dim: Vec<compositor_monitor_compositor_iced_base::IcedRenderElement>,
    pub background_two: Option<compositor_background_two_draw_element::element::ParallaxBackground>,
    pub background_three: Vec<compositor_support_bevy_core_compositor_base::BevyRenderElement>,
}

/// GLES-only preparation phase: runs the per-frame hooks and builds the iced /
/// bevy / parallax GLES resources. Always runs on the (winit/native) GLES
/// renderer — separate from the renderer-agnostic `scene()` so the scene can be
/// composed by any renderer (e.g. Vulkan).
pub fn prepare(
    state: &mut Loop,
    renderer: &mut GlesRenderer,
    size: Size<i32, Physical>,
) -> PreparedGles {
    // Temporary method of calling binding hooks for external renderers lazily.
    hooks::hooks(state, renderer, size);

    let (surfaces, surfaces_screen, surfaces_dim) =
        compositor_y5_surface_draw_scene::scene::scene(state, renderer, size);
    // Parallax background is now a system (`TwoSystem`): it ticks its animation
    // in `update()` and emits a renderer-agnostic node from `draw()`. Run the
    // active world's draw pass, then bridge its `Background2D` node back into the
    // GLES prepare slot. The continuous-redraw cadence the parallax needs is a
    // driver concern, applied here while a node is live.
    let mut background_two = {
        let mut frame = compositor_support_system_world_frame_base::base::FramePlan::new();
        let mut platform = unsafe {
            compositor_orchestration_draw_platform_base::platform::Platform::new(
                Some(renderer),
                &mut state.inner.space_state_mut().state,
            )
        };
        let kernel = &state.inner.kernel;
        state.inner.worlds.active_mut().draw(kernel, &mut frame, Some(&mut platform));
        drop(platform);
        frame.sorted().into_iter().find_map(|(_, node)| {
            node.downcast::<compositor_background_two_draw_element::element::ParallaxBackground>()
                .ok()
                .map(|b| *b)
        })
    };
    // An overlay world (e.g. lock) carries no parallax of its own, so the active
    // world's draw yields none — fall back to the focused session world's
    // (spawn_target). This keeps the real desktop background in the frame that the
    // lock capture blits, so the lock screenshot shows windows AND background.
    if background_two.is_none() {
        background_two = compositor_background_two_draw_scene::scene::scene(state);
    }
    if background_two.is_some() {
        state.schedule_redraw_post_vblank();
    }
    let background_three =
        compositor_background_three_draw_scene::scene::scene(state, renderer, size);

    // Lock setup is GLES construction (builds iced/bevy lock surfaces).
    if let Some(setlock) = state.inner.__set_lock.clone() {
        let sleep = setlock.sleep;
        state.inner.__set_lock = None;
        compositor_y5_lock_interface_base::interface::lock(state, renderer, size, sleep);
    }

    // World-selection screen: a two-frame capture-on-leave. Frame A (the request
    // is present) arms a framebuffer capture of the still-active origin world;
    // this frame's render fills it. Frame B (request cleared, capture armed)
    // snapshots it into the origin's thumbnail and switches to the picker. Both
    // run here, while the origin world is the one being drawn.
    if state.inner.__set_picker.take().is_some() {
        compositor_y5_picker_interface_capture::capture::arm(state, renderer, size);
    } else {
        compositor_y5_picker_interface_capture::capture::finish_arm_and_open(state, renderer, size);
    }

    PreparedGles {
        surfaces,
        surfaces_screen,
        surfaces_dim,
        background_two,
        background_three,
    }
}

pub fn scene<R>(
    state: &mut Loop,
    renderer: &mut R,
    size: Size<i32, Physical>,
    prepared: PreparedGles,
) -> Scene<R>
where
    R: Renderer + ImportAll + ImportDma + ImportMem + SceneDispatch,
    R::TextureId: Texture + Clone + Send + 'static,
{
    // Handles incoming buffers such as the RPC buffer.
    buffers::update(state, renderer, size);

    // Invoke state machine updates, such as the navigator state machine

    // Assemble the frame as a layered plan of owned draw nodes, then lower it
    // to the renderer's element list at the single backend seam. Layering is
    // explicit (BACKGROUND..POINTER); contributors no longer rely on push order.
    let mut plan: Plan<R> = Plan::new();

    let pointer = compositor_orchestration_seat_pointer_draw::scene::element(state, renderer, size);
    plan.extend(layer::POINTER, pointer.into_iter().map(DrawNode::Pointer));

    let layer_shell = layershell(state, size);
    plan.extend(layer::LAYERSHELL, layer_shell.into_iter().map(DrawNode::Surface));

    plan.extend(layer::ICED_SCREEN, prepared.surfaces_screen.into_iter().map(DrawNode::Iced));

    // CONTENT band: windows + world iced INTERLEAVED by the DrawOrder authority
    // ("everything interleaves"). The canvas scene returns the ordered mix; we
    // map each to its draw node at one layer. (World iced is no longer a
    // separate below-windows band — it shares this band, ordered by DrawOrder.)
    let (content, canvas_window) =
        compositor_y5_canvas_draw_scene::scene::scene(state, renderer, size);
    for item in content {
        match item {
            compositor_y5_canvas_draw_scene::scene::ContentItem::Canvas(e) => {
                plan.push(layer::CANVAS, DrawNode::Canvas(e))
            }
            compositor_y5_canvas_draw_scene::scene::ContentItem::Iced(e) => {
                plan.push(layer::CANVAS, DrawNode::Iced(e))
            }
        }
    }

    // Capture-dim backdrop: below the content band, above background.
    plan.extend(layer::CAPTURE_DIM, prepared.surfaces_dim.into_iter().map(DrawNode::Iced));
    let _ = prepared.surfaces; // world iced now drawn per-id in the content band
    plan.extend(layer::WORLD_3D, prepared.background_three.into_iter().map(DrawNode::Background3D));
    if let Some(bg) = prepared.background_two {
        plan.push(layer::BACKGROUND, DrawNode::Background2D(bg));
    }

    let elements = plan.lower(renderer);

    // Apply fractional scaling.
    let updated_fractional = compositor_support_smithay_state_fractional_dispatch::hook(
        &mut state.state.fractional,
        &state.inner.space_state().state,
        state.inner.camera().transform.zoom,
    );
    if let Some(_fractional) = updated_fractional {
        if let Some(_registry) = &mut state.inner.surface_mut().registry {
            // registry.set_instance_scale(fractional as f32);
        }
    }

    // Auto-tick the resize debounce per frame: emit any due (throttled) `send_configure` even
    // without a pointer motion, so a mid-drag pause re-renders the client to the paused size on
    // its own instead of waiting for the next motion / release. Collect first (borrows the space),
    // then send.
    let resize_due: Vec<(Window, Size<i32, Logical>)> = state
        .inner.space_state()
        .state
        .elements()
        .filter_map(|w| {
            compositor_y5_camera_transform_translate::slot::resize_due(w).map(|s| (w.clone(), s))
        })
        .collect();
    for (window, size) in resize_due {
        if let Some(toplevel) = window.toplevel() {
            toplevel.with_pending_state(|s| s.size = Some(size));
            toplevel.send_configure();
        }
    }

    // Return the resulting scene
    Scene {
        Element: elements,
        visible_window: canvas_window,
    }
}
