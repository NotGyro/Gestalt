use std::sync::{Arc, RwLock};

use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::{AutoCommandBufferBuilder, AutoCommandBuffer, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::device::Device;
use vulkano::format::R8Unorm;
use vulkano::framebuffer::{FramebufferAbstract, RenderPass, RenderPassDesc, Subpass, RenderPassAbstract};
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::memory::pool::{PotentialDedicatedAllocation, StdMemoryPoolAlloc};
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, MipmapMode};
use vulkano::swapchain::Swapchain;
use winit::Window;
use rusttype::{Font, Scale, PositionedGlyph, point};
use hashbrown::HashMap;
use rusttype::gpu_cache::Cache;

use crate::renderpass::RenderPassUnclearedColorWithDepth;
use crate::geometry::vertex::VertexPositionUVColor;
use crate::renderer::RenderQueue;
use crate::shader::text as TextShaders;
use crate::pipeline::{RenderPipelineAbstract, PipelineCbCreateInfo};
use crate::buffer::CpuAccessibleBufferAutoPool;
use crate::memory::pool::AutoMemoryPool;


/// The size of each font's cache texture, in pixels (i.e. 512 x 512)
pub const CACHE_SIZE: usize = 512;


fn layout_paragraph<'a>(
    font: &'a Font,
    scale: Scale,
    width: u32,
    text: &str,
) -> Vec<PositionedGlyph<'a>> {
    let mut result = Vec::new();
    let v_metrics = font.v_metrics(scale);
    let advance_height = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;
    let mut caret = point(0.0, v_metrics.ascent);
    let mut last_glyph_id = None;
    for c in text.chars() {
        if c.is_control() {
            match c {
                '\r' => {
                    caret = point(0.0, caret.y + advance_height);
                }
                '\n' => {}
                _ => {}
            }
            continue;
        }
        let base_glyph = font.glyph(c);
        if let Some(id) = last_glyph_id.take() {
            caret.x += font.pair_kerning(scale, id, base_glyph.id());
        }
        last_glyph_id = Some(base_glyph.id());
        let mut glyph = base_glyph.scaled(scale).positioned(caret);
        if let Some(bb) = glyph.pixel_bounding_box() {
            if bb.max.x > width as i32 {
                caret = point(0.0, caret.y + advance_height);
                glyph.set_position(caret);
                last_glyph_id = None;
            }
        }
        caret.x += glyph.unpositioned().h_metrics().advance_width;
        result.push(glyph);
    }
    result
}


pub struct TextData {
    pub text: String,
    pub position: (i32, i32),
    pub size: f32,
    pub color: [f32; 4],
    pub family: String,
}
impl Default for TextData {
    fn default() -> Self {
        Self {
            text: "".to_string(),
            position: (0, 0),
            size: 24.0,
            color: [1.0, 1.0, 1.0, 1.0],
            family: "Roboto Regular".to_string(),
        }
    }
}


pub struct FontData {
    pub font: Box<Font<'static>>,
    pub cache: Box<Cache<'static>>,
    pub cache_buffer: Arc<CpuAccessibleBufferAutoPool<[u8]>>,
    pub cache_texture: Arc<AttachmentImage<R8Unorm, PotentialDedicatedAllocation<StdMemoryPoolAlloc>>>,
}


pub struct TextRenderPipeline {
    device: Arc<Device>,
    vulkan_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    pub framebuffers: Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>>,
    renderpass: Arc<RenderPass<RenderPassUnclearedColorWithDepth>>,
    fonts: HashMap<String, FontData>,
    sampler: Arc<Sampler>,
    memory_pool: AutoMemoryPool,
}


impl TextRenderPipeline {
    pub fn new(swapchain: &Swapchain<Window>, device: &Arc<Device>, memory_pool: &AutoMemoryPool) -> Self {
        let vs = TextShaders::vertex::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = TextShaders::fragment::Shader::load(device.clone()).expect("failed to create shader module");

        let renderpass = Arc::new(
            RenderPassUnclearedColorWithDepth { color_format: swapchain.format() }
                .build_render_pass(device.clone())
                .unwrap()
        );

        let pipeline = Arc::new(GraphicsPipeline::start()
            .vertex_input_single_buffer::<VertexPositionUVColor>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .blend_alpha_blending()
            .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap());

        let mut fonts = HashMap::new();
        fonts.insert("Roboto Regular".into(), FontData {
            font: Box::new(Font::from_bytes(include_bytes!("../../../fonts/Roboto-Regular.ttf") as & [u8]).unwrap()),
            cache: Box::new(Cache::builder().dimensions(CACHE_SIZE as u32, CACHE_SIZE as u32).build()),
            cache_buffer: CpuAccessibleBufferAutoPool::from_iter(device.clone(),
                                                         memory_pool.clone(),
                                                         BufferUsage::all(),
                                                         (0 .. CACHE_SIZE*CACHE_SIZE).map(|_| 0u8)
                                                    ).expect("failed to create buffer"),
            cache_texture: AttachmentImage::with_usage(
                device.clone(),
                [CACHE_SIZE as u32, CACHE_SIZE as u32],
                R8Unorm,
                ImageUsage {
                    transfer_destination: true,
                    sampled: true,
                    storage: true,
                    color_attachment: true,
                    input_attachment: true, // TODO: remove unncessary usages
                    .. ImageUsage::none()
                }).unwrap(),
        });

        TextRenderPipeline {
            device: device.clone(),
            vulkan_pipeline: pipeline,
            framebuffers: None,
            renderpass,
            fonts,
            sampler: Sampler::new(device.clone(), Filter::Nearest, Filter::Nearest, MipmapMode::Nearest,
                                  SamplerAddressMode::Repeat, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
                                  0.0, 4.0, 0.0, 0.0).unwrap(),
            memory_pool: memory_pool.clone(),
        }
    }
}


impl RenderPipelineAbstract for TextRenderPipeline {
    fn get_framebuffers_mut(&mut self) -> &mut Option<Vec<Arc<dyn FramebufferAbstract + Send + Sync>>> {
        &mut self.framebuffers
    }


    fn get_renderpass(&self) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        self.renderpass.clone() as Arc<dyn RenderPassAbstract + Send + Sync>
    }

    fn build_command_buffer(&mut self, info: PipelineCbCreateInfo, render_queue: Arc<RwLock<RenderQueue>>) -> AutoCommandBuffer {
        let lock = render_queue.read().unwrap();

        let mut cb = AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), info.queue.family())
            .unwrap();
        for text_data in (*lock).text.iter() {
            let font = self.fonts.get(&text_data.family).unwrap();
            cb = cb.copy_buffer_to_image(font.cache_buffer.clone(), font.cache_texture.clone()).unwrap()
        }
        cb = cb.begin_render_pass(
                self.framebuffers.as_ref().unwrap()[info.image_num].clone(), false,
                vec![::vulkano::format::ClearValue::None, ::vulkano::format::ClearValue::None]).unwrap();

        for text_data in (*lock).text.iter() {
            // TODO: this really doesn't need to be unsafe, but i give up on fighting the borrow checker right now
            unsafe {
                let font: *mut FontData = self.fonts.get_mut(&text_data.family).unwrap();

                (*font).cache.cache_queued(|rect, data| {
                    let x1 = rect.min.x as usize;
                    let x2 = x1 + rect.width() as usize;
                    let y1 = rect.min.y as usize;
                    let y2 = y1 + rect.height() as usize;

                    let mut lock = (*font).cache_buffer.write().unwrap();
                    let mut i = 0;
                    for y in y1..y2 {
                        for x in x1..x2 {
                            lock[y*(CACHE_SIZE as usize)+x] = data[i];
                            i += 1;
                        }
                    }
                }).unwrap();

                let glyphs = layout_paragraph(&(*font).font, Scale::uniform(text_data.size), 500, &text_data.text);
                for glyph in &glyphs {
                    (*font).cache.queue_glyph(0, glyph.clone());
                }

                let mut vertices = Vec::new();
                for g in glyphs.iter() {
                    if let Ok(Some((uv_rect, screen_rect))) = (*font).cache.rect_for(0, g) {
                        let scaled_x_min: f32 = (screen_rect.min.x + text_data.position.0) as f32 / info.dimensions[0] as f32 * 2.0 - 1.0;
                        let scaled_x_max: f32 = (screen_rect.max.x + text_data.position.0) as f32 / info.dimensions[0] as f32 * 2.0 - 1.0;
                        let scaled_y_min: f32 = (screen_rect.min.y + text_data.position.1) as f32 / info.dimensions[1] as f32 * 2.0 - 1.0;
                        let scaled_y_max: f32 = (screen_rect.max.y + text_data.position.1) as f32 / info.dimensions[1] as f32 * 2.0 - 1.0;
                        vertices.push(VertexPositionUVColor {
                            position: [scaled_x_min, scaled_y_max, 0.0],
                            uv: [uv_rect.min.x, uv_rect.max.y],
                            color: text_data.color,
                        });
                        vertices.push(VertexPositionUVColor {
                            position: [scaled_x_min, scaled_y_min, 0.0],
                            uv: [uv_rect.min.x, uv_rect.min.y],
                            color: text_data.color,
                        });
                        vertices.push(VertexPositionUVColor {
                            position: [scaled_x_max, scaled_y_min, 0.0],
                            uv: [uv_rect.max.x, uv_rect.min.y],
                            color: text_data.color,
                        });
                        vertices.push(VertexPositionUVColor {
                            position: [scaled_x_max, scaled_y_min, 0.0],
                            uv: [uv_rect.max.x, uv_rect.min.y],
                            color: text_data.color,
                        });
                        vertices.push(VertexPositionUVColor {
                            position: [scaled_x_max, scaled_y_max, 0.0],
                            uv: [uv_rect.max.x, uv_rect.max.y],
                            color: text_data.color,
                        });
                        vertices.push(VertexPositionUVColor {
                            position: [scaled_x_min, scaled_y_max, 0.0],
                            uv: [uv_rect.min.x, uv_rect.max.y],
                            color: text_data.color,
                        });
                    }
                }
                let vertex_buffer = CpuAccessibleBufferAutoPool::<[VertexPositionUVColor]>::from_iter(
                    self.device.clone(),
                    self.memory_pool.clone(),
                    BufferUsage::all(),
                    vertices.iter().cloned()
                ).unwrap();

                let descriptor_set = PersistentDescriptorSet::start(self.vulkan_pipeline.clone(), 0)
                    .add_sampled_image((*font).cache_texture.clone(), self.sampler.clone()).unwrap()
                    .build().unwrap();

                cb = cb.draw(self.vulkan_pipeline.clone(), &DynamicState {
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
                                     vec![vertex_buffer.clone()],
                                     descriptor_set, ()).unwrap();
            }
        }
        cb.end_render_pass().unwrap().build().unwrap()
    }
}
