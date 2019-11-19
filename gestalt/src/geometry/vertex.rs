//! Vertex types.


/// A vertex type with position data.
#[derive(Debug, Clone, Default)]
pub struct VertexPosition {
    pub position: [f32; 3]
}
impl_vertex!(VertexPosition, position);


/// A vertex type with position and uv data.
#[derive(Debug, Clone, Default)]
pub struct VertexPositionUV {
    pub position: [f32; 3],
    pub uv: [f32; 2]
}
impl_vertex!(VertexPositionUV, position, uv);


/// Vertex type for pbr pipeline: position, normal, tangent, and uv data.
///
/// Used in DeferredRenderPipeline
#[derive(Debug, Clone, Default)]
pub struct DeferredShadingVertex {
    pub position:  [f32; 3],
    pub normal:    [f32; 3],
    pub tangent:   [f32; 3],
    pub uv:        [f32; 2]
}
impl_vertex!(DeferredShadingVertex, position, normal, tangent, uv);


/// A vertex type with position and color + alpha data.
///
/// Used in LinesRenderPipeline
#[derive(Debug, Clone, Default)]
pub struct VertexPositionColorAlpha {
    pub position: [f32; 3],
    pub color:    [f32; 4]
}
impl_vertex!(VertexPositionColorAlpha, position, color);


/// A vertex type with position, uv, and color data.
///
/// Used in TextRenderPipeline
#[derive(Debug, Clone, Default)]
pub struct VertexPositionUVColor {
    pub position: [f32; 3],
    pub uv:       [f32; 2],
    pub color:    [f32; 4]
}
impl_vertex!(VertexPositionUVColor, position, uv, color);


/// A vertex type with position and object ID data.
///
/// Used in OcclusionRenderPipeline
#[derive(Debug, Clone, Default)]
pub struct VertexPositionObjectId {
    pub position: [f32; 3],
    pub id:      u32,
}
impl_vertex!(VertexPositionObjectId, position, id);