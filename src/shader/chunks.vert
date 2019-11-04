#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec3 color;

layout(location = 0) out vec3 normal_out;
layout(location = 1) out vec2 uv_out;
layout(location = 2) out vec3 color_out;
layout(location = 3) out vec3 surface_pos_out;

layout(set = 0, binding = 1) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
    vec3 view_pos;
    float specular_exponent;
    float specular_strength;
} uniforms;


void main() {
    normal_out = transpose(inverse(mat3(uniforms.world))) * normal; // normal in world space
    uv_out = uv;
    color_out = color;
    surface_pos_out = (uniforms.world * vec4(position, 1.0)).xyz;

    gl_Position = uniforms.proj * uniforms.view * uniforms.world * vec4(position, 1.0);
}
