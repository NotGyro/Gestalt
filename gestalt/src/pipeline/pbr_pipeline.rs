use std::sync::{Arc, RwLock};

use vulkano::buffer::BufferUsage;
use vulkano::buffer::cpu_pool::CpuBufferPool;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::device::{Device, Queue};
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, MipmapMode};
use vulkano::swapchain::Swapchain;
use vulkano::format::{D32Sfloat, R16G16B16A16Sfloat, R8G8B8A8Srgb};
use winit::Window;
use vulkano::image::{SwapchainImage, AttachmentImage, ImmutableImage};

use crate::geometry::{PBRPipelineVertex, VertexPosition, VertexPositionUV};
use crate::renderpass::PBRMainRenderPass;
use crate::renderer::RenderQueue;
use crate::shader::pbr as PBRShaders;
use crate::shader::tonemapper as TonemapperShaders;
use crate::shader::skybox as SkyboxShaders;
use crate::pipeline::{RenderPipelineAbstract, PipelineCbCreateInfo};
use crate::pipeline::text_pipeline::TextData;
use crate::buffer::CpuAccessibleBufferXalloc;
use std::path::Path;
use crate::memory::xalloc::XallocMemoryPool;


pub struct PBRRenderPipeline {
    device: Arc<Device>,
    voxel_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    tonemapper_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    skybox_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<PBRMainRenderPass>>,
    voxel_uniform_buffer_pool: CpuBufferPool<PBRShaders::vertex::ty::Data>,
    tonemapper_uniform_buffer_pool: CpuBufferPool<TonemapperShaders::fragment::ty::Data>,
    skybox_uniform_buffer_pool: CpuBufferPool<SkyboxShaders::vertex::ty::Data>,
    fullscreen_vertex_buffer: Arc<CpuAccessibleBufferXalloc<[VertexPosition]>>,
    linear_sampler: Arc<Sampler>,
    skybox_vertex_buffer: Arc<CpuAccessibleBufferXalloc<[VertexPositionUV]>>,
    skybox_index_buffer: Arc<CpuAccessibleBufferXalloc<[u32]>>,
    skybox_texture: Arc<ImmutableImage<R8G8B8A8Srgb>>
}


impl PBRRenderPipeline {
    pub fn new(_swapchain: &Swapchain<Window>, device: &Arc<Device>, queue: &Arc<Queue>, memory_pool: &XallocMemoryPool) -> Self {
        let renderpass = Arc::new(
            PBRMainRenderPass{}
                .build_render_pass(device.clone())
                .unwrap()
        );

        let skybox_pipeline = {
            let vs = SkyboxShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
            let fs = SkyboxShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

            Arc::new(GraphicsPipeline::start()
                .vertex_input_single_buffer::<VertexPositionUV>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .depth_stencil_simple_depth()
                .blend_alpha_blending()
                .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
                .build(device.clone())
                .unwrap())
        };

        let voxel_pipeline = {
            let vs = PBRShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
            let fs = PBRShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

            Arc::new(GraphicsPipeline::start()
                .cull_mode_back()
                .vertex_input_single_buffer::<PBRPipelineVertex>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .depth_stencil_simple_depth()
                .blend_alpha_blending()
                .render_pass(Subpass::from(renderpass.clone(), 1).unwrap())
                .build(device.clone())
                .unwrap())
        };

        let tonemapper_pipeline = {
            let vs = TonemapperShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
            let fs = TonemapperShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

            Arc::new(GraphicsPipeline::start()
                .cull_mode_disabled()
                .vertex_input_single_buffer::<VertexPosition>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .render_pass(Subpass::from(renderpass.clone(), 2).unwrap())
                .build(device.clone())
                .unwrap())
        };

        let fullscreen_vertex_buffer = CpuAccessibleBufferXalloc::<[VertexPosition]>::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), vec![
            VertexPosition { position: [ -1.0,  1.0, 0.0 ] },
            VertexPosition { position: [  1.0,  1.0, 0.0 ] },
            VertexPosition { position: [  1.0, -1.0, 0.0 ] },
            VertexPosition { position: [ -1.0,  1.0, 0.0 ] },
            VertexPosition { position: [  1.0, -1.0, 0.0 ] },
            VertexPosition { position: [ -1.0, -1.0, 0.0 ] },
        ].iter().cloned()).expect("failed to create buffer");

        const SKYBOX_SIZE: f32 = 5000.0;
        let skybox_verts = vec![
            VertexPositionUV { position: [  SKYBOX_SIZE, -SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.3333, 0.5 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE, -SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.6666, 0.5 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE,  SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.6666, 0.0 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE,  SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.3333, 0.0 ] },

            VertexPositionUV { position: [  SKYBOX_SIZE, -SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 1.0000, 0.5 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE, -SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.6666, 0.5 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE,  SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.6666, 0.0 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE,  SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 1.0000, 0.0 ] },

            VertexPositionUV { position: [ -SKYBOX_SIZE, -SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.3335, 1.0 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE, -SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.6663, 1.0 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE,  SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.6663, 0.5 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE,  SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.3335, 0.5 ] },

            VertexPositionUV { position: [ -SKYBOX_SIZE, -SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.3333, 0.5 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE, -SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.0000, 0.5 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE,  SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.0000, 0.0 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE,  SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.3333, 0.0 ] },

            VertexPositionUV { position: [  SKYBOX_SIZE, -SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.668, 0.502 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE, -SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.998, 0.502 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE, -SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.998, 0.998 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE, -SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.668, 0.998 ] },

            VertexPositionUV { position: [ -SKYBOX_SIZE,  SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.332, 0.998 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE,  SKYBOX_SIZE,  SKYBOX_SIZE ], uv: [ 0.001, 0.998 ] },
            VertexPositionUV { position: [  SKYBOX_SIZE,  SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.001, 0.502 ] },
            VertexPositionUV { position: [ -SKYBOX_SIZE,  SKYBOX_SIZE, -SKYBOX_SIZE ], uv: [ 0.332, 0.502 ] },
        ];
        let skybox_idxs = vec![
            0, 1, 2, 2, 3, 0,
            4, 5, 6, 6, 7, 4,
            8, 9, 10, 10, 11, 8,
            12, 13, 14, 14, 15, 12,
            16, 17, 18, 18, 19, 16,
            20, 21, 22, 22, 23, 20
        ];
        let skybox_vertex_buffer = CpuAccessibleBufferXalloc::<[VertexPositionUV]>::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), skybox_verts.iter().cloned()).expect("failed to create buffer");
        let skybox_index_buffer = CpuAccessibleBufferXalloc::<[u32]>::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), skybox_idxs.iter().cloned()).expect("failed to create buffer");

        let (skybox_texture, _future) = {
            let path_str = String::from("textures/skybox.png");
            let image = ::image::open(Path::new(&path_str)).unwrap().to_rgba();
            let (w, h) = image.dimensions();
            let image_data = image.into_raw().clone();

            vulkano::image::immutable::ImmutableImage::from_iter(
                image_data.iter().cloned(),
                vulkano::image::Dimensions::Dim2d { width: w, height: h },
                vulkano::format::R8G8B8A8Srgb,
                queue.clone()).unwrap()
        };

        PBRRenderPipeline {
            device: device.clone(),
            voxel_pipeline,
            tonemapper_pipeline,
            skybox_pipeline,
            framebuffers: None,
            renderpass,
            voxel_uniform_buffer_pool: CpuBufferPool::<PBRShaders::vertex::ty::Data>::new(device.clone(), BufferUsage::all()),
            tonemapper_uniform_buffer_pool: CpuBufferPool::<TonemapperShaders::fragment::ty::Data>::new(device.clone(), BufferUsage::all()),
            skybox_uniform_buffer_pool: CpuBufferPool::<SkyboxShaders::vertex::ty::Data>::new(device.clone(), BufferUsage::all()),
            fullscreen_vertex_buffer,
            linear_sampler: Sampler::new(device.clone(), Filter::Linear, Filter::Linear, MipmapMode::Linear,
                                         SamplerAddressMode::Repeat, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
                                         0.0, 4.0, 0.0, 0.0).unwrap(),
            skybox_vertex_buffer,
            skybox_index_buffer,
            skybox_texture
        }
    }
}


impl RenderPipelineAbstract for PBRRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }

    fn build_command_buffer(&mut self, info: PipelineCbCreateInfo, render_queue: Arc<RwLock<RenderQueue>>) -> AutoCommandBuffer {
        {
            let mut lock = render_queue.write().unwrap();
            let num = lock.chunk_meshes.len();
            lock.text.push(TextData {
                text: format!("Chunks drawing: {}", num),
                position: (10, 70),
                ..TextData::default()
            });
        }
        let mut voxel_descriptor_sets = Vec::new();
        let lock = render_queue.read().unwrap();
        for entry in lock.chunk_meshes.iter() {
            let uniform_data = PBRShaders::vertex::ty::Data {
                world: entry.transform.clone().into(),
                view: info.view_mat.into(),
                proj: info.proj_mat.into(),
                view_pos: info.camera_transform.position.into(),
                specular_exponent: entry.material.specular_exponent,
                specular_strength: entry.material.specular_strength
            };

            let subbuffer = self.voxel_uniform_buffer_pool.next(uniform_data).unwrap();
            voxel_descriptor_sets.push(Arc::new(PersistentDescriptorSet::start(self.voxel_pipeline.clone(), 0)
                .add_buffer(subbuffer).unwrap()
                .add_sampled_image(info.tex_registry.get("test_albedo").unwrap().clone(), self.linear_sampler.clone()).unwrap()
                .add_sampled_image(info.tex_registry.get("test_normal").unwrap().clone(), self.linear_sampler.clone()).unwrap()
                .add_sampled_image(info.tex_registry.get("black").unwrap().clone(), self.linear_sampler.clone()).unwrap()
                .add_sampled_image(info.tex_registry.get("black").unwrap().clone(), self.linear_sampler.clone()).unwrap()
                .build().unwrap()
            ));
        };

        let tonemapper_subbuffer = self.tonemapper_uniform_buffer_pool.next(TonemapperShaders::fragment::ty::Data {
            exposure: 1.0
        }).unwrap();
        let tonemapper_descriptor_set = Arc::new(PersistentDescriptorSet::start(self.tonemapper_pipeline.clone(), 0)
            .add_buffer(tonemapper_subbuffer).unwrap()
            .add_image(info.hdr_buffer_image.clone()).unwrap()
            .build().unwrap()
        );
        let skybox_subbuffer = self.skybox_uniform_buffer_pool.next(SkyboxShaders::vertex::ty::Data {
            projection: info.proj_mat.into(),
            view: info.view_mat.into()
        }).unwrap();
        let skybox_descriptor_set = Arc::new(PersistentDescriptorSet::start(self.skybox_pipeline.clone(), 0)
            .add_buffer(skybox_subbuffer).unwrap()
            .add_sampled_image(self.skybox_texture.clone(), self.linear_sampler.clone()).unwrap()
            .build().unwrap()
        );

        let mut cb = AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), info.queue.family())
            .unwrap()
            .begin_render_pass(
                self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                vec![[0.0, 0.0, 0.0, 1.0].into(), [0.0, 0.0, 0.0, 1.0].into(), 1f32.into()]).unwrap();

        // skybox
        cb = cb.draw_indexed(self.skybox_pipeline.clone(), &DynamicState {
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
                           vec![self.skybox_vertex_buffer.clone()],
                           self.skybox_index_buffer.clone(),
                           skybox_descriptor_set, ()).unwrap()
            .next_subpass(false).unwrap();

        // chunks
        for (i, entry) in lock.chunk_meshes.iter().enumerate() {
            cb = cb.draw_indexed(self.voxel_pipeline.clone(), &DynamicState {
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
                                 vec![entry.vertex_group.vertex_buffer.as_ref().unwrap().clone()],
                                 entry.vertex_group.index_buffer.as_ref().unwrap().clone(),
                                 voxel_descriptor_sets[i].clone(), ()).unwrap();
        }
        // tonemapper
        cb = cb.next_subpass(false).unwrap()
            .draw(self.tonemapper_pipeline.clone(), &DynamicState {
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
                  tonemapper_descriptor_set, ()).unwrap();
        cb.end_render_pass().unwrap().build().unwrap()
    }

    fn recreate_framebuffers_if_none(&mut self, images: &Vec<Arc<SwapchainImage<Window>>>, hdr_buffer: &Arc<AttachmentImage<R16G16B16A16Sfloat>>, depth_buffer: &Arc<AttachmentImage<D32Sfloat>>) {
        if self.get_framebuffers_mut().is_none() {
            let new_framebuffers = Some(images.iter().map(|image| {
                let arc: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(self.get_renderpass().clone())
                    .add(hdr_buffer.clone()).unwrap()
                    .add(image.clone()).unwrap()
                    .add(depth_buffer.clone()).unwrap()
                    .build().unwrap());
                arc
            }).collect::<Vec<_>>());
            ::std::mem::replace(self.get_framebuffers_mut(), new_framebuffers);
        }
    }
}
