use vulkano::framebuffer::{RenderPassDesc, AttachmentDescription, PassDescription, PassDependencyDescription, LoadOp, StoreOp, RenderPassDescClearValues};
use vulkano::image::ImageLayout;
use vulkano::format::{Format, ClearValue};

/// Render pass for the occlusion phase.
pub struct OcclusionRenderPass { }

const OUTPUT_BUFFER:   usize = 0;
const DEPTH_BUFFER:    usize = 1;

unsafe impl RenderPassDesc for OcclusionRenderPass {
    fn num_attachments(&self) -> usize { 2 }
    fn attachment_desc(&self, num: usize) -> Option<AttachmentDescription> {
        match num {
            OUTPUT_BUFFER => Some(AttachmentDescription {
                format: Format::R32Uint,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
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

    fn num_subpasses(&self) -> usize { 1 }
    fn subpass_desc(&self, num: usize) -> Option<PassDescription> {
        match num {
            0 => Some(PassDescription {
                color_attachments: vec![ (OUTPUT_BUFFER, ImageLayout::ColorAttachmentOptimal) ],
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


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for OcclusionRenderPass {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}