#version 450

layout (input_attachment_index = 0, set = 0, binding = 0) uniform subpassInput inputColor;
layout(set = 0, binding = 1) uniform usampler2D occlusion_buffer;

layout (location = 0) out vec4 f_color;

layout(push_constant) uniform Constants {
    float exposure;
    uint debug_vis_mode;
    vec2 screen_dimensions;
} constants;

#include "debug_vis.inc"

vec3 tonemapWhitePreservingLuma(vec3 color) {
    float white = 3.0; // white point
    float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    float toneMappedLuma = luma * (1. + luma / (white*white)) / (1. + luma);
    return color * toneMappedLuma / luma;
}

void main() {
    //vec3 hdrColor =  exp( -1.0 / ( 2.72 * subpassLoad(inputColor).rgb + 0.15 ) );
    vec3 hdrColor = tonemapWhitePreservingLuma(subpassLoad(inputColor).rgb);
    vec3 exposure_adj = vec3(1.0) - exp(-hdrColor * constants.exposure);

    if (constants.debug_vis_mode != 0) {
        if (constants.debug_vis_mode == DEBUG_VISUALIZE_OCCLUSION_BUFFER) {
            vec2 uv = vec2(gl_FragCoord.x / constants.screen_dimensions[0],
                           gl_FragCoord.y / constants.screen_dimensions[1]);
            uvec4 occlusion_id = texture(occlusion_buffer, uv);
            uint full_id = occlusion_id[3] + occlusion_id[2] + occlusion_id[1] + occlusion_id[0];
            float occlusion_normalized = mod(full_id, 256) / 256.0;
            vec3 color = (exposure_adj * 0.333) + (vec3(occlusion_normalized) * 0.666);
            f_color = vec4(vec3(occlusion_normalized), 1.0);
        }
        else {
            // passthrough
            f_color = vec4(subpassLoad(inputColor).rgb, 1.0);
        }
    }
    else {
        f_color = vec4(exposure_adj, 1.0);
    }
}
