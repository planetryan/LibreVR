use crate::session::{VulkanContext, SwapchainInfo};
use std::error::Error;

pub struct VrRenderer {
    // vulkan context
    pub vk: VulkanContext,
}

impl VrRenderer {
    // create a new renderer with an existing vulkan context
    pub fn new(vk: VulkanContext) -> Self {
        Self { vk }
    }
    // TODO:
    // upload an rgba8 frame into a swapchain image
    // width and height must match the swapchain image extents
    pub fn upload_frame_to_swapchain(
        &self,
        _swapchain: &SwapchainInfo,
        _image_index: usize,
        _rgba_pixels: &[u8],
        _width: u32,
        _height: u32,
    ) -> Result<(), Box<dyn Error>> {
        // TODO:
        // - create or reuse a host-visible staging buffer
        // - write rgba_pixels into it
        // - record command buffer to transition swapchain image to transfer dst
        // - copy buffer to image
        // - transition image to color attachment or shader read
        // - submit and wait (or use semaphores/fences)
        println!("renderer: upload frame {}x{} to swapchain image {}", _width, _height, _image_index);
        Ok(())
    }
}