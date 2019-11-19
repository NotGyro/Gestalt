use std::sync::Arc;
use std::path::Path;

use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor::DescriptorSet;
use vulkano::device::Queue;
use vulkano::format::R8G8B8A8Srgb;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::image::{SwapchainImage, ImmutableImage};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, MipmapMode};
use vulkano::image::Dimensions::Dim2d;
use winit::Window;

use crate::cpu_pool::XallocCpuBufferPool;
use crate::geometry::{DeferredShadingVertex, VertexPositionUV};
use crate::pipeline::text::TextData;
use crate::pipeline::RenderPipelineAbstract;
use crate::renderer::RenderInfo;
use crate::renderpass::DeferredShadingRenderPass;
use crate::shader::deferred_shading as DeferredShadingShaders;
use crate::shader::skybox as SkyboxShaders;
use crate::buffer::CpuAccessibleBufferXalloc;


pub struct DeferredShadingRenderPipeline {
    skybox_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    voxel_shading_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<DeferredShadingRenderPass>>,
    voxel_uniform_buffer_pool: XallocCpuBufferPool<DeferredShadingShaders::vertex::ty::InstanceData>,
    skybox_uniform_buffer_pool: XallocCpuBufferPool<SkyboxShaders::vertex::ty::Data>,
    // TODO: texture bindings per material
    voxel_texture_descriptors: Arc<dyn DescriptorSet + Send + Sync>,
    linear_sampler: Arc<Sampler>,
    skybox_vertex_buffer: Arc<CpuAccessibleBufferXalloc<[VertexPositionUV]>>,
    skybox_index_buffer: Arc<CpuAccessibleBufferXalloc<[u32]>>,
    skybox_texture: Arc<ImmutableImage<R8G8B8A8Srgb>>,
}


impl DeferredShadingRenderPipeline {
    pub fn new(info: &RenderInfo) -> Self {
        let renderpass = Arc::new(
            DeferredShadingRenderPass {}
                .build_render_pass(info.device.clone())
                .unwrap()
        );

        let skybox_pipeline = {
            let vs = SkyboxShaders::vertex::Shader::load(info.device.clone()).expect("failed to create shader module");
            let fs = SkyboxShaders::fragment::Shader::load(info.device.clone()).expect("failed to create shader module");

            Arc::new(GraphicsPipeline::start()
                .vertex_input_single_buffer::<VertexPositionUV>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .depth_stencil_simple_depth()
                .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
                .build(info.device.clone())
                .unwrap())
        };

        let voxel_shading_pipeline = {
            let vs = DeferredShadingShaders::vertex::Shader::load(info.device.clone()).expect("failed to create shader module");
            let fs = DeferredShadingShaders::fragment::Shader::load(info.device.clone()).expect("failed to create shader module");

            Arc::new(GraphicsPipeline::start()
                .cull_mode_back()
                .vertex_input_single_buffer::<DeferredShadingVertex>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .depth_stencil_simple_depth()
                .render_pass(Subpass::from(renderpass.clone(), 1).unwrap())
                .build(info.device.clone())
                .unwrap())
        };

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
        let skybox_vertex_buffer = CpuAccessibleBufferXalloc::<[VertexPositionUV]>::from_iter(
            info.device.clone(), info.memory_pool.clone(), BufferUsage::all(),
            skybox_verts.iter().cloned()).expect("failed to create buffer");
        let skybox_index_buffer = CpuAccessibleBufferXalloc::<[u32]>::from_iter(
            info.device.clone(), info.memory_pool.clone(), BufferUsage::all(),
            skybox_idxs.iter().cloned()).expect("failed to create buffer");

        let (skybox_texture, _future) = {
            let path_str = String::from("textures/skybox.png");
            let image = image::open(Path::new(&path_str)).unwrap().to_rgba();
            let (w, h) = image.dimensions();
            let image_data = image.into_raw().clone();

            ImmutableImage::from_iter(image_data.iter().cloned(), Dim2d { width: w, height: h }, R8G8B8A8Srgb, info.queue_main.clone()).unwrap()
        };

        let linear_sampler = Sampler::new(info.device.clone(), Filter::Linear, Filter::Linear, MipmapMode::Linear,
            SamplerAddressMode::Repeat, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
            0.0, 4.0, 0.0, 0.0).unwrap();

        let voxel_texture_descriptors = Arc::new(PersistentDescriptorSet::start(voxel_shading_pipeline.clone(), 0)
            .add_sampled_image(info.tex_registry.get("test_albedo").unwrap().clone(), linear_sampler.clone()).unwrap()
            .add_sampled_image(info.tex_registry.get("test_normal").unwrap().clone(), linear_sampler.clone()).unwrap()
            .add_sampled_image(info.tex_registry.get("black").unwrap().clone(), linear_sampler.clone()).unwrap()
            .add_sampled_image(info.tex_registry.get("black").unwrap().clone(), linear_sampler.clone()).unwrap()
            .build().unwrap()
        );

        DeferredShadingRenderPipeline {
            skybox_pipeline,
            voxel_shading_pipeline,
            framebuffers: None,
            renderpass,
            voxel_uniform_buffer_pool: XallocCpuBufferPool::<DeferredShadingShaders::vertex::ty::InstanceData>::new(info.device.clone(), BufferUsage::all(), info.memory_pool.clone()),
            skybox_uniform_buffer_pool: XallocCpuBufferPool::<SkyboxShaders::vertex::ty::Data>::new(info.device.clone(), BufferUsage::all(), info.memory_pool.clone()),
            voxel_texture_descriptors,
            linear_sampler,
            skybox_vertex_buffer,
            skybox_index_buffer,
            skybox_texture,
        }
    }
}

const CLEAR_BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

impl RenderPipelineAbstract for DeferredShadingRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }

    fn build_command_buffer(&mut self, info: &RenderInfo) -> (AutoCommandBuffer, Arc<Queue>) {
        let skybox_subbuffer = self.skybox_uniform_buffer_pool.next(SkyboxShaders::vertex::ty::Data {
            projection: info.proj_mat.into(),
            view: info.view_mat.into()
        }).unwrap();

        let skybox_descriptor_set = Arc::new(PersistentDescriptorSet::start(self.skybox_pipeline.clone(), 0)
            .add_buffer(skybox_subbuffer).unwrap()
            .add_sampled_image(self.skybox_texture.clone(), self.linear_sampler.clone()).unwrap()
            .build().unwrap()
        );

        {
            let mut lock = info.render_queues.write().unwrap();
            let num = lock.meshes.len();
            lock.text.push(TextData {
                text: format!("Chunks drawing: {}", num),
                position: (5, 110),
                ..TextData::default()
            });
        }

        let mut voxel_descriptor_sets = Vec::new();
        let lock = info.render_queues.read().unwrap();
        for entry in lock.meshes.iter() {
            let uniform_data = DeferredShadingShaders::vertex::ty::InstanceData {
                world: entry.transform.clone().into()
            };

            let subbuffer = self.voxel_uniform_buffer_pool.next(uniform_data).unwrap();
            voxel_descriptor_sets.push(Arc::new(PersistentDescriptorSet::start(self.voxel_shading_pipeline.clone(), 1)
                .add_buffer(subbuffer).unwrap()
                .build().unwrap()
            ));
        };

        let mut cb = AutoCommandBufferBuilder::primary_one_time_submit(info.device.clone(), info.queue_main.family())
            .unwrap()
            .begin_render_pass(self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                               vec![CLEAR_BLACK.into(), CLEAR_BLACK.into(), CLEAR_BLACK.into(), CLEAR_BLACK.into(), CLEAR_BLACK.into(), 1f32.into()]).unwrap()
                .draw_indexed(self.skybox_pipeline.clone(), &DynamicState {
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

        for (i, entry) in lock.meshes.iter().enumerate() {
            cb = cb.draw_indexed(self.voxel_shading_pipeline.clone(), &DynamicState {
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
                                 vec![entry.vertex_group.vertex_buffer.clone()],
                                 entry.vertex_group.index_buffer.clone(),
                                 (self.voxel_texture_descriptors.clone(), voxel_descriptor_sets[i].clone()),
                                 DeferredShadingShaders::vertex::ty::Constants {
                                     view: info.view_mat.into(),
                                     proj: info.proj_mat.into(),
                                 }).unwrap();
        }
        cb = cb.end_render_pass().unwrap();

        (cb.build().unwrap(), info.queue_main.clone())
    }

    fn recreate_framebuffers_if_none(&mut self, images: &Vec<Arc<SwapchainImage<Window>>>, info: &RenderInfo) {
        if self.get_framebuffers_mut().is_none() {
            let new_framebuffers = Some(images.iter().map(|_image| {
                let arc: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(self.get_renderpass().clone())
                    .add(info.position_buffer_image.clone()).unwrap()
                    .add(info.normal_buffer_image.clone()).unwrap()
                    .add(info.albedo_buffer_image.clone()).unwrap()
                    .add(info.roughness_buffer_image.clone()).unwrap()
                    .add(info.metallic_buffer_image.clone()).unwrap()
                    .add(info.depth_buffer_image.clone()).unwrap()
                    .build().unwrap());
                arc
            }).collect::<Vec<_>>());
            ::std::mem::replace(self.get_framebuffers_mut(), new_framebuffers);
        }
    }
}
