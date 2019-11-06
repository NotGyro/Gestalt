use std::sync::{Arc, RwLock};
use std::path::Path;

use vulkano::buffer::BufferUsage;
use vulkano::buffer::cpu_pool::CpuBufferPool;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::device::{Device, Queue};
use vulkano::framebuffer::{FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, MipmapMode};
use vulkano::swapchain::Swapchain;
use vulkano::format::R8G8B8A8Srgb;
use vulkano::image::immutable::ImmutableImage;
use winit::Window;

use buffer::CpuAccessibleBufferAutoPool;
use geometry::VertexPositionUV;
use memory::pool::AutoMemoryPool;
use renderpass::RenderPassClearedColorWithDepth;
use renderer::RenderQueue;
use shader::skybox as SkyboxShaders;
use super::{RenderPipelineAbstract, PipelineCbCreateInfo};


pub struct SkyboxRenderPipeline {
    device: Arc<Device>,
    vulkan_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<RenderPassClearedColorWithDepth>>,
    uniform_buffer_pool: CpuBufferPool<SkyboxShaders::vertex::ty::Data>,
    vertex_buffer: Arc<CpuAccessibleBufferAutoPool<[VertexPositionUV]>>,
    index_buffer: Arc<CpuAccessibleBufferAutoPool<[u32]>>,
    sampler: Arc<Sampler>,
    texture: Arc<ImmutableImage<R8G8B8A8Srgb>>
}


impl SkyboxRenderPipeline {
    pub fn new(swapchain: &Swapchain<Window>, device: &Arc<Device>, queue: &Arc<Queue>, memory_pool: &AutoMemoryPool) -> SkyboxRenderPipeline {
        let vs = SkyboxShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = SkyboxShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

        let renderpass = Arc::new(
            RenderPassClearedColorWithDepth { color_format: swapchain.format() }
                .build_render_pass(device.clone())
                .unwrap()
        );

        let pipeline = Arc::new(GraphicsPipeline::start()
            .vertex_input_single_buffer::<VertexPositionUV>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .blend_alpha_blending()
            .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap());

        const SIZE: f32 = 500.0;
        let verts = vec![
            VertexPositionUV { position: [  SIZE, -SIZE, -SIZE ], uv: [ 0.3333, 0.5 ] },
            VertexPositionUV { position: [ -SIZE, -SIZE, -SIZE ], uv: [ 0.6666, 0.5 ] },
            VertexPositionUV { position: [ -SIZE,  SIZE, -SIZE ], uv: [ 0.6666, 0.0 ] },
            VertexPositionUV { position: [  SIZE,  SIZE, -SIZE ], uv: [ 0.3333, 0.0 ] },

            VertexPositionUV { position: [  SIZE, -SIZE,  SIZE ], uv: [ 1.0000, 0.5 ] },
            VertexPositionUV { position: [  SIZE, -SIZE, -SIZE ], uv: [ 0.6666, 0.5 ] },
            VertexPositionUV { position: [  SIZE,  SIZE, -SIZE ], uv: [ 0.6666, 0.0 ] },
            VertexPositionUV { position: [  SIZE,  SIZE,  SIZE ], uv: [ 1.0000, 0.0 ] },

            VertexPositionUV { position: [ -SIZE, -SIZE,  SIZE ], uv: [ 0.3335, 1.0 ] },
            VertexPositionUV { position: [  SIZE, -SIZE,  SIZE ], uv: [ 0.6663, 1.0 ] },
            VertexPositionUV { position: [  SIZE,  SIZE,  SIZE ], uv: [ 0.6663, 0.5 ] },
            VertexPositionUV { position: [ -SIZE,  SIZE,  SIZE ], uv: [ 0.3335, 0.5 ] },

            VertexPositionUV { position: [ -SIZE, -SIZE, -SIZE ], uv: [ 0.3333, 0.5 ] },
            VertexPositionUV { position: [ -SIZE, -SIZE,  SIZE ], uv: [ 0.0000, 0.5 ] },
            VertexPositionUV { position: [ -SIZE,  SIZE,  SIZE ], uv: [ 0.0000, 0.0 ] },
            VertexPositionUV { position: [ -SIZE,  SIZE, -SIZE ], uv: [ 0.3333, 0.0 ] },

            VertexPositionUV { position: [  SIZE, -SIZE,  SIZE ], uv: [ 0.668, 0.502 ] },
            VertexPositionUV { position: [ -SIZE, -SIZE,  SIZE ], uv: [ 0.998, 0.502 ] },
            VertexPositionUV { position: [ -SIZE, -SIZE, -SIZE ], uv: [ 0.998, 0.998 ] },
            VertexPositionUV { position: [  SIZE, -SIZE, -SIZE ], uv: [ 0.668, 0.998 ] },

            VertexPositionUV { position: [ -SIZE,  SIZE,  SIZE ], uv: [ 0.332, 0.998 ] },
            VertexPositionUV { position: [  SIZE,  SIZE,  SIZE ], uv: [ 0.001, 0.998 ] },
            VertexPositionUV { position: [  SIZE,  SIZE, -SIZE ], uv: [ 0.001, 0.502 ] },
            VertexPositionUV { position: [ -SIZE,  SIZE, -SIZE ], uv: [ 0.332, 0.502 ] },
        ];
        let idxs = vec![
            0, 1, 2, 2, 3, 0,
            4, 5, 6, 6, 7, 4,
            8, 9, 10, 10, 11, 8,
            12, 13, 14, 14, 15, 12,
            16, 17, 18, 18, 19, 16,
            20, 21, 22, 22, 23, 20
        ];

        let vertex_buffer = CpuAccessibleBufferAutoPool::<[VertexPositionUV]>::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), verts.iter().cloned()).expect("failed to create buffer");
        let index_buffer = CpuAccessibleBufferAutoPool::<[u32]>::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), idxs.iter().cloned()).expect("failed to create buffer");

        let (texture, _future) = {
            let path_str = String::from("textures/skybox.png");
            let image = ::image::open(Path::new(&path_str)).unwrap().to_rgba();
            let (w, h) = image.dimensions();
            let image_data = image.into_raw().clone();

            ::vulkano::image::immutable::ImmutableImage::from_iter(
                image_data.iter().cloned(),
                ::vulkano::image::Dimensions::Dim2d { width: w, height: h },
                ::vulkano::format::R8G8B8A8Srgb,
                queue.clone()).unwrap()
        };

        SkyboxRenderPipeline {
            device: device.clone(),
            vulkan_pipeline: pipeline,
            framebuffers: None,
            renderpass,
            uniform_buffer_pool: CpuBufferPool::<SkyboxShaders::vertex::ty::Data>::new(device.clone(), BufferUsage::all()),
            vertex_buffer,
            index_buffer,
            sampler: Sampler::new(device.clone(), Filter::Nearest, Filter::Nearest, MipmapMode::Nearest,
                                  SamplerAddressMode::Repeat, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
                                  0.0, 4.0, 0.0, 0.0).unwrap(),
            texture
        }
    }
}


impl RenderPipelineAbstract for SkyboxRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }


    fn build_command_buffer(&self, info: PipelineCbCreateInfo, _rq: Arc<RwLock<RenderQueue>>) -> AutoCommandBuffer {
        let descriptor_set;
        let subbuffer = self.uniform_buffer_pool.next(SkyboxShaders::vertex::ty::Data {
            projection: info.proj_mat.into(),
            view: info.view_mat.into()
        }).unwrap();
        descriptor_set = Arc::new(PersistentDescriptorSet::start(self.vulkan_pipeline.clone(), 0)
            .add_buffer(subbuffer).unwrap()
            .add_sampled_image(self.texture.clone(), self.sampler.clone()).unwrap()
            .build().unwrap()
        );

        AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), info.queue.family())
            .unwrap()
            .begin_render_pass(
                self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                vec![[0.0, 0.0, 0.0, 1.0].into(), 1f32.into()]).unwrap()
            .draw_indexed(self.vulkan_pipeline.clone(), &DynamicState {
                line_width: None,
                viewports: Some(vec![Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [info.dimensions[0] as f32, info.dimensions[1] as f32],
                    depth_range: 0.0..1.0,
                }]),
                scissors: None,
            },
                          vec![self.vertex_buffer.clone()],
                          self.index_buffer.clone(),
                          descriptor_set.clone(), ()).unwrap()
            .end_render_pass().unwrap()
            .build().unwrap()
    }
}
