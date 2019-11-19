//! Rendering pipeline types.

pub mod occlusion;
pub mod deferred_shading;
pub mod deferred_lighting;
pub mod lines;
pub mod text;
pub mod postprocess;
pub use self::occlusion::OcclusionRenderPipeline;
pub use self::deferred_shading::DeferredShadingRenderPipeline;
pub use self::deferred_lighting::DeferredLightingRenderPipeline;
pub use self::lines::LinesRenderPipeline;
pub use self::text::TextRenderPipeline;
pub use self::postprocess::PostProcessRenderPipeline;


use std::sync::Arc;
use vulkano::command_buffer::AutoCommandBuffer;
use vulkano::device::Queue;
use vulkano::image::swapchain::SwapchainImage;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use winit::Window;

use crate::renderer::RenderInfo;


pub trait RenderPipelineAbstract {
    // Required methods

    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>;
    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync>;
    fn build_command_buffer(&mut self, info: &RenderInfo) -> (AutoCommandBuffer, Arc<Queue>);


    // Provided methods

    fn remove_framebuffers(&mut self) { *self.get_framebuffers_mut() = None; }

    fn recreate_framebuffers_if_none(&mut self, images: &Vec<Arc<SwapchainImage<Window>>>, info: &RenderInfo) {
        if self.get_framebuffers_mut().is_none() {
            let new_framebuffers = Some(images.iter().map(|image| {
                let arc: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(self.get_renderpass().clone())
                    .add(image.clone()).unwrap()
                    .add(info.depth_buffer_image.clone()).unwrap()
                    .build().unwrap());
                arc
            }).collect::<Vec<_>>());
            ::std::mem::replace(self.get_framebuffers_mut(), new_framebuffers);
        }
    }
}