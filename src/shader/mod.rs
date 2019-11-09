//! Shaders. Macro-generated with `vulkano-shader-derive`.


/// Shader for rendering chunk meshes.
pub mod chunks {
    /// Vertex shader.
    #[allow(dead_code)]
    pub mod vertex {
        #[derive(VulkanoShader)]
        #[ty = "vertex"]
        #[path = "src/shader/chunks.vert"]
        struct Dummy;
    }

    /// Fragment shader.
    #[allow(dead_code)]
    pub mod fragment {
        #[derive(VulkanoShader)]
        #[ty = "fragment"]
        #[path = "src/shader/chunks.frag"]
        struct Dummy;
    }
}


/// Shader for rendering line sets.
pub mod lines {
    /// Vertex shader.
    #[allow(dead_code)]
    pub mod vertex {
        #[derive(VulkanoShader)]
        #[ty = "vertex"]
        #[path = "src/shader/lines.vert"]
        struct Dummy;
    }

    /// Fragment shader.
    #[allow(dead_code)]
    pub mod fragment {
        #[derive(VulkanoShader)]
        #[ty = "fragment"]
        #[path = "src/shader/lines.frag"]
        struct Dummy;
    }
}


/// Shader for rendering the skybox.
pub mod skybox {
    /// Vertex shader.
    #[allow(dead_code)]
    pub mod vertex {
        #[derive(VulkanoShader)]
        #[ty = "vertex"]
        #[path = "src/shader/skybox.vert"]
        struct Dummy;
    }

    /// Fragment shader.
    #[allow(dead_code)]
    pub mod fragment {
        #[derive(VulkanoShader)]
        #[ty = "fragment"]
        #[path = "src/shader/skybox.frag"]
        struct Dummy;
    }
}


/// Shader for rendering text.
pub mod text {
    /// Vertex shader.
    #[allow(dead_code)]
    pub mod vertex {
        #[derive(VulkanoShader)]
        #[ty = "vertex"]
        #[path = "src/shader/text.vert"]
        struct Dummy;
    }

    /// Fragment shader.
    #[allow(dead_code)]
    pub mod fragment {
        #[derive(VulkanoShader)]
        #[ty = "fragment"]
        #[path = "src/shader/text.frag"]
        struct Dummy;
    }
}
