#version 450

layout(location = 0) in vec3 ws_normal;
layout(location = 1) in vec3 tangent;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec3 pos;

layout(location = 0) out vec4 gbuffer_position;
layout(location = 1) out vec4 gbuffer_normal;
layout(location = 2) out vec4 gbuffer_albedo;
layout(location = 3) out vec4 gbuffer_roughness;
layout(location = 4) out vec4 gbuffer_metallic;

layout(set = 0, binding = 0) uniform sampler2D tex_albedo;
layout(set = 0, binding = 1) uniform sampler2D tex_normal;
layout(set = 0, binding = 2) uniform sampler2D tex_roughness;
layout(set = 0, binding = 3) uniform sampler2D tex_metal;

layout(push_constant) uniform Constants {
    mat4 view;
    mat4 proj;
} constants;

layout(set = 1, binding = 0) uniform InstanceData {
    mat4 world;
} instancedata;

const float NEAR_PLANE = 0.1f;
const float FAR_PLANE = 1000.0f;

float linearDepth(float depth) {
    float z = depth * 2.0f - 1.0f;
    return (2.0f * NEAR_PLANE * FAR_PLANE) / (FAR_PLANE + NEAR_PLANE - z * (FAR_PLANE - NEAR_PLANE));
}

void main() {
    gbuffer_position = vec4(pos, linearDepth(gl_FragCoord.z));

    vec3 ts_normal = texture(tex_normal, uv).xyz;
    // flip green channel
    ts_normal = vec3(ts_normal.x, -ts_normal.y, ts_normal.z);
    vec3 binormal = cross(ws_normal, tangent);
    gbuffer_normal = vec4(normalize(tangent * ts_normal.x + binormal * ts_normal.y + ws_normal * ts_normal.z), 1.0);

    gbuffer_albedo = texture(tex_albedo, uv);
    gbuffer_roughness = vec4(0.2);//texture(tex_roughness, uv).x;
    gbuffer_metallic = vec4(texture(tex_metal, uv).x);
}
