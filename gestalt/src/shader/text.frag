#version 450

layout(set = 0, binding = 0) uniform sampler2D tex;
layout(location = 0) in vec2 v_tex_coords;
layout(location = 1) in vec4 v_color;
layout(location = 0) out vec4 f_color;

void main() {
    f_color = v_color * vec4(1.0, 1.0, 1.0, texture(tex, v_tex_coords).r);
}
