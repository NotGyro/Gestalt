use std::sync::Arc;

use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::device::Queue;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::format::ClearValue;
use vulkano::image::SwapchainImage;

use crate::geometry::VertexPosition;
use crate::renderer::RenderInfo;
use crate::renderpass::PostProcessRenderPass;
use crate::shader::tonemapper as TonemapperShaders;
use crate::pipeline::RenderPipelineAbstract;
use crate::buffer::CpuAccessibleBufferXalloc;
use winit::Window;
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, MipmapMode};


pub struct PostProcessRenderPipeline {
    pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<PostProcessRenderPass>>,
    fullscreen_vertex_buffer: Arc<CpuAccessibleBufferXalloc<[VertexPosition]>>,
    occlusion_buf_sampler: Arc<Sampler>,
}


impl PostProcessRenderPipeline {
    pub fn new(info: &RenderInfo) -> Self {
        let renderpass = Arc::new(
            PostProcessRenderPass {}
                .build_render_pass(info.device.clone())
                .unwrap()
        );

        let pipeline = {
            let vs = TonemapperShaders::vertex::Shader::load(info.device.clone()).expect("failed to create shader module");
            let fs = TonemapperShaders::fragment::Shader::load(info.device.clone()).expect("failed to create shader module");

            Arc::new(GraphicsPipeline::start()
                .vertex_input_single_buffer::<VertexPosition>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .blend_alpha_blending()
                .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
                .build(info.device.clone())
                .unwrap())
        };

        let fullscreen_vertex_buffer = CpuAccessibleBufferXalloc::<[VertexPosition]>::from_iter(info.device.clone(), info.memory_pool.clone(), BufferUsage::all(), vec![
            VertexPosition { position: [ -1.0,  1.0, 1.0 ] },
            VertexPosition { position: [  1.0,  1.0, 1.0 ] },
            VertexPosition { position: [  1.0, -1.0, 1.0 ] },
            VertexPosition { position: [ -1.0,  1.0, 1.0 ] },
            VertexPosition { position: [  1.0, -1.0, 1.0 ] },
            VertexPosition { position: [ -1.0, -1.0, 1.0 ] },
        ].iter().cloned()).expect("failed to create buffer");

        let occlusion_buf_sampler = Sampler::new(info.device.clone(), Filter::Nearest, Filter::Nearest, MipmapMode::Linear,
                                                 SamplerAddressMode::Repeat, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
                                                 0.0, 4.0, 0.0, 0.0).unwrap();

        PostProcessRenderPipeline {
            pipeline,
            framebuffers: None,
            renderpass,
            fullscreen_vertex_buffer,
            occlusion_buf_sampler
        }
    }
}


impl RenderPipelineAbstract for PostProcessRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }

    fn build_command_buffer(&mut self, info: &RenderInfo) -> (AutoCommandBuffer, Arc<Queue>) {
        let descriptor_set = Arc::new(PersistentDescriptorSet::start(self.pipeline.clone(), 0)
            .add_image(info.hdr_color_buffer_image.clone()).unwrap()
            .add_sampled_image(info.occlusion_buffer_image.as_ref().unwrap().clone(), self.occlusion_buf_sampler.clone()).unwrap()
            .build().unwrap()
        );

        let mut cb = AutoCommandBufferBuilder::primary_one_time_submit(info.device.clone(), info.queue_main.family()).unwrap()
            .begin_render_pass(
                self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                vec![ClearValue::None, [0.0, 0.0, 0.0, 1.0].into()]).unwrap();

        cb = cb.draw(self.pipeline.clone(), &DynamicState {
            line_width: None,
            viewports: Some(vec![Viewport {
                origin: [0.0, 0.0],
                dimensions: [info.dimensions[0] as f32, info.dimensions[1] as f32],
                depth_range: 0.0..1.0,
            }]),
            scissors: None,
            compare_mask: None,
            write_mask: None,
            reference: None
        },
                             vec![self.fullscreen_vertex_buffer.clone()],
                             descriptor_set, TonemapperShaders::fragment::ty::Constants {
                                exposure: 1.0,
                                debug_vis_mode: info.debug_visualize_setting,
                                screen_dimensions: [info.dimensions[0] as f32, info.dimensions[1] as f32]
                            }).unwrap()
            .end_render_pass().unwrap();
        (cb.build().unwrap(), info.queue_main.clone())
    }

    fn recreate_framebuffers_if_none(&mut self, images: &Vec<Arc<SwapchainImage<Window>>>, info: &RenderInfo) {
        if self.get_framebuffers_mut().is_none() {
            let new_framebuffers = Some(images.iter().map(|image| {
                let arc: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(self.get_renderpass().clone())
                    .add(info.hdr_color_buffer_image.clone()).unwrap()
                    .add(image.clone()).unwrap()
                    .build().unwrap());
                arc
            }).collect::<Vec<_>>());
            ::std::mem::replace(self.get_framebuffers_mut(), new_framebuffers);
        }
    }
}
