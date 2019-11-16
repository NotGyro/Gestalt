//! Rendering pipeline types.

pub mod pbr_pipeline;
pub mod lines_pipeline;
pub mod text_pipeline;
pub mod occlusion_pipeline;
pub use self::pbr_pipeline::PBRRenderPipeline;
pub use self::lines_pipeline::LinesRenderPipeline;
pub use self::text_pipeline::TextRenderPipeline;
pub use self::occlusion_pipeline::OcclusionRenderPipeline;


use std::sync::{Arc, RwLock};

use cgmath::{Matrix4, Deg};
use vulkano::command_buffer::AutoCommandBuffer;
use vulkano::device::Queue;
use vulkano::image::swapchain::SwapchainImage;
use vulkano::format::{D32Sfloat, R16G16B16A16Sfloat};
use vulkano::image::attachment::AttachmentImage;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use winit::Window;

use crate::util::Transform;
use crate::registry::TextureRegistry;
use crate::renderer::RenderQueue;


#[derive(Clone)]
pub struct PipelineCbCreateInfo {
    pub image_num: usize,
    pub dimensions: [u32; 2],
    pub queue: Arc<Queue>,
    pub camera_transform: Transform,
    pub view_mat: Matrix4<f32>,
    pub proj_mat: Matrix4<f32>,
    pub tex_registry: Arc<TextureRegistry>,
    pub hdr_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub fov: Deg<f32>,
}


pub trait RenderPipelineAbstract {
    // Required methods

    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>;
    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync>;
    fn build_command_buffer(&mut self, info: PipelineCbCreateInfo, render_queue: Arc<RwLock<RenderQueue>>) -> AutoCommandBuffer;


    // Provided methods

    fn remove_framebuffers(&mut self) { *self.get_framebuffers_mut() = None; }

    fn recreate_framebuffers_if_none(&mut self, images: &Vec<Arc<SwapchainImage<Window>>>, _hdr_buffer: &Arc<AttachmentImage<R16G16B16A16Sfloat>>, depth_buffer: &Arc<AttachmentImage<D32Sfloat>>) {
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