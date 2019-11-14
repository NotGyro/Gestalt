#version 450

layout(location = 0) in vec3 ws_normal; // normal in world space
layout(location = 1) in vec3 tangent;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec3 surface_pos;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
    vec3 view_pos;
    float specular_exponent;
    float specular_strength;
} uniforms;
layout(set = 0, binding = 1) uniform sampler2D tex_albedo;
layout(set = 0, binding = 2) uniform sampler2D tex_normal;
layout(set = 0, binding = 3) uniform sampler2D tex_roughness;
layout(set = 0, binding = 4) uniform sampler2D tex_metal;

#include "bsdf.inc"

void main() {
    vec3 light_positions[3];
    vec3 light_colors[3];

    light_positions[0] = vec3(16.0, 26.0, 16.0);
    light_colors[0] = vec3(0.2, 0.4, 1.0) * 50.0;

    light_positions[1] = vec3(96.0, 14.0, 14.0);
    light_colors[1] = vec3(1.0, 0.7, 0.3) * 1000.0;

    light_positions[2] = vec3(64.0, 40.0, -64.0);
    light_colors[2] = vec3(1.0, 0.2, 0.4) * 500.0;

    vec3 ts_normal = texture(tex_normal, uv).xyz;
    // flip green channel
    ts_normal = vec3(ts_normal.x, -ts_normal.y, ts_normal.z);
    vec3 binormal = cross(ws_normal, tangent);
    vec3 N = normalize(tangent * ts_normal.x + binormal * ts_normal.y + ws_normal * ts_normal.z);
    vec3 V = normalize(uniforms.view_pos - surface_pos);

    vec3 albedo = texture(tex_albedo, uv).xyz;
    float roughness = 0.2;//texture(tex_roughness, uv).x;
    float metallic = texture(tex_metal, uv).x;

    // summing irradiance for all lights
    vec3 Lo = vec3(0.0);
    for(int i = 0; i < 3; ++i) {
        vec3 L = normalize(light_positions[i] - surface_pos);
        vec3 H = normalize(V + L);

        float distance = length(light_positions[i] - surface_pos);
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
        //Lo += specular * radiance * NdotL;
    }

    vec3 ambient = vec3(0.03) * albedo; // * ao;
    vec3 color = ambient + Lo;

    f_color = vec4(color, 1.0);
}
