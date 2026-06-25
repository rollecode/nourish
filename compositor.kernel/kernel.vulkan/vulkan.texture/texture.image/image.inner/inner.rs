use ash::vk;
use smithay::backend::allocator::Fourcc;
use std::fmt;

/// GPU resources behind `VulkanTexture`'s `Arc`; destroyed when the last
/// handle drops. dmabuf-imported memory is owned by us (fd dup'd at import).
pub struct TextureInner {
    pub device: ash::Device,
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    /// Extra per-plane allocations for a DISJOINT multi-plane (Intel CCS)
    /// import; empty for the common single-memory case. Freed with `memory`.
    pub extra_memory: Vec<vk::DeviceMemory>,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub fourcc: Option<Fourcc>,
    pub width: u32,
    pub height: u32,
    pub owns_memory: bool,
}

impl fmt::Debug for TextureInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextureInner")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("fourcc", &self.fourcc)
            .finish()
    }
}

impl Drop for TextureInner {
    fn drop(&mut self) {
        unsafe {
            if self.view != vk::ImageView::null() {
                self.device.destroy_image_view(self.view, None);
            }
            if self.image != vk::Image::null() {
                self.device.destroy_image(self.image, None);
            }
            if self.owns_memory && self.memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.memory, None);
                for m in &self.extra_memory {
                    self.device.free_memory(*m, None);
                }
            }
        }
    }
}
