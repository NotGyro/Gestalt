use std::sync::Arc;

use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::device::Queue;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::image::{AttachmentImage, ImageUsage, SwapchainImage};
use vulkano::format::{R32Uint, D32Sfloat};

use crate::geometry::VertexPositionObjectId;
use crate::renderer::{VULKAN_CORRECT_CLIP, RenderInfo};
use crate::renderpass::OcclusionRenderPass;
use crate::shader::occlusion as OcclusionShaders;
use crate::pipeline::RenderPipelineAbstract;
use cgmath::Deg;
use winit::Window;


pub const OCCLUSION_FRAME_SIZE: [u32; 2] = [192, 144];


pub struct OcclusionRenderPipeline {
    vulkan_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    framebuffer: Arc<dyn FramebufferAbstract + Send + Sync>,
    renderpass: Arc<RenderPass<OcclusionRenderPass>>,
    color_attachment: Arc<AttachmentImage<R32Uint>>,
    dummy_fb: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
}


impl OcclusionRenderPipeline {
    pub fn new(info: &mut RenderInfo, dimensions: [u32; 2]) -> Self {
        let vs = OcclusionShaders::vertex::Shader::load(info.device.clone()).expect("failed to create shader module");
        let fs = OcclusionShaders::fragment::Shader::load(info.device.clone()).expect("failed to create shader module");

        let renderpass = Arc::new(
            OcclusionRenderPass{}
                .build_render_pass(info.device.clone())
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
            .build(info.device.clone())
            .unwrap());

        let depth_buffer = AttachmentImage::with_usage(info.device.clone(), dimensions, D32Sfloat,
                                                           ImageUsage {
                                                               transfer_source: true,
                                                               depth_stencil_attachment: true,
                                                               ..ImageUsage::none()
                                                           }).unwrap();
        let color_attachment = AttachmentImage::with_usage(info.device.clone(), dimensions, R32Uint,
                                                     ImageUsage {
                                                         color_attachment: true,
                                                         transfer_source: true,
                                                         sampled: true,
                                                         ..ImageUsage::none()
                                                     }).unwrap();
        info.occlusion_buffer_image = Some(color_attachment.clone());

        let framebuffer = Arc::new(Framebuffer::start(renderpass.clone())
            .add(color_attachment.clone()).unwrap()
            .add(depth_buffer.clone()).unwrap()
            .build().unwrap());

        OcclusionRenderPipeline {
            vulkan_pipeline: pipeline,
            framebuffer,
            renderpass,
            color_attachment,
            dummy_fb: None
        }
    }
}


impl RenderPipelineAbstract for OcclusionRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> { &mut self.dummy_fb }

    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }

    fn build_command_buffer(&mut self, info: &RenderInfo) -> (AutoCommandBuffer, Arc<Queue>) {
        let proj = VULKAN_CORRECT_CLIP * cgmath::perspective(Deg(60f32), (OCCLUSION_FRAME_SIZE[0] as f32) / (OCCLUSION_FRAME_SIZE[1] as f32), 0.1, 100.0);
        let lock = info.render_queues.read().unwrap();

        let cb = AutoCommandBufferBuilder::primary_one_time_submit(info.device.clone(), info.queue_offscreen.family())
            .unwrap()
            .begin_render_pass(
                self.framebuffer.clone(), false,
                vec![[0u32].into(), 1f32.into()]).unwrap()
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
                          vec![lock.occluders.vertex_group.vertex_buffer.clone()],
                          lock.occluders.vertex_group.index_buffer.clone(),
                          (), OcclusionShaders::vertex::ty::Constants {
                                view: info.view_mat.into(),
                                proj: proj.into(),
                          }).unwrap()
            .end_render_pass().unwrap()
            .copy_image_to_buffer(self.color_attachment.clone(), lock.occluders.output_cpu_buffer.clone()).unwrap()
            .build().unwrap();
        (cb, info.queue_offscreen.clone())
    }

    fn recreate_framebuffers_if_none(&mut self, _: &Vec<Arc<SwapchainImage<Window>>>, _: &RenderInfo) {
        // OcclusionRenderPipeline uses a fixed offscreen framebuffer
    }
}