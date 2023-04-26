// Vertex shader
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(1) @binding(0)
var<uniform> camera: CameraUniform;

// Vertex shader
struct ModelPush {
    model: mat4x4<f32>,
};
var<push_constant> model_matrix: ModelPush;

struct VertexInput {
    @location(0) @interpolate(flat) vertex_data: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) tex_idx: i32,
}

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    var bitmask_6 = u32(63); //AND with this to get lowest six bits of data.
	//Extract X:
	var x = f32(vertex.vertex_data & bitmask_6);
	//Extract Y:
	var y = f32((vertex.vertex_data >> u32(6)) & bitmask_6);
	//Extract Z:
	var z = f32((vertex.vertex_data >> u32(12)) & bitmask_6);

    var vertex_position = vec3<f32>(x, y, z);
    out.clip_position = camera.view_proj * model_matrix.model * vec4<f32>(vertex_position, 1.0);

	//Extract texture ID
	var bitmask_12 = u32(4095);
	var t_id = i32((vertex.vertex_data >> u32(18)) & bitmask_12);
    out.tex_idx = t_id;
	
	var bitmask_1 = u32(1); //Lowest one bit time.
	var u = f32((vertex.vertex_data >> u32(30)) & bitmask_1);
	var v = f32((vertex.vertex_data >> u32(31)) & bitmask_1);
    out.tex_coords = vec2<f32>(u, v);

    return out;
}

// Fragment shader
@group(0) @binding(0)
var t_diffuse: texture_2d_array<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // TODO: This is most likely a bug in WGPU -
    // u32 is supposed to be supported as a texture index but currently
    // it throws a shader compilation error, so we convert it to an
    // i32 instead.
    var tex_idx: i32 = i32(in.tex_idx);
    return textureSample(t_diffuse, s_diffuse, in.tex_coords, tex_idx);
}
