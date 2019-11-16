use std::sync::{Arc, RwLock};

use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::device::Device;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::format::{R8G8B8A8Uint, D32Sfloat};

use crate::geometry::VertexPositionObjectId;
use crate::renderer::{RenderQueue, VULKAN_CORRECT_CLIP};
use crate::renderpass::OcclusionRenderPass;
use crate::shader::occlusion as OcclusionShaders;
use crate::pipeline::{RenderPipelineAbstract, PipelineCbCreateInfo};
use cgmath::Deg;


pub const OCCLUSION_FRAME_SIZE: [u32; 2] = [128, 96];


pub struct OcclusionRenderPipeline {
    device: Arc<Device>,
    vulkan_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    framebuffer: Arc<dyn FramebufferAbstract + Send + Sync>,
    renderpass: Arc<RenderPass<OcclusionRenderPass>>,
    color_attachment: Arc<AttachmentImage<R8G8B8A8Uint>>,
    // TODO: get rid of this (pipelines aren't really generic anymore anyway)
    dummy: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
}


impl OcclusionRenderPipeline {
    pub fn new(device: &Arc<Device>, dimensions: [u32; 2]) -> Self {
        let vs = OcclusionShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = OcclusionShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

        let renderpass = Arc::new(
            OcclusionRenderPass{}
                .build_render_pass(device.clone())
                .unwrap()
        );

        let pipeline = Arc::new(GraphicsPipeline::start()
            .cull_mode_disabled()
            .vertex_input_single_buffer::<VertexPositionObjectId>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap());

        let depth_buffer = AttachmentImage::with_usage(device.clone(),
                                                           dimensions,
                                                       D32Sfloat,
                                                           ImageUsage {
                                                               transfer_source: true,
                                                               depth_stencil_attachment: true,
                                                               ..ImageUsage::none()
                                                           }).unwrap();
        let color_attachment = AttachmentImage::with_usage(device.clone(),
                                                     dimensions,
                                                           R8G8B8A8Uint,
                                                     ImageUsage {
                                                         color_attachment: true,
                                                         transfer_source: true,
                                                         ..ImageUsage::none()
                                                     }).unwrap();

        let framebuffer = Arc::new(Framebuffer::start(renderpass.clone())
            .add(color_attachment.clone()).unwrap()
            .add(depth_buffer.clone()).unwrap()
            .build().unwrap());

        OcclusionRenderPipeline {
            device: device.clone(),
            vulkan_pipeline: pipeline,
            framebuffer,
            renderpass,
            color_attachment,
            dummy: None
        }
    }
}


impl RenderPipelineAbstract for OcclusionRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.dummy // unused
    }

    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }

    fn build_command_buffer(&mut self, info: PipelineCbCreateInfo, render_queue: Arc<RwLock<RenderQueue>>) -> AutoCommandBuffer {
        let proj = VULKAN_CORRECT_CLIP * cgmath::perspective(Deg(60f32), (OCCLUSION_FRAME_SIZE[0] as f32) / (OCCLUSION_FRAME_SIZE[1] as f32), 0.1, 100.0);
        let lock = render_queue.read().unwrap();
        AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), info.queue.family())
            .unwrap()
            .begin_render_pass(
                self.framebuffer.clone(), false,
                vec![[1u32].into(), 1f32.into()]).unwrap()
            .draw_indexed(self.vulkan_pipeline.clone(), &DynamicState {
                line_width: None,
                viewports: Some(vec![Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [OCCLUSION_FRAME_SIZE[0] as f32, OCCLUSION_FRAME_SIZE[1] as f32],
                    depth_range: 0.0..1.0,
                }]),
                scissors: None,
                compare_mask: None,
                write_mask: None,
                reference: None
            },
                          vec![lock.occluders.vertex_buffer.clone()],
                          lock.occluders.index_buffer.clone(),
                          (), OcclusionShaders::vertex::ty::Constants {
                                view: info.view_mat.into(),
                                proj: proj.into(),
                          }).unwrap()
            .end_render_pass().unwrap()
            .copy_image_to_buffer(self.color_attachment.clone(), lock.occluders.output_cpu_buffer.clone()).unwrap()
            .build().unwrap()
    }
}