use vulkano::framebuffer::{RenderPassDesc, AttachmentDescription, PassDescription, PassDependencyDescription, LoadOp, StoreOp, RenderPassDescClearValues};
use vulkano::image::ImageLayout;
use vulkano::format::{Format, ClearValue};

pub struct DeferredShadingRenderPass { }

const POSITION_BUFFER:  usize = 0;
const NORMAL_BUFFER:    usize = 1;
const ALBEDO_BUFFER:    usize = 2;
const ROUGHNESS_BUFFER: usize = 3;
const METALLIC_BUFFER:  usize = 4;
const DEPTH_BUFFER:     usize = 5;

const FLOAT_ATTACHMENT_DESC: AttachmentDescription = AttachmentDescription {
    format: Format::R16G16B16A16Sfloat,
    samples: 1,
    load: LoadOp::Clear,
    store: StoreOp::Store,
    stencil_load: LoadOp::DontCare,
    stencil_store: StoreOp::DontCare,
    initial_layout: ImageLayout::Undefined,
    final_layout: ImageLayout::ColorAttachmentOptimal
};

unsafe impl RenderPassDesc for DeferredShadingRenderPass {
    fn num_attachments(&self) -> usize { 6 }
    fn attachment_desc(&self, num: usize) -> Option<AttachmentDescription> {
        match num {
            POSITION_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            NORMAL_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            ALBEDO_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            ROUGHNESS_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            METALLIC_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            DEPTH_BUFFER => Some(AttachmentDescription {
                format: Format::D32Sfloat,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::DepthStencilAttachmentOptimal
            }),
            _ => None
        }
    }

    fn num_subpasses(&self) -> usize { 2 }
    fn subpass_desc(&self, num: usize) -> Option<PassDescription> {
        match num {
            0 => Some(PassDescription {
                color_attachments: vec![
                    (ALBEDO_BUFFER, ImageLayout::ColorAttachmentOptimal),
                ],
                depth_stencil: Some((DEPTH_BUFFER, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            1 => Some(PassDescription {
                color_attachments: vec![
                    (POSITION_BUFFER, ImageLayout::ColorAttachmentOptimal),
                    (NORMAL_BUFFER, ImageLayout::ColorAttachmentOptimal),
                    (ALBEDO_BUFFER, ImageLayout::ColorAttachmentOptimal),
                    (ROUGHNESS_BUFFER, ImageLayout::ColorAttachmentOptimal),
                    (METALLIC_BUFFER, ImageLayout::ColorAttachmentOptimal),
                ],
                depth_stencil: Some((DEPTH_BUFFER, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            _ => None
        }
    }

    fn num_dependencies(&self) -> usize { 0 }
    fn dependency_desc(&self, _num: usize) -> Option<PassDependencyDescription> { None }
}


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for DeferredShadingRenderPass {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}