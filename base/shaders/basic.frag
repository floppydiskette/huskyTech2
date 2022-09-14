#version 330 core

in vec2 uv;
in vec3 normal;
in vec3 frag_pos;

out vec4 o_colour;

uniform vec3 u_camera_pos;

uniform float u_opacity = 1.0;

struct Material {
    sampler2D diffuse;
    sampler2D roughness;
    sampler2D metallic;
    sampler2D normal;
};

uniform Material u_material;

// point light
struct Light {
    vec3 position;
    vec3 colour;
    float intensity;
};

#define MAX_LIGHTS 100

uniform Light u_lights[MAX_LIGHTS];
uniform int u_light_count;

vec3 calculate_ambient(float strength, vec3 colour) {
    return strength * colour;
}

vec3 calculate_light(Light light, Material material, vec2 uv, vec3 normal, vec3 frag_pos, vec3 view_dir) {
    vec3 light_dir = normalize(light.position - frag_pos);
    vec3 halfway_dir = normalize(light_dir + view_dir);

    float diff = max(dot(normal, light_dir), 0.0);

    vec3 reflect_dir = reflect(-light_dir, normal);

    float shininess = texture(material.roughness, uv).r * 256.0;
    float spec = pow(max(dot(normal, halfway_dir), 0.0), shininess);

    return light.intensity * (diff * light.colour + spec * light.colour);
}

void main() {
    float specular_strength = 0.5;

    vec3 norm = normalize(normal);

    vec3 view_dir = normalize(u_camera_pos - frag_pos);

    // calculate ambient
    vec3 ambient = calculate_ambient(0.1, vec3(1.0, 1.0, 1.0));

    // calculate lights (point lights)
    vec3 result = vec3(0.0, 0.0, 0.0);
    for (int i = 0; i < u_light_count; i++) {
        result += calculate_light(u_lights[i], u_material, uv, norm, frag_pos, view_dir);
    }

    vec3 colour = texture(u_material.diffuse, uv).rgb;
    vec3 metallic = texture(u_material.metallic, uv).rgb;
    vec3 roughness = texture(u_material.roughness, uv).rgb;
    vec3 normal = texture(u_material.normal, uv).rgb;

    vec3 final_colour = (ambient + result) * colour;

    o_colour = vec4(final_colour, u_opacity);
}