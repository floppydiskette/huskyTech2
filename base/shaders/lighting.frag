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

    vec3 reflect_dir = reflect(-light_dir, normal);

    float shininess = specu * 4.0;
    float spec = pow(max(dot(normal, halfway_dir), 0.0), shininess);

    return light.intensity * (diff * light.colour + spec * light.colour);
}

void main() {
    vec3 frag_pos = texture(position, uv).rgb;
    vec3 normal = texture(normal, uv).rgb * 2.0 - 1.0;
    vec3 albedo = texture(albedospec, uv).rgb;

    vec3 info = texture(info, uv).rgb;
    float spec = info.r;
    float opacity = info.g;
    float unlit = info.b;


    vec3 view_dir = normalize(u_camera_pos - frag_pos);

    // calculate ambient
    vec3 ambient = calculate_ambient(0.1, vec3(1.0, 1.0, 1.0));

    // calculate lights (point lights)
    vec3 result = vec3(0.0, 0.0, 0.0);
    for (int i = 0; i < u_light_count; i++) {
        result += calculate_light(u_lights[i], albedo, spec, uv, normal, frag_pos, view_dir, ambient);
    }

    vec3 final_colour = (ambient + result) * albedo;

    if (unlit > 0.5) {
        FragColor = vec4(albedo, opacity);
    } else {
        FragColor = vec4(final_colour, opacity);
    }
}