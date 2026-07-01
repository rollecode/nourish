use compositor_background_two_draw_element::element::ParallaxBackground;
use compositor_background_two_state_base::state::Two;
use compositor_support_system_buffer_token_base::y5_buffer;
use compositor_support_system_storage_token_base::base::{Token, TokenMut};
use compositor_support_system_trait_system_base::base::{BufferCx, System, SystemCx, WorldBuilder};
use compositor_support_system_world_frame_base::base::{self as layer, FramePlan, FrameTick};
use smithay::backend::renderer::gles::GlesRenderer;
use std::any::Any;

pub static BG_TWO: Token<Two> = Token::new();
/// TRANSITIONAL pub: lock/capture still mutate the instance directly.
pub static BG_TWO_MUT: TokenMut<Two> = TokenMut::new(&BG_TWO);

enum TwoCmd {
    SetInstance(ParallaxBackground),
    Tick,
    Pan(f32, f32),
    Zoom(f32),
}
y5_buffer!(TWO_BUF: TwoCmd);

/// The 2D parallax background system: `update()` (re)builds the GPU resource
/// via the platform hatch and ticks animation; `draw()` emits the node.
#[derive(Default)]
pub struct TwoSystem;

impl System for TwoSystem {
    fn name(&self) -> &'static str {
        "background.two"
    }

    fn register(&mut self, builder: &mut WorldBuilder) {
        builder.storage.insert(&BG_TWO, Two::new());
        builder.receive(&compositor_y5_camera_system_base::base::CAMERA_MOVED, Self::on_camera_moved);
        builder.receive(&compositor_y5_camera_system_base::base::CAMERA_ZOOMED, Self::on_camera_zoomed);
    }

    fn update(&mut self, cx: &mut SystemCx, _tick: &FrameTick) {
        // bevy lock-morph gate; absent BG_THREE (test worlds) = not locked.
        if cx.storage.try_get(&compositor_background_three_system_base::base::BG_THREE)
            .is_some_and(|b| b.example_lock_done) { return; }
        // Physical output size from the per-frame screen driver-data (set by the
        // frame driver before systems run) — no background-private size token.
        let size = cx.kernel.get(&compositor_orchestration_smithay_data_base::data::SCREEN).size;
        let size = (size.w as f32, size.h as f32);
        let state = cx.storage.get(&BG_TWO);
        let stale = state.instance.as_ref().is_some_and(|i| i.output_size != size);
        let rebuild = state.instance.is_none() || stale;
        // Resolve once: this world's override → preference default → built-in.
        // (Setting `instance = None` from the rim forces a rebuild on change.)
        let override_sel = state.background_shader.clone();
        let params = state.params.clone();
        if rebuild {
            if let Some(renderer) = cx
                .platform
                .as_deref_mut()
                .and_then(|p| p.downcast_mut::<compositor_orchestration_draw_platform_base::platform::Platform>())
                .and_then(|p| p.renderer())
            {
                let sel = override_sel.or_else(
                    compositor_developer_stats_registry_base::base::background_shader_default,
                );
                let instance = ParallaxBackground::new(renderer, size, sel.as_deref(), &params);
                cx.write(&TWO_BUF, TwoCmd::SetInstance(instance));
            }
            return;
        }
        // Advance the parallax animation (mutation -> buffer, honoring read-only update).
        cx.write(&TWO_BUF, TwoCmd::Tick);
    }

    fn draw(&mut self, cx: &mut SystemCx, plan: &mut FramePlan) {
        if cx.storage.try_get(&compositor_background_three_system_base::base::BG_THREE)
            .is_some_and(|b| b.example_lock_done) { return; }
        // Renderer-agnostic node; the frame driver bridges + lowers it.
        if let Some(instance) = &cx.storage.get(&BG_TWO).instance {
            plan.push(layer::BACKGROUND, Box::new(instance.clone()));
        }
    }

    fn buffer(&mut self, cx: &mut BufferCx, message: Box<dyn Any>) {
        let two = cx.storage.get_mut(&BG_TWO_MUT);
        match *message.downcast::<TwoCmd>().expect("two buffer type") {
            TwoCmd::SetInstance(instance) => {
                two.shader_error = instance.shader_error.clone();
                two.instance = Some(instance);
            }
            TwoCmd::Tick => { if let Some(i) = &mut two.instance { i.update(); } }
            TwoCmd::Pan(x, y) => { if let Some(i) = &mut two.instance { i.pan = (x, y); } }
            TwoCmd::Zoom(z) => { if let Some(i) = &mut two.instance { i.zoom = z; } }
        }
    }

    /// Persist this world's background selection + variable overrides into a
    /// single per-world file `<world>/world.background.json`, rehydrated into the
    /// `BG_TWO` slot at world build.
    fn persist(
        &self,
    ) -> &'static [&'static compositor_support_system_persist_entry_base::base::PersistEntry] {
        BACKGROUND_PERSISTS
    }
}

/// This world's persisted background: the shader override + its edited variable
/// values keyed by `@prop` name (robust to slot/order changes).
#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct BackgroundPersisted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shader: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    params: Vec<(String, f32)>,
}

/// Transforms the per-world `Two` slot to/from its persisted form (a single
/// value, so `Persist`/`y5_persist!` — not the collection `Document`).
struct BackgroundPersist;
impl compositor_support_system_persist_trait_base::base::Persist for BackgroundPersist {
    type Live = Two;
    type Persisted = BackgroundPersisted;
    const KEY: &'static str = "world.background";
    const CURRENT_VERSION: u32 = 1;
    fn to_persisted(live: &Two) -> BackgroundPersisted {
        BackgroundPersisted { shader: live.background_shader.clone(), params: live.params.clone() }
    }
    fn from_persisted(p: BackgroundPersisted) -> Two {
        let mut two = Two::new();
        two.background_shader = p.shader;
        two.params = p.params;
        two
    }
}
compositor_support_system_persist_trait_base::y5_persist!(
    BACKGROUND_PERSIST, BackgroundPersist, BG_TWO, BG_TWO_MUT
);
static BACKGROUND_PERSISTS: &[&compositor_support_system_persist_entry_base::base::PersistEntry] =
    &[&BACKGROUND_PERSIST];

impl TwoSystem {
    fn on_camera_moved(&mut self, cx: &mut SystemCx, event: &compositor_y5_camera_system_base::base::CameraMoved) {
        cx.write(&TWO_BUF, TwoCmd::Pan(event.x as f32, event.y as f32));
    }

    fn on_camera_zoomed(&mut self, cx: &mut SystemCx, event: &compositor_y5_camera_system_base::base::CameraZoomed) {
        cx.write(&TWO_BUF, TwoCmd::Zoom(event.zoom as f32));
    }
}
