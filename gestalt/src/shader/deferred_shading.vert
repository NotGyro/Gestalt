#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 tangent;
layout(location = 3) in vec2 uv;

layout(location = 0) out vec3 normal_out;
layout(location = 1) out vec3 tangent_out;
layout(location = 2) out vec2 uv_out;
layout(location = 3) out vec3 surface_pos_out;

layout(push_constant) uniform Constants {
    mat4 view;
    mat4 proj;
} constants;

layout(set = 1, binding = 0) uniform InstanceData {
    mat4 world;
} instancedata;


void main() {
    normal_out = transpose(inverse(mat3(instancedata.world))) * normal;
    tangent_out = tangent;
    uv_out = uv;
    surface_pos_out = (instancedata.world * vec4(position, 1.0)).xyz;

    gl_Position = constants.proj * constants.view * instancedata.world * vec4(position, 1.0);
}
