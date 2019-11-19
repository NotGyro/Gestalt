#version 450

layout (input_attachment_index = 0, binding = 0) uniform subpassInput gbufferPosition;
layout (input_attachment_index = 1, binding = 1) uniform subpassInput gbufferNormal;
layout (input_attachment_index = 2, binding = 2) uniform subpassInput gbufferAlbedo;
layout (input_attachment_index = 3, binding = 3) uniform subpassInput gbufferRoughness;
layout (input_attachment_index = 4, binding = 4) uniform subpassInput gbufferMetallic;

layout(location = 0) out vec4 f_color;

layout(push_constant) uniform Constants {
    mat4 view;
    vec3 view_pos;
    uint debug_vis_mode;
} constants;

#include "bsdf.inc"
#include "debug_vis.inc"

void main() {
    vec3 light_positions[3];
    vec3 light_colors[3];

    light_positions[0] = vec3(16.0, 26.0, 16.0);
    light_colors[0] = vec3(0.2, 0.4, 1.0) * 50.0;

    light_positions[1] = vec3(96.0, 14.0, 14.0);
    light_colors[1] = vec3(1.0, 0.7, 0.3) * 1000.0;

    light_positions[2] = vec3(64.0, 40.0, -64.0);
    light_colors[2] = vec3(1.0, 0.2, 0.4) * 500.0;

    vec3 frag_pos = subpassLoad(gbufferPosition).rgb;
    vec3 N = subpassLoad(gbufferNormal).rgb;
    vec3 V = normalize(constants.view_pos - frag_pos);
    vec3 albedo = subpassLoad(gbufferAlbedo).rgb;
    float roughness = subpassLoad(gbufferRoughness).r;
    float metallic = subpassLoad(gbufferMetallic).r;

    // summing irradiance for all lights
    vec3 Lo = vec3(0.0);
    for(int i = 0; i < 3; ++i) {
        vec3 L = normalize(light_positions[i] - frag_pos);
        vec3 H = normalize(V + L);

        float distance = length(light_positions[i] - frag_pos);
        float attenuation = 1.0 / (distance * distance);
        vec3 radiance = light_colors[i] * attenuation;

        // F: Cook-Torrance specular term
        vec3 F0 = vec3(0.04);
        F0 = mix(F0, albedo, metallic);
        vec3 F = fresnelSchlick(max(dot(H, V), 0.0), F0);

        // NDF: Normal distribution function (Trowbridge-Reitz GGX)
        float NDF = DistributionGGX(N, H, roughness);

        // G: Geometry function (Schlick-GGX)
        float G = GeometrySmith(N, V, L, roughness);

        // ratio of refraction
        vec3 kS = F;
        vec3 kD = vec3(1.0) - kS;
        // metallic surfaces don't refract
        kD *= 1.0 - metallic;

        // calculating final Cook-Torrance BRDF value
        vec3 numerator = NDF * G * F;
        float denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0);
        vec3 specular = numerator / max(denominator, 0.001);

        float NdotL = max(dot(N, L), 0.0);
        // final radiance value
        Lo += (kD * albedo / PI + specular) * radiance * NdotL;
    }

    vec3 ambient = vec3(0.03) * albedo; // * ao;
    vec3 color = ambient + Lo;

    if (constants.debug_vis_mode == DEBUG_VISUALIZE_POSITION_BUFFER) {
        f_color = vec4(frag_pos / 100.0, 1.0);
    }
    else if (constants.debug_vis_mode == DEBUG_VISUALIZE_NORMAL_BUFFER) {
        f_color = vec4(N, 1.0);
    }
    else if (constants.debug_vis_mode == DEBUG_VISUALIZE_ALBEDO_BUFFER) {
        f_color = vec4(albedo, 1.0);
    }
    else if (constants.debug_vis_mode == DEBUG_VISUALIZE_ROUGHNESS_BUFFER) {
        f_color = vec4(vec3(roughness), 1.0);
    }
    else if (constants.debug_vis_mode == DEBUG_VISUALIZE_METALLIC_BUFFER) {
        f_color = vec4(vec3(metallic), 1.0);
    }
    else if (constants.debug_vis_mode == DEBUG_VISUALIZE_DEFERRED_LIGHTING_ONLY) {
        f_color = vec4(Lo, 1.0);
    }
    else {
        f_color = vec4(color, 1.0);
    }
}
