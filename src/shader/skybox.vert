#version 450

#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable

layout (location = 0) in vec3 position;
layout (location = 1) in vec2 uv;

layout (binding = 0) uniform Data {
	mat4 projection;
	mat4 view;
} uniforms;

layout (location = 0) out vec2 uv_out;

out gl_PerVertex {
	vec4 gl_Position;
};

void main() {
	uv_out = uv;
	gl_Position = uniforms.projection * uniforms.view * vec4(position.xyz, 1.0);
}