#version 330 core
out vec4 FragColor;

in vec2 uv;

uniform sampler2D position;
uniform sampler2D normal;
uniform sampler2D albedospec;
uniform sampler2D info;

// point light
struct Light {
    vec3 position;
    vec3 colour;
    float intensity;
};

#define MAX_LIGHTS 100

uniform Light u_lights[MAX_LIGHTS];
uniform int u_light_count;

uniform vec3 u_camera_pos;

vec3 calculate_ambient(float strength, vec3 colour) {
    return strength * colour;
}

vec3 calculate_light(Light light, vec3 albedo, float specu, vec2 uv, vec3 normal, vec3 frag_pos, vec3 view_dir, vec3 ambient_colour) {
    vec3 light_dir = normalize(light.position - frag_pos);
    vec3 halfway_dir = normalize(light_dir + view_dir);

    float diff = max(dot(normal, light_dir), 0.0);

    float spec = pow(max(dot(normal, halfway_dir), 0.0), specu);

    float constant = 1.0;
    float linear = 0.09;
    float quadratic = 0.032;

    float distance = length(light.position - frag_pos);
    float attenuation = 1.0 / (constant + linear * distance + quadratic * (distance * distance));

    vec3 ambient = ambient_colour * albedo.rgb;
    vec3 diffuse = diff * light.colour * albedo.rgb;
    vec3 specular = spec * light.colour * vec3(spec);

    ambient *= attenuation;
    diffuse *= attenuation;
    specular *= attenuation;

    return light.intensity * (ambient + diffuse + specular);
}

void main() {
    vec3 frag_pos = texture(position, uv).rgb;
    vec3 normal = texture(normal, uv).rgb;
    vec3 albedo = texture(albedospec, uv).rgb;

    vec3 info = texture(info, uv).rgb;
    float spec = info.r;
    float opacity = info.g;
    float unlit = info.b;

    vec3 norm = normalize(normal * 2.0 - 1.0);

    vec3 view_dir = normalize(u_camera_pos - frag_pos);

    // calculate ambient
    vec3 ambient = calculate_ambient(0.1, vec3(1.0, 1.0, 1.0));

    // calculate lights (point lights)
    vec3 result = vec3(0.0, 0.0, 0.0);
    for (int i = 0; i < u_light_count; i++) {
        result += calculate_light(u_lights[i], albedo, spec, uv, norm, frag_pos, view_dir, ambient);
    }

    if (unlit > 0.5) {
        FragColor = vec4(albedo, opacity);
    } else {
        FragColor = vec4(result, opacity);
    }
}