#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in uint id;

layout(location = 0) out flat uint id_out;

layout(push_constant) uniform Constants {
    mat4 view;
    mat4 proj;
} constants;


void main() {
    id_out = id;
    gl_Position = constants.proj * constants.view * vec4(position, 1.0);
}
