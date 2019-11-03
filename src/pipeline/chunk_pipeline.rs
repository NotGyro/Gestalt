use std::sync::Arc;

use vulkano::buffer::BufferUsage;
use vulkano::buffer::cpu_pool::CpuBufferPool;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::device::Device;
use vulkano::framebuffer::{FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, MipmapMode};
use vulkano::swapchain::Swapchain;
use winit::Window;

use geometry::VertexPositionNormalUVColor;
use renderpass::RenderPassUnclearedColorWithDepth;
use renderer::RenderQueue;
use shader::chunks as ChunksShaders;
use super::{RenderPipelineAbstract, PipelineCbCreateInfo};


pub struct ChunkRenderPipeline {
    device: Arc<Device>,
    vulkan_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<RenderPassUnclearedColorWithDepth>>,
    uniform_buffer_pool: CpuBufferPool<ChunksShaders::vertex::ty::Data>,
    sampler: Arc<Sampler>,
}


impl ChunkRenderPipeline {
    pub fn new(swapchain: &Swapchain<Window>, device: &Arc<Device>) -> ChunkRenderPipeline {
        let vs = ChunksShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = ChunksShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

        let renderpass = Arc::new(
            RenderPassUnclearedColorWithDepth { color_format: swapchain.format() }
                .build_render_pass(device.clone())
                .unwrap()
        );

        let pipeline = Arc::new(GraphicsPipeline::start()
            .cull_mode_back()
            .vertex_input_single_buffer::<VertexPositionNormalUVColor>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .blend_alpha_blending()
            .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap());

        ChunkRenderPipeline {
            device: device.clone(),
            vulkan_pipeline: pipeline,
            framebuffers: None,
            renderpass,
            uniform_buffer_pool: CpuBufferPool::<ChunksShaders::vertex::ty::Data>::new(device.clone(), BufferUsage::all()),
            sampler: Sampler::new(device.clone(), Filter::Nearest, Filter::Nearest, MipmapMode::Nearest,
                                  SamplerAddressMode::Repeat, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
                                  0.0, 4.0, 0.0, 0.0).unwrap(),
        }
    }
}


impl RenderPipelineAbstract for ChunkRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }


    fn build_command_buffer(&self, info: PipelineCbCreateInfo, render_queue: &RenderQueue) -> AutoCommandBuffer {
        let mut descriptor_sets = Vec::new();
        for entry in render_queue.chunk_meshes.iter() {
            let uniform_data = ChunksShaders::vertex::ty::Data {
                world: entry.transform.clone().into(),
                view: info.view_mat.into(),
                proj: info.proj_mat.into(),
                view_pos: info.camera_transform.position.into(),
                specular_exponent: entry.material.specular_exponent,
                specular_strength: entry.material.specular_strength
            };

            let subbuffer = self.uniform_buffer_pool.next(uniform_data).unwrap();
            descriptor_sets.push(Arc::new(PersistentDescriptorSet::start(self.vulkan_pipeline.clone(), 0)
                .add_sampled_image(info.tex_registry.get(&entry.material.albedo_map_name).unwrap().clone(), self.sampler.clone()).unwrap()
                .add_buffer(subbuffer).unwrap()
                .build().unwrap()
            ));
        };

        let mut cb = AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), info.queue.family())
            .unwrap()
            .begin_render_pass(
                self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                vec![::vulkano::format::ClearValue::None, ::vulkano::format::ClearValue::None]).unwrap();
        for (i, entry) in render_queue.chunk_meshes.iter().enumerate() {
            cb = cb.draw_indexed(self.vulkan_pipeline.clone(), &DynamicState {
                line_width: None,
                viewports: Some(vec![Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [info.dimensions[0] as f32, info.dimensions[1] as f32],
                    depth_range: 0.0..1.0,
                }]),
                scissors: None,
            },
                                 vec![entry.vertex_group.vertex_buffer.as_ref().unwrap().clone()],
                                 entry.vertex_group.index_buffer.as_ref().unwrap().clone(),
                                 descriptor_sets[i].clone(), ()).unwrap();
        }
        cb.end_render_pass().unwrap().build().unwrap()
    }
}
