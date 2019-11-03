use std::sync::Arc;

use cgmath::Matrix4;
use vulkano::buffer::BufferUsage;
use vulkano::buffer::cpu_pool::CpuBufferPool;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::device::Device;
use vulkano::framebuffer::{FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::swapchain::Swapchain;
use winit::Window;

use geometry::VertexPositionColorAlpha;
use renderer::RenderQueue;
use renderpass::RenderPassUnclearedColorWithDepth;
use shader::lines as LinesShaders;
use super::{RenderPipelineAbstract, PipelineCbCreateInfo};


pub struct LinesRenderPipeline {
    device: Arc<Device>,
    vulkan_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<RenderPassUnclearedColorWithDepth>>,
    uniform_buffer_pool: CpuBufferPool<LinesShaders::vertex::ty::Data>,
}


impl LinesRenderPipeline {
    pub fn new(swapchain: &Swapchain<Window>, device: &Arc<Device>) -> LinesRenderPipeline {
        let vs = LinesShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = LinesShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

        let renderpass= Arc::new(
            RenderPassUnclearedColorWithDepth { color_format: swapchain.format() }
                .build_render_pass(device.clone())
                .unwrap()
        );

        let pipeline = Arc::new(GraphicsPipeline::start()
            .vertex_input_single_buffer::<VertexPositionColorAlpha>()
            .vertex_shader(vs.main_entry_point(), ())
            .line_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .blend_alpha_blending()
            .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap());

        LinesRenderPipeline {
            device: device.clone(),
            vulkan_pipeline: pipeline,
            framebuffers: None,
            renderpass,
            uniform_buffer_pool: CpuBufferPool::<LinesShaders::vertex::ty::Data>::new(device.clone(), BufferUsage::all()),
        }
    }
}


impl RenderPipelineAbstract for LinesRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }


    fn build_command_buffer(&self, info: PipelineCbCreateInfo, render_queue: &RenderQueue) -> AutoCommandBuffer {
        let descriptor_set;
        let subbuffer = self.uniform_buffer_pool.next(LinesShaders::vertex::ty::Data {
            world: Matrix4::from_scale(1.0).into(),
            view: info.view_mat.into(),
            proj: info.proj_mat.into(),
        }).unwrap();
        descriptor_set = Arc::new(PersistentDescriptorSet::start(self.vulkan_pipeline.clone(), 0)
            .add_buffer(subbuffer).unwrap()
            .build().unwrap()
        );
        AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), info.queue.family())
            .unwrap()
            .begin_render_pass(
                self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                vec![::vulkano::format::ClearValue::None, ::vulkano::format::ClearValue::None]).unwrap()
            .draw_indexed(self.vulkan_pipeline.clone(), &DynamicState {
                line_width: None,
                viewports: Some(vec![Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [info.dimensions[0] as f32, info.dimensions[1] as f32],
                    depth_range: 0.0..1.0,
                }]),
                scissors: None,
            },
                          vec![render_queue.lines.chunk_lines_vertex_buffer.clone()],
                          render_queue.lines.chunk_lines_index_buffer.clone(),
                          descriptor_set.clone(), ()).unwrap()
            .end_render_pass().unwrap()
            .build().unwrap()
    }
}