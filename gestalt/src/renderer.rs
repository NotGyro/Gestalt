//! Main renderer.

use std::sync::{Arc, RwLock};

use cgmath::{EuclideanSpace, Matrix4, Vector4, SquareMatrix, Deg};

use vulkano::buffer::BufferUsage;
use vulkano::device::{Device, DeviceExtensions, Queue};
use vulkano::format::{D32Sfloat, R16G16B16A16Sfloat, R32Uint};
use vulkano::image::attachment::AttachmentImage;
use vulkano::image::swapchain::SwapchainImage;
use vulkano::instance::{Instance, PhysicalDevice};
use vulkano::swapchain::{Swapchain, Surface, SwapchainCreationError};
use vulkano::sync::GpuFuture;
use winit::Window;
use vulkano::command_buffer::AutoCommandBuffer;
use vulkano::image::ImageUsage;

use crate::util::{Camera, Transform};
use crate::geometry::{VertexGroup, Material, VertexPositionObjectId, DeferredShadingVertex};
use crate::registry::TextureRegistry;
use crate::memory::xalloc::XallocMemoryPool;
use crate::pipeline::{RenderPipelineAbstract, DeferredShadingRenderPipeline, DeferredLightingRenderPipeline, LinesRenderPipeline, TextRenderPipeline, OcclusionRenderPipeline, PostProcessRenderPipeline};
use crate::buffer::CpuAccessibleBufferXalloc;
use crate::geometry::VertexPositionColorAlpha;
use crate::pipeline::text::TextData;
use crate::metrics::FrameMetrics;
use crate::pipeline::occlusion::OCCLUSION_FRAME_SIZE;


/// Matrix to correct vulkan clipping planes and flip y axis.
/// See [https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/](https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/).
pub static VULKAN_CORRECT_CLIP: Matrix4<f32> = Matrix4 {
    x: Vector4 { x: 1.0, y:  0.0, z: 0.0, w: 0.0 },
    y: Vector4 { x: 0.0, y: -1.0, z: 0.0, w: 0.0 },
    z: Vector4 { x: 0.0, y:  0.0, z: 0.5, w: 0.5 },
    w: Vector4 { x: 0.0, y:  0.0, z: 0.0, w: 1.0 }
};


pub const DEBUG_VISUALIZE_DISABLED: u32 = 0;
pub const DEBUG_VISUALIZE_POSITION_BUFFER: u32 = 1;
pub const DEBUG_VISUALIZE_NORMAL_BUFFER: u32 = 2;
pub const DEBUG_VISUALIZE_ALBEDO_BUFFER: u32 = 3;
pub const DEBUG_VISUALIZE_ROUGHNESS_BUFFER: u32 = 4;
pub const DEBUG_VISUALIZE_METALLIC_BUFFER: u32 = 5;
pub const DEBUG_VISUALIZE_DEFERRED_LIGHTING_ONLY: u32 = 6;
pub const DEBUG_VISUALIZE_NO_POST_PROCESSING: u32 = 7;
pub const DEBUG_VISUALIZE_OCCLUSION_BUFFER: u32 = 8;
pub const DEBUG_VISUALIZE_MAX: u32 = 9;


#[derive(Clone)]
pub struct RenderInfo {
    /// Vulkan device.
    pub device: Arc<Device>,
    /// Memory pool for memory-managed objects.
    pub memory_pool: XallocMemoryPool,

    pub queue_main: Arc<Queue>,
    pub queue_offscreen: Arc<Queue>,

    pub image_num: usize,
    pub dimensions: [u32; 2],
    pub camera_transform: Transform,
    pub view_mat: Matrix4<f32>,
    pub proj_mat: Matrix4<f32>,
    pub fov: Deg<f32>,

    pub tex_registry: Arc<TextureRegistry>,

    pub depth_buffer_image: Arc<AttachmentImage<D32Sfloat>>,
    pub position_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub normal_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub albedo_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub roughness_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub metallic_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub hdr_color_buffer_image: Arc<AttachmentImage<R16G16B16A16Sfloat>>,
    pub occlusion_buffer_image: Option<Arc<AttachmentImage<R32Uint>>>,

    pub render_queues: Arc<RwLock<RenderQueues>>,

    pub debug_visualize_setting: u32,
}


pub enum GestaltRenderPass {
    Occlusion        = 0,
    DeferredShading  = 1,
    DeferredLighting = 2,
    PostProcess      = 3,
    Lines            = 4,
    Text             = 5,
}


/// Queue of all objects to be drawn.
pub struct RenderQueues {
    pub occluders: OcclusionRenderQueue,
    pub meshes: Vec<MeshRenderQueueEntry>,
    pub lines: LineRenderQueue,
    pub text: Vec<TextData>,
}


/// Render queue entry for a single mesh
pub struct MeshRenderQueueEntry {
    pub vertex_group: Arc<VertexGroup<DeferredShadingVertex>>,
    pub material: Material,
    pub transform: Matrix4<f32>
}


/// Render queue for all lines to be drawn.
pub struct LineRenderQueue {
    pub chunk_lines_vg: Arc<VertexGroup<VertexPositionColorAlpha>>,
    pub chunks_changed: bool,
}

/// Render queue for the occlusion pass.
pub struct OcclusionRenderQueue {
    pub vertex_group: Arc<VertexGroup<VertexPositionObjectId>>,
    pub output_cpu_buffer: Arc<CpuAccessibleBufferXalloc<[u32]>>,
}


/// Main renderer.
pub struct Renderer {
    /// Vulkano surface.
    surface: Arc<Surface<Window>>,
    /// Vulkano swapchain.
    swapchain: Arc<Swapchain<Window>>,
    /// Swapchain images.
    images: Vec<Arc<SwapchainImage<Window>>>,
    /// If true, swapchain needs to be recreated.
    recreate_swapchain: bool,
    /// List of render pipelines.
    pipelines: Vec<Box<dyn RenderPipelineAbstract>>,
    /// Information required by render pipelines
    pub info: RenderInfo
}


impl Renderer {
    /// Creates a new `Renderer`.
    pub fn new(instance: Arc<Instance>, surface: Arc<Surface<Window>>) -> Renderer {
        let physical = PhysicalDevice::enumerate(&instance).next().expect("no device available");

        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            .. DeviceExtensions::none()
        };

        let family_graphics = physical.queue_families().find(|&q| q.supports_graphics() &&
            surface.is_supported(q).unwrap_or(false))
            .expect("couldn't find a graphical queue family (main)");
        let family_offscreen = physical.queue_families().find(|&q| q.supports_graphics())
            .expect("couldn't find a graphical queue family (offscreen)");

        let (device, mut queues) = Device::new(physical, physical.supported_features(),
                                               &device_ext,
                                               [(family_graphics, 0.9), (family_offscreen, 0.5)].iter().cloned())
            .expect("failed to create device");
        let queue_main = queues.next().unwrap();
        let queue_offscreen = queues.next().unwrap();

        let dimensions;
        let capabilities;
        let (swapchain, images) = {
            capabilities = surface.capabilities(physical.clone()).expect("failed to get surface capabilities");

            dimensions = [1024, 768];
            let usage = capabilities.supported_usage_flags;
            let alpha = capabilities.supported_composite_alpha.iter().next().unwrap();

            Swapchain::new(device.clone(), surface.clone(), capabilities.min_image_count,
                           vulkano::format::Format::B8G8R8A8Srgb, dimensions, 1, usage, &queue_main,
                           vulkano::swapchain::SurfaceTransform::Identity, alpha,
                           vulkano::swapchain::PresentMode::Fifo, true, None)
                .expect("failed to create swapchain")
        };

        let gbuffer_usage = ImageUsage {
            color_attachment: true,
            input_attachment: true,
            ..ImageUsage::none()
        };
        let position_buffer_image = AttachmentImage::with_usage(device.clone(), dimensions, R16G16B16A16Sfloat, gbuffer_usage.clone()).unwrap();
        let normal_buffer_image = AttachmentImage::with_usage(device.clone(), dimensions, R16G16B16A16Sfloat, gbuffer_usage.clone()).unwrap();
        let albedo_buffer_image = AttachmentImage::with_usage(device.clone(), dimensions, R16G16B16A16Sfloat, gbuffer_usage.clone()).unwrap();
        let roughness_buffer_image = AttachmentImage::with_usage(device.clone(), dimensions, R16G16B16A16Sfloat, gbuffer_usage.clone()).unwrap();
        let metallic_buffer_image = AttachmentImage::with_usage(device.clone(), dimensions, R16G16B16A16Sfloat, gbuffer_usage.clone()).unwrap();
        let hdr_color_buffer_image = AttachmentImage::with_usage(device.clone(), dimensions, R16G16B16A16Sfloat, gbuffer_usage.clone()).unwrap();
        let depth_buffer_image = AttachmentImage::transient(device.clone(), dimensions, D32Sfloat).unwrap();

        let mut tex_registry = TextureRegistry::new();
        tex_registry.load(queue_main.clone());
        let tex_registry = Arc::new(tex_registry);

        let memory_pool = XallocMemoryPool::new(device.clone());

        let chunk_lines_vg = Arc::new(VertexGroup::new(Vec::<VertexPositionColorAlpha>::new().iter().cloned(), Vec::new().iter().cloned(), 0, device.clone(), memory_pool.clone()));
        let occlusion_vg = Arc::new(VertexGroup::new(Vec::<VertexPositionObjectId>::new().iter().cloned(), Vec::new().iter().cloned(), 0, device.clone(), memory_pool.clone()));
        let occlusion_cpu_buffer = CpuAccessibleBufferXalloc::<[u32]>::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), vec![0u32; 320*240].iter().cloned()).expect("failed to create buffer");

        let mut info = RenderInfo {
            device,
            image_num: 0,
            dimensions: [1024, 768],
            camera_transform: Transform::identity(),
            view_mat: Matrix4::identity(),
            proj_mat: Matrix4::identity(),
            fov: Deg(45f32),
            tex_registry: tex_registry.clone(),
            queue_main,
            queue_offscreen,
            position_buffer_image,
            normal_buffer_image,
            albedo_buffer_image,
            roughness_buffer_image,
            metallic_buffer_image,
            hdr_color_buffer_image,
            render_queues: Arc::new(RwLock::new(RenderQueues {
                lines: LineRenderQueue {
                    chunk_lines_vg,
                    chunks_changed: false,
                },
                text: Vec::new(),
                occluders: OcclusionRenderQueue {
                    vertex_group: occlusion_vg,
                    output_cpu_buffer: occlusion_cpu_buffer
                },
                meshes: Vec::new()
            })),
            memory_pool,
            depth_buffer_image,
            debug_visualize_setting: DEBUG_VISUALIZE_DISABLED,
            occlusion_buffer_image: None,
        };

        let mut pipelines = Vec::<Box<dyn RenderPipelineAbstract>>::with_capacity(3);
        pipelines.insert(GestaltRenderPass::Occlusion as usize,        Box::new(OcclusionRenderPipeline::new(&mut info, OCCLUSION_FRAME_SIZE)));
        pipelines.insert(GestaltRenderPass::DeferredShading as usize,  Box::new(DeferredShadingRenderPipeline::new(&info)));
        pipelines.insert(GestaltRenderPass::DeferredLighting as usize,  Box::new(DeferredLightingRenderPipeline::new(&info)));
        pipelines.insert(GestaltRenderPass::PostProcess as usize,      Box::new(PostProcessRenderPipeline::new(&info)));
        pipelines.insert(GestaltRenderPass::Lines as usize,            Box::new(LinesRenderPipeline::new(&info)));
        pipelines.insert(GestaltRenderPass::Text as usize,             Box::new(TextRenderPipeline::new(&info)));

        Renderer {
            surface,
            swapchain,
            images,
            recreate_swapchain: false,
            pipelines,
            info
        }
    }


    /// Draw all objects in the render queue. Called every frame in the game loop.
    pub fn draw(&mut self, camera: &Camera, transform: Transform, frame_metrics: &mut FrameMetrics) {
        self.info.dimensions = match self.surface.window().get_inner_size() {
            Some(logical_size) => [logical_size.width as u32, logical_size.height as u32],
            None => [800, 600]
        };
        // minimizing window makes dimensions = [0, 0] which breaks swapchain creation.
        // skip draw loop until window is restored.
        if self.info.dimensions[0] < 1 || self.info.dimensions[1] < 1 { return; }

        self.info.view_mat = Matrix4::from(transform.rotation) * Matrix4::from_translation((transform.position * -1.0).to_vec());
        self.info.proj_mat = VULKAN_CORRECT_CLIP * cgmath::perspective(camera.fov, { self.info.dimensions[0] as f32 / self.info.dimensions[1] as f32 }, 0.1, 100.0);

        if self.recreate_swapchain {
            info!(Renderer, "Recreating swapchain");
            let (new_swapchain, new_images) = match self.swapchain.recreate_with_dimension(self.info.dimensions) {
                Ok(r) => r,
                Err(SwapchainCreationError::UnsupportedDimensions) => {
                    error!(Renderer, "SwapchainCreationError::UnsupportedDimensions");
                    return;
                },
                Err(err) => panic!("{:?}", err)
            };

            let gbuffer_usage = ImageUsage {
                color_attachment: true,
                input_attachment: true,
                ..ImageUsage::none()
            };

            std::mem::replace(&mut self.swapchain, new_swapchain);
            std::mem::replace(&mut self.images, new_images);

            let new_depth_buffer = AttachmentImage::transient(self.info.device.clone(), self.info.dimensions, D32Sfloat).unwrap();
            std::mem::replace(&mut self.info.depth_buffer_image, new_depth_buffer);
            let new_position_buffer  = AttachmentImage::with_usage(self.info.device.clone(), self.info.dimensions, R16G16B16A16Sfloat, gbuffer_usage).unwrap();
            std::mem::replace(&mut self.info.position_buffer_image, new_position_buffer);
            let new_normal_buffer    = AttachmentImage::with_usage(self.info.device.clone(), self.info.dimensions, R16G16B16A16Sfloat, gbuffer_usage).unwrap();
            std::mem::replace(&mut self.info.normal_buffer_image, new_normal_buffer);
            let new_albedo_buffer    = AttachmentImage::with_usage(self.info.device.clone(), self.info.dimensions, R16G16B16A16Sfloat, gbuffer_usage).unwrap();
            std::mem::replace(&mut self.info.albedo_buffer_image, new_albedo_buffer);
            let new_roughness_buffer = AttachmentImage::with_usage(self.info.device.clone(), self.info.dimensions, R16G16B16A16Sfloat, gbuffer_usage).unwrap();
            std::mem::replace(&mut self.info.roughness_buffer_image, new_roughness_buffer);
            let new_metallic_buffer  = AttachmentImage::with_usage(self.info.device.clone(), self.info.dimensions, R16G16B16A16Sfloat, gbuffer_usage).unwrap();
            std::mem::replace(&mut self.info.metallic_buffer_image, new_metallic_buffer);
            let new_hdr_buffer       = AttachmentImage::with_usage(self.info.device.clone(), self.info.dimensions, R16G16B16A16Sfloat, gbuffer_usage).unwrap();
            std::mem::replace(&mut self.info.hdr_color_buffer_image, new_hdr_buffer);

            for p in self.pipelines.iter_mut() {
                p.remove_framebuffers();
            }

            self.recreate_swapchain = false;
        }

        for p in self.pipelines.iter_mut() {
            p.recreate_framebuffers_if_none(&self.images, &self.info);
        }

        let (image_num, future) = match vulkano::swapchain::acquire_next_image(self.swapchain.clone(), None) {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                self.recreate_swapchain = true;
                warn!(Renderer, "AcquireError::OutOfDate");
                return
            },
            Err(err) => { fatal!(Renderer, "{:?}", err); }
        };
        self.info.image_num = image_num;

        self.info.fov = camera.fov.clone();
        self.info.camera_transform = transform.clone();

        let mut cbs = Vec::new();
//        for p in self.pipelines.iter_mut() {
//            cbs.push(p.build_command_buffer(&self.info));
//        }

        frame_metrics.end_draw();

        self.submit(cbs, Box::new(future));

        frame_metrics.end_gpu();
    }

    pub fn submit(&mut self, _cbs: Vec<(AutoCommandBuffer, Arc<Queue>)>, image_acq_fut: Box<dyn GpuFuture>) {
        let mut main_future:      Box<dyn GpuFuture> = Box::new(image_acq_fut);
        let occlusion_finished_future;

        let (cb, q) = self.pipelines[GestaltRenderPass::Occlusion as usize].build_command_buffer(&self.info);
        occlusion_finished_future = vulkano::sync::now(self.info.device.clone())
            .then_execute(q.clone(), cb).unwrap()
            .then_signal_semaphore_and_flush().unwrap();

        let (cb, q) = self.pipelines[GestaltRenderPass::DeferredShading as usize].build_command_buffer(&self.info);
        main_future = Box::new(main_future.then_execute(q.clone(), cb).unwrap());

        let (cb, q) = self.pipelines[GestaltRenderPass::DeferredLighting as usize].build_command_buffer(&self.info);
        main_future = Box::new(main_future.then_execute(q.clone(), cb).unwrap());

        let (cb, q) = self.pipelines[GestaltRenderPass::PostProcess as usize].build_command_buffer(&self.info);
        main_future = Box::new(main_future.join(occlusion_finished_future)
            .then_execute(q.clone(), cb).unwrap());

        let (cb, q) = self.pipelines[GestaltRenderPass::Lines as usize].build_command_buffer(&self.info);
        main_future = Box::new(main_future.then_execute(q.clone(), cb).unwrap());

        let (cb, q) = self.pipelines[GestaltRenderPass::Text as usize].build_command_buffer(&self.info);
        main_future = Box::new(main_future.then_execute(q.clone(), cb).unwrap());

        let final_main_future = main_future.then_swapchain_present(self.info.queue_main.clone(),
                                                                  self.swapchain.clone(),
                                                                  self.info.image_num)
                                                                  .then_signal_fence_and_flush();
        match final_main_future {
            Ok(mut f) => {
                // This wait is required when using NVIDIA or running on macOS. See https://github.com/vulkano-rs/vulkano/issues/1247
                f.wait(None).unwrap();
                f.cleanup_finished();
            }
            Err(::vulkano::sync::FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                return;
            }
            Err(e) => {
                error!(Renderer, "Error in submit(): {:?}", e);
            }
        }
    }
}
