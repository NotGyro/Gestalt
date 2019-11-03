#version 450

layout(location = 0) in vec3 normal_world; // normal in world space
layout(location = 1) in vec2 uv;
layout(location = 2) in vec3 v_color;
layout(location = 3) in vec3 surface_pos;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D tex;

layout(set = 0, binding = 1) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
    vec3 view_pos;
    float specular_exponent;
    float specular_strength;
} uniforms;


vec3 hemisphere_light(vec3 normal, vec3 lightDirection, vec3 sky, vec3 ground) {
  float weight = 0.5 * dot(normalize(normal), lightDirection) + 0.5;
  return mix(ground, sky, weight);
}


// all parameters in world space
vec3 DirectionalLight(const in vec3 normal, const in vec3 light_dir, const in vec3 surface_pos) {
    vec3 view_dir = normalize(uniforms.view_pos - surface_pos);
    vec3 half_vec = normalize(light_dir + view_dir);
	float spec = pow(max(dot(normal, half_vec), 0.0), uniforms.specular_exponent);

    vec3 result = vec3(0.2); // ambient
	result += vec3(1.0) * max(0.0, dot(normal, normalize(light_dir))); // diffuse
	result += vec3(uniforms.specular_strength) * spec; // specular
	return result;
}

void main() {
    vec3 light_dir = normalize(vec3(0.4, 0.7, 1.0));

    vec3 lighting = DirectionalLight(normal_world, light_dir, surface_pos);

    f_color = vec4(lighting * texture(tex, uv).xyz * v_color, 1.0);
}
