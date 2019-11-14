#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;

layout(location = 0) out vec2 v_tex_coords;
layout(location = 1) out vec4 v_color;

void main() {
    gl_Position = vec4(position, 1.0);
    v_tex_coords = uv;
    v_color = color;
}
