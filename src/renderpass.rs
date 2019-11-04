//! Custom RenderPass types.


use vulkano::framebuffer::{RenderPassDesc, LayoutAttachmentDescription, LayoutPassDescription, LayoutPassDependencyDescription, LoadOp, StoreOp, RenderPassDescClearValues};
use vulkano::image::ImageLayout;
use vulkano::format::{Format, ClearValue};


/// Render pass that uses a single color attachment and a depth buffer, and clears both before it runs.
pub struct RenderPassClearedColorWithDepth {
    pub color_format: Format
}


unsafe impl RenderPassDesc for RenderPassClearedColorWithDepth {
    fn num_attachments(&self) -> usize { 2 }
    fn attachment_desc(&self, num: usize) -> Option<LayoutAttachmentDescription> {
        match num {
            0 => Some(LayoutAttachmentDescription {
                format: self.color_format,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
            1 => Some(LayoutAttachmentDescription {
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
    fn subpass_desc(&self, num: usize) -> Option<LayoutPassDescription> {
        match num {
            0 => Some(LayoutPassDescription {
                color_attachments: vec![ (0, ImageLayout::ColorAttachmentOptimal) ],
                depth_stencil: Some((1, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            _ => None
        }
    }

    fn num_dependencies(&self) -> usize { 0 }
    fn dependency_desc(&self, _num: usize) -> Option<LayoutPassDependencyDescription> { None }
}


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for RenderPassClearedColorWithDepth {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}


/// Render pass that uses a single color attachment and a depth buffer, and does not clear them before it runs.
pub struct RenderPassUnclearedColorWithDepth {
    pub color_format: Format
}


unsafe impl RenderPassDesc for RenderPassUnclearedColorWithDepth {
    fn num_attachments(&self) -> usize { 2 }
    fn attachment_desc(&self, num: usize) -> Option<LayoutAttachmentDescription> {
        match num {
            0 => Some(LayoutAttachmentDescription {
                format: self.color_format,
                samples: 1,
                load: LoadOp::Load,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
            1 => Some(LayoutAttachmentDescription {
                format: Format::D32Sfloat,
                samples: 1,
                load: LoadOp::Load,
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
    fn subpass_desc(&self, num: usize) -> Option<LayoutPassDescription> {
        match num {
            0 => Some(LayoutPassDescription {
                color_attachments: vec![ (0, ImageLayout::ColorAttachmentOptimal) ],
                depth_stencil: Some((1, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            _ => None
        }
    }

    fn num_dependencies(&self) -> usize { 0 }
    fn dependency_desc(&self, _num: usize) -> Option<LayoutPassDependencyDescription> { None }
}


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for RenderPassUnclearedColorWithDepth {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}
