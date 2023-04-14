#version 330 core
out vec4 FragColor;

in vec2 uv;

uniform sampler2D position;
uniform sampler2D normal;
uniform sampler2D albedospec;
uniform sampler2D info;
uniform sampler2D info2;
uniform sampler2D shadow_depth_back;
uniform sampler2D shadow_depth_front;
uniform sampler2D shadow_mask;

uniform vec2 noise_scale;

uniform vec3 kernels[256];
const int kernel_count = 16;

uniform mat4 u_projection;
uniform mat4 u_view;

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

    float diff = clamp(dot(normal, light_dir), 0.0, 1.0);

    vec3 reflect_dir = reflect(light_dir, normal);

    float shininess = specu * 4.0;
    float spec = pow(max(dot(normal, halfway_dir), 0.0), shininess);

    return light.intensity * (diff * light.colour + spec * light.colour);
}

float rand(vec2 co){
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

// ssao
float ssao(vec2 uv, vec3 pos_w) {
    vec2 texel_size = vec2(textureSize(position, 0).xy);

    float result = 0.0;
    float radius = 0.5;
    float bias = 0.05;

    vec3 random_vec = vec3(rand(uv), rand(uv + vec2(1.0, 1.0)), rand(uv + vec2(2.0, 2.0)));

    vec3 P = pos_w;

    vec3 N = normalize(texture(normal, uv).rgb * 2.0 - 1.0);

    float rad = radius / (length(P - u_camera_pos));

    for (int i = 0; i < kernel_count; i++) {
        vec2 coord = reflect(kernels[i].xy, random_vec.xy) * rad;
        vec3 diff = texture(position, uv + coord).rgb - P;
        vec3 v = normalize(diff);
        float d = length(diff)*0.5;

        float ao = max(0.0, dot(N, v) - bias) * (1.0 - smoothstep(0.0, 1.0, d)) * 2.0;
        result += ao;
    }

    result = 1.0 - (result / kernel_count);
    return result;
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

    vec2 uv_seed = uv;

    float scene_depth = texture(info2, uv).r;

    float in_shadow = 1.0 - texture(shadow_depth_front, uv).r;

    vec3 final_colour = (ambient + (result * in_shadow)) * albedo * vec3(pow(ssao(uv, frag_pos), 2.0));

    if (unlit > 0.5) {
        FragColor = vec4(final_colour, 1.0);
    } else {
        FragColor = vec4(final_colour, 1.0);
    }
}