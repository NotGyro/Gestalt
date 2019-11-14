#include "util.inc"

vec3 hemisphere_light(const in vec3 normal, const in vec3 lightDirection, const in vec3 sky, const in vec3 ground) {
    float weight = 0.5 * dot(normalize(normal), lightDirection) + 0.5;
    return mix(ground, sky, weight);
}

// all parameters in world space
vec3 DirectionalLight(const in vec3 normal,
                      const in vec3 light_dir,
                      const in vec3 view_pos,
                      const in vec3 surface_pos,
                      const in float intensity,
                      const in float specular_strength,
                      const in float specular_exponent) {

    vec3 view_dir = normalize(view_pos - surface_pos);
    vec3 half_vec = normalize(light_dir + view_dir);
    float spec = pow(max(dot(normal, half_vec), 0.0), specular_exponent);

    vec3 result = vec3(intensity * 0.2); // ambient
    result += vec3(intensity * 1.0) * max(0.0, dot(normal, normalize(light_dir))); // diffuse
    result += vec3(specular_strength) * spec; // specular
    return result;
}

vec3 PointLight(const in vec3 normal, const in vec3 light_color, const in vec3 light_pos, const in vec3 view_pos, const in vec3 surface_pos, const in float intensity, const in float specular_exponent) {
    vec3 light_dir = normalize(light_pos - surface_pos);
    vec3 view_dir = normalize(view_pos - surface_pos);
    vec3 halfway = normalize(light_dir + view_dir);

    float spec = pow(max(dot(normal, halfway), 0.0), specular_exponent);
    vec3 specular = light_color * spec;

    vec3 diffuse = light_color * max(dot(normal, light_dir), 0.0);

    float falloff = intensity / (length(light_pos - surface_pos) * length(light_pos - surface_pos));
    return (diffuse * falloff) + (specular * falloff);
}