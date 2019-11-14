//! Custom RenderPass types.


use vulkano::framebuffer::{RenderPassDesc, AttachmentDescription, PassDescription, PassDependencyDescription, LoadOp, StoreOp, RenderPassDescClearValues};
use vulkano::image::ImageLayout;
use vulkano::format::{Format, ClearValue};


/// Render pass that uses a single color attachment and a depth buffer, and clears both before it runs.
pub struct RenderPassClearedColorWithDepth {
    pub color_format: Format
}


unsafe impl RenderPassDesc for RenderPassClearedColorWithDepth {
    fn num_attachments(&self) -> usize { 2 }
    fn attachment_desc(&self, num: usize) -> Option<AttachmentDescription> {
        match num {
            0 => Some(AttachmentDescription {
                format: self.color_format,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
            1 => Some(AttachmentDescription {
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
    fn dependency_desc(&self, _num: usize) -> Option<PassDependencyDescription> { None }
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
    fn attachment_desc(&self, num: usize) -> Option<AttachmentDescription> {
        match num {
            0 => Some(AttachmentDescription {
                format: self.color_format,
                samples: 1,
                load: LoadOp::Load,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
            1 => Some(AttachmentDescription {
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
    fn subpass_desc(&self, num: usize) -> Option<PassDescription> {
        match num {
            0 => Some(PassDescription {
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
    fn dependency_desc(&self, _num: usize) -> Option<PassDependencyDescription> { None }
}


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for RenderPassUnclearedColorWithDepth {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}


/// Render pass that uses a color attachment and a depth buffer, plus a color input attachment,
/// and does not clear any of them before it runs.
pub struct PBRMainRenderPass { }

const HDR_BUFFER:      usize = 0;
const SWAPCHAIN_IMAGE: usize = 1;
const DEPTH_BUFFER:    usize = 2;

unsafe impl RenderPassDesc for PBRMainRenderPass {
    fn num_attachments(&self) -> usize { 3 }
    fn attachment_desc(&self, num: usize) -> Option<AttachmentDescription> {
        match num {
            // HDR float buffer
            HDR_BUFFER => Some(AttachmentDescription {
                format: Format::R16G16B16A16Sfloat,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::DontCare,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal
            }),
            // LDR clamped output to swapchain
            SWAPCHAIN_IMAGE => Some(AttachmentDescription {
                format: Format::B8G8R8A8Srgb,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::DontCare,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::PresentSrc
            }),
            // depth buffer
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

    fn num_subpasses(&self) -> usize { 3 }
    fn subpass_desc(&self, num: usize) -> Option<PassDescription> {
        match num {
            // skybox
            0 => Some(PassDescription {
                color_attachments: vec![ (HDR_BUFFER, ImageLayout::ColorAttachmentOptimal) ],
                depth_stencil: Some((DEPTH_BUFFER, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            // HDR rendering
            1 => Some(PassDescription {
                color_attachments: vec![ (HDR_BUFFER, ImageLayout::ColorAttachmentOptimal) ],
                depth_stencil: Some((DEPTH_BUFFER, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            // tonemapping
            2 => Some(PassDescription {
                color_attachments: vec![ (SWAPCHAIN_IMAGE, ImageLayout::ColorAttachmentOptimal) ],
                depth_stencil: None,
                input_attachments: vec![ (HDR_BUFFER, ImageLayout::ColorAttachmentOptimal) ],
                resolve_attachments: vec![],
                preserve_attachments: vec![]
            }),
            _ => None
        }
    }

    fn num_dependencies(&self) -> usize { 0 }
    fn dependency_desc(&self, _num: usize) -> Option<PassDependencyDescription> { None }
}


unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for PBRMainRenderPass {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<dyn Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}