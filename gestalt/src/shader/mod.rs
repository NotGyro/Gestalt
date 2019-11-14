//! Shaders. Macro-generated with `vulkano-shaders`.

/// Shader for rendering line sets.
pub mod lines {
    pub mod vertex {
        vulkano_shaders::shader!{
            ty: "vertex",
            path: "src/shader/lines.vert"
        }
    }
    pub mod fragment {
        vulkano_shaders::shader!{
            ty: "fragment",
            path: "src/shader/lines.frag"
        }
    }
}

/// Shader for rendering the skybox.
pub mod skybox {
    pub mod vertex {
        vulkano_shaders::shader!{
            ty: "vertex",
            path: "src/shader/skybox.vert"
        }
    }
    pub mod fragment {
        vulkano_shaders::shader!{
            ty: "fragment",
            path: "src/shader/skybox.frag"
        }
    }
}

/// Shader for rendering text.
pub mod text {
    pub mod vertex {
        vulkano_shaders::shader!{
            ty: "vertex",
            path: "src/shader/text.vert"
        }
    }
    pub mod fragment {
        vulkano_shaders::shader!{
            ty: "fragment",
            path: "src/shader/text.frag"
        }
    }
}

/// Pbr rendering pipeline shaders
pub mod pbr {
    pub mod vertex {
        vulkano_shaders::shader!{
            ty: "vertex",
            path: "src/shader/pbr.vert"
        }
    }
    pub mod fragment {
        vulkano_shaders::shader!{
            ty: "fragment",
            path: "src/shader/pbr.frag"
        }
    }
}

/// Tonemapping pass shaders
pub mod tonemapper {
    pub mod vertex {
        vulkano_shaders::shader!{
            ty: "vertex",
            path: "src/shader/tonemapper.vert"
        }
    }
    pub mod fragment {
        vulkano_shaders::shader!{
            ty: "fragment",
            path: "src/shader/tonemapper.frag"
        }
    }
}