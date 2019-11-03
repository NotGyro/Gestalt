//! Rendering pipeline types.

pub mod chunk_pipeline;
pub mod lines_pipeline;
pub mod skybox_pipeline;
pub use self::chunk_pipeline::ChunkRenderPipeline;
pub use self::lines_pipeline::LinesRenderPipeline;
pub use self::skybox_pipeline::SkyboxRenderPipeline;


use std::sync::Arc;

use cgmath::Matrix4;
use vulkano::command_buffer::AutoCommandBuffer;
use vulkano::device::Queue;
use vulkano::image::swapchain::SwapchainImage;
use vulkano::format::D32Sfloat;
use vulkano::image::attachment::AttachmentImage;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use winit::Window;

use util::Transform;
use registry::TextureRegistry;
use renderer::RenderQueue;


pub struct PipelineCbCreateInfo {
    pub image_num: usize,
    pub dimensions: [u32; 2],
    pub queue: Arc<Queue>,
    pub camera_transform: Transform,
    pub view_mat: Matrix4<f32>,
    pub proj_mat: Matrix4<f32>,
    pub tex_registry: Arc<TextureRegistry>
}


pub trait RenderPipelineAbstract {
    // Required methods

    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>;
    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync>;
    fn build_command_buffer(&self, info: PipelineCbCreateInfo, render_queue: &RenderQueue) -> AutoCommandBuffer;


    // Provided methods

    fn remove_framebuffers(&mut self) { *self.get_framebuffers_mut() = None; }

    fn recreate_framebuffers_if_none(&mut self, images: &Vec<Arc<SwapchainImage<Window>>>, depth_buffer: &Arc<AttachmentImage<D32Sfloat>>) {
        if self.get_framebuffers_mut().is_none() {
            let new_framebuffers = Some(images.iter().map(|image| {
                let arc: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(self.get_renderpass().clone())
                    .add(image.clone()).unwrap()
                    .add(depth_buffer.clone()).unwrap()
                    .build().unwrap());
                arc
            }).collect::<Vec<_>>());
            ::std::mem::replace(self.get_framebuffers_mut(), new_framebuffers);
        }
    }
}