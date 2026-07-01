//! `ParallaxBackground`: the infinite-canvas background render element.
use compositor_background_two_draw_motion::Motion;
use compositor_orchestration_draw_dispatch_frame::SceneDispatch;
use smithay::backend::renderer::RendererSuper;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement};
use smithay::backend::renderer::gles::{GlesPixelProgram, GlesRenderer};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Physical, Point, Rectangle, Scale, Size, Transform};
use std::time::Instant;

#[derive(Clone)]
pub struct ParallaxBackground {
    id: Id,
    commit: CommitCounter,
    /// `None` when compositing from dmabufs (Vulkan; the native shader runs).
    program: Option<GlesPixelProgram>,
    start_time: Instant,
    pub lock_time: Option<Instant>,
    pub output_size: (f32, f32),
    /// Render-rect top-left (physical px); a pane's origin per-pane, else `(0,0)`.
    pub offset: (i32, i32),
    pub pan: (f32, f32), // state passed from your main loop
    pub zoom: f32,
    motion: Motion,
}

impl ParallaxBackground {
    pub fn new(renderer: &mut GlesRenderer, output_size: (f32, f32)) -> Self {
        // Vulkan runs the native shader; the GLES pixel program is never sampled.
        let program = if compositor_developer_stats_registry_base::base::compositor_prefers_dmabuf() {
            None
        } else {
            Some(compositor_background_two_draw_program::compile_program(renderer))
        };
        Self {
            output_size,
            offset: (0, 0),
            id: Id::new(),
            commit: CommitCounter::default(),
            program,
            lock_time: None,
            start_time: Instant::now(),
            pan: (0.0, 0.0), zoom: 1.0, motion: Motion::new(),
        }
    }

    /// Call right before draw to splice the previous pan.
    pub fn update(&mut self) {
        self.motion.tick(self.pan, self.lock_time.is_some());
        self.commit.increment();
    }
    /// Rebind a clone to a viewport pane (render rect + pane camera + distinct id).
    pub fn bind_pane(&mut self, offset: (i32, i32), size: (f32, f32), pan: (f32, f32), zoom: f32, id: Id) {
        self.offset = offset; self.output_size = size; self.pan = pan; self.zoom = zoom; self.id = id;
    }
}

impl Element for ParallaxBackground {
    fn id(&self) -> &Id { &self.id }
    fn current_commit(&self) -> CommitCounter { self.commit }
    fn src(&self) -> Rectangle<f64, Buffer> { Rectangle::from_loc_and_size((0.0, 0.0), (1.0, 1.0)) }
    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        Rectangle::from_loc_and_size(self.offset, (self.output_size.0 as i32, self.output_size.1 as i32))
    }
    fn location(&self, _scale: Scale<f64>) -> Point<i32, Physical> { Point::from(self.offset) }
    fn transform(&self) -> Transform { Transform::Normal }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        if commit != Some(self.commit) {
            vec![self.geometry(scale)].into_iter().collect()
        } else {
            DamageSet::default()
        }
    }
    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> { OpaqueRegions::default() }
    fn alpha(&self) -> f32 { 1.0 }
    fn kind(&self) -> Kind { Kind::Unspecified }
    fn is_framebuffer_effect(&self) -> bool { false }
}
impl<R: SceneDispatch> RenderElement<R> for ParallaxBackground {
    fn draw(
        &self,
        frame: &mut <R as RendererSuper>::Frame<'_, '_>,
        _src: Rectangle<f64, Buffer>, dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>], _cache: Option<&UserDataMap>,
    ) -> Result<(), <R as RendererSuper>::Error> {
        let time = self.start_time.elapsed().as_secs_f32();
        let (uniforms, vk) = compositor_background_two_draw_motion::uniforms(
            time, self.motion.lock_amount, self.pan, self.motion.flow_offset,
            self.motion.velocity, self.zoom, (dst.size.w as f32, dst.size.h as f32));
        let pass = compositor_background_two_draw_vulkan::vulkan::ParallaxPass::new(&vk);
        R::draw_pixel_program(
            frame,
            self.program.as_ref(),
            Rectangle::from_loc_and_size((0.0, 0.0), (dst.size.w as f64, dst.size.h as f64)),
            dst, Size::from((dst.size.w, dst.size.h)), damage, 1.0, &uniforms, pass.pass(),
        )
    }
}
