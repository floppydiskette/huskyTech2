#version 330

in vec2 uv;
in vec3 normal;
in vec3 frag_pos;

out vec4 o_colour;

uniform sampler2D mixmap;
uniform sampler2D tex0; // r
uniform sampler2D tex1; // g
uniform sampler2D tex2; // b
uniform sampler2D tex3; // a

uniform float scale = 1;

uniform vec3 u_camera_pos;

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

vec3 calculate_light(Light light, float shiny, vec3 normal, vec3 frag_pos, vec3 view_dir) {
    vec3 light_dir = normalize(light.position - frag_pos);
    vec3 halfway_dir = normalize(light_dir + view_dir);

    float diff = max(dot(normal, light_dir), 0.0);

    vec3 reflect_dir = reflect(-light_dir, normal);

    float spec = pow(max(dot(normal, halfway_dir), 0.0), shiny);

    return light.intensity * (diff * light.colour + spec * light.colour);
}

// uses the mixmap to blend between the 4 textures
// applies scale to uv
void main() {
    float specular_strength = 0.5;

    // scale the uv
    vec2 scaled_uv = uv * scale;

    vec3 r = texture2D(tex0, scaled_uv).rgb;
    vec3 g = texture2D(tex1, scaled_uv).rgb;
    vec3 b = texture2D(tex2, scaled_uv).rgb;
    vec3 a = texture2D(tex3, scaled_uv).rgb;

    // use the mixmap to blend between the 4 textures
    vec4 mixmap = texture(mixmap, uv);

    r *= mixmap.r;
    g = mix(g, r, mixmap.g);
    b = mix(b, g, mixmap.b);
    a = mix(a, b, mixmap.a);


    vec3 view_dir = normalize(u_camera_pos - frag_pos);

    vec3 ambient = calculate_ambient(0.1, vec3(1.0, 1.0, 1.0));

    vec3 result = vec3(0);
    for (int i = 0; i < u_light_count; i++) {
        result += calculate_light(u_lights[i], 256.0, normal, frag_pos, view_dir);
    }

    // apply the lighting
    o_colour = vec4(a * (ambient + result), 1);
}