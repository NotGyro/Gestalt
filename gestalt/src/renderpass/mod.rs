//! Custom RenderPass types.

pub mod deferred_shading;
pub use self::deferred_shading::DeferredShadingRenderPass;

pub mod deferred_lighting;
pub use self::deferred_lighting::DeferredLightingRenderPass;

pub mod lines;
pub use self::lines::LinesRenderPass;

pub mod color_depth_cleared;
pub use self::color_depth_cleared::RenderPassClearedColorWithDepth;

pub mod occlusion;
pub use self::occlusion::OcclusionRenderPass;

pub mod postprocess;
pub use self::postprocess::PostProcessRenderPass;
