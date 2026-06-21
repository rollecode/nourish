use smithay::backend::renderer::gles::{GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexture, Uniform};
use smithay::backend::renderer::{Renderer, RendererSuper};
use smithay::utils::{Buffer as BufferCoord, Physical, Rectangle, Size};

pub use compositor_orchestration_draw_dispatch_uniforms::uniforms::{
    NativeShaderPass, ParallaxUniforms, ShaderVariant,
};
use compositor_orchestration_draw_dispatch_uniforms::uniforms as gles;

/// The per-renderer dispatch seam keeping GLES-welded scene elements (iced UI,
/// bevy 3D, the parallax background) renderer-agnostic. GLES implements it for
/// real; other renderers (Vulkan) implement a blank draw until their native
/// path lands. Taken as a param-on-renderer trait to dodge the GAT-HRTB
/// limitation (rust#100013).
pub trait SceneDispatch: Renderer {
    /// Whether scene composition should feed this renderer the iced/bevy/parallax
    /// output as a dmabuf (native texture) instead of the GLES-welded seam.
    fn prefers_dmabuf() -> bool {
        false
    }

    /// Draw a pre-rendered GLES texture into `frame`. Blank for renderers that
    /// cannot sample a `GlesTexture`.
    fn draw_prerendered_texture(
        frame: &mut <Self as RendererSuper>::Frame<'_, '_>,
        texture: &GlesTexture,
        src: Rectangle<f64, BufferCoord>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        alpha: f32,
    ) -> Result<(), <Self as RendererSuper>::Error>;

    /// Run a GLES pixel-shader program over a region. Blank for renderers
    /// without a pixel-shader path; `program` is `None` for those. `pass`
    /// carries a renderer-native fullscreen-shader draw (SPIR-V + push bytes)
    /// for renderers that composite the background with their own pipeline.
    #[allow(clippy::too_many_arguments)]
    fn draw_pixel_program(
        frame: &mut <Self as RendererSuper>::Frame<'_, '_>,
        program: Option<&GlesPixelProgram>,
        src: Rectangle<f64, BufferCoord>,
        dst: Rectangle<i32, Physical>,
        size: Size<i32, BufferCoord>,
        damage: &[Rectangle<i32, Physical>],
        alpha: f32,
        uniforms: &[Uniform<'_>],
        pass: NativeShaderPass<'_>,
    ) -> Result<(), <Self as RendererSuper>::Error>;
}

impl SceneDispatch for GlesRenderer {
    fn draw_prerendered_texture(
        frame: &mut GlesFrame<'_, '_>,
        texture: &GlesTexture,
        src: Rectangle<f64, BufferCoord>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        alpha: f32,
    ) -> Result<(), <Self as RendererSuper>::Error> {
        gles::draw_prerendered_texture(frame, texture, src, dst, damage, alpha)
    }

    fn draw_pixel_program(
        frame: &mut GlesFrame<'_, '_>,
        program: Option<&GlesPixelProgram>,
        src: Rectangle<f64, BufferCoord>,
        dst: Rectangle<i32, Physical>,
        size: Size<i32, BufferCoord>,
        damage: &[Rectangle<i32, Physical>],
        alpha: f32,
        uniforms: &[Uniform<'_>],
        _pass: NativeShaderPass<'_>,
    ) -> Result<(), <Self as RendererSuper>::Error> {
        gles::draw_pixel_program(frame, program, src, dst, size, damage, alpha, uniforms)
    }
}
