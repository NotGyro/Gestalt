#version 450

layout (input_attachment_index = 0, set = 0, binding = 1) uniform subpassInput inputColor;

layout (location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Data {
    float exposure;
} uniforms;

void main() {
    vec3 hdrColor = subpassLoad(inputColor).rgb * uniforms.exposure;
    vec3 mapped = vec3(1.0) - exp(-hdrColor * uniforms.exposure);
    f_color = vec4(mapped, 1.0);
    //f_color = vec4(subpassLoad(inputColor).rgb, 1.0);
}
