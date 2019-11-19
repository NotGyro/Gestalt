use vulkano::framebuffer::{RenderPassDesc, AttachmentDescription, PassDescription, PassDependencyDescription, LoadOp, StoreOp, RenderPassDescClearValues};
use vulkano::image::ImageLayout;
use vulkano::format::{Format, ClearValue};

pub struct DeferredLightingRenderPass { }

const POSITION_BUFFER:  usize = 0;
const NORMAL_BUFFER:    usize = 1;
const ALBEDO_BUFFER:    usize = 2;
const ROUGHNESS_BUFFER: usize = 3;
const METALLIC_BUFFER:  usize = 4;
const HDR_COLOR_BUFFER: usize = 5;

const FLOAT_ATTACHMENT_DESC: AttachmentDescription = AttachmentDescription {
    format: Format::R16G16B16A16Sfloat,
    samples: 1,
    load: LoadOp::Load,
    store: StoreOp::DontCare,
    stencil_load: LoadOp::DontCare,
    stencil_store: StoreOp::DontCare,
    initial_layout: ImageLayout::ShaderReadOnlyOptimal,
    final_layout: ImageLayout::ShaderReadOnlyOptimal
};

unsafe impl RenderPassDesc for DeferredLightingRenderPass {
    fn num_attachments(&self) -> usize { 6 }
    fn attachment_desc(&self, num: usize) -> Option<AttachmentDescription> {
        match num {
            POSITION_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            NORMAL_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            ALBEDO_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            ROUGHNESS_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            METALLIC_BUFFER => Some(FLOAT_ATTACHMENT_DESC),
            HDR_COLOR_BUFFER => Some(AttachmentDescription {
                format: Format::R16G16B16A16Sfloat,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::ColorAttachmentOptimal,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
            _ => None
        }
    }

    fn num_subpasses(&self) -> usize { 1 }
    fn subpass_desc(&self, num: usize) -> Option<PassDescription> {
        match num {
            0 =>  Some(PassDescription {
                color_attachments: vec![ (HDR_COLOR_BUFFER, ImageLayout::ColorAttachmentOptimal) ],
                depth_stencil: None,
                input_attachments: vec![
                    (POSITION_BUFFER, ImageLayout::ShaderReadOnlyOptimal),
                    (NORMAL_BUFFER, ImageLayout::ShaderReadOnlyOptimal),
                    (ALBEDO_BUFFER, ImageLayout::ShaderReadOnlyOptimal),
                    (ROUGHNESS_BUFFER, ImageLayout::ShaderReadOnlyOptimal),
                    (METALLIC_BUFFER, ImageLayout::ShaderReadOnlyOptimal),
                ],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            _ => None
        }
    }

    fn num_dependencies(&self) -> usize { 0 }
    fn dependency_desc(&self, _num: usize) -> Option<PassDependencyDescription> { None }
}


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for DeferredLightingRenderPass {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}