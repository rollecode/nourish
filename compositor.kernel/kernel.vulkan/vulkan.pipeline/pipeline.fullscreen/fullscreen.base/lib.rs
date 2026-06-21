//! Generic native fullscreen-shader pass for the Vulkan renderer.
//!
//! A parallax-agnostic `VkPipeline` wrapper: hand it a SPIR-V module (with a
//! vertex + fragment entry point) and a push-constant size, and it draws a
//! fullscreen triangle running that shader — premultiplied-alpha-over blend,
//! dynamic viewport/scissor, no vertex buffers or descriptor sets (positions
//! come from `SV_VertexID`/`@builtin(vertex_index)`).
//!
//! This is "the generalization of how to use a shader". The specific shaders
//! and push-constant layouts (e.g. the parallax background) live in the scene
//! layer that produces the draw; the kernel only knows how to run them.

pub mod fullscreen;
