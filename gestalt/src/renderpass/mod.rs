//! Custom RenderPass types.

pub mod pbr_main;
pub use self::pbr_main::PBRMainRenderPass;

pub mod color_depth_uncleared;
pub use self::color_depth_uncleared::RenderPassUnclearedColorWithDepth;

pub mod color_depth_cleared;
pub use self::color_depth_cleared::RenderPassClearedColorWithDepth;

pub mod occlusion;
pub use self::occlusion::OcclusionRenderPass;
