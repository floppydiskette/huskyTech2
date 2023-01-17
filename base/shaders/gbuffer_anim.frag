#version 330 core

layout (location = 0) out vec4 out_pos;
layout (location = 1) out vec4 out_normal;
layout (location = 2) out vec4 out_albedospec;
layout (location = 3) out vec4 out_info;

in vec2 uv;
in vec3 frag_pos;
in vec3 normal;

uniform sampler2D diffuse;
uniform sampler2D specular;
uniform sampler2D normalmap;

uniform float opacity = 1.0;
uniform bool unlit = false;

void main() {
    if (!unlit) {
        out_pos = vec4(frag_pos, opacity);
        out_normal = vec4(normalize(normal), 1.0);//vec4(texture(normalmap, uv).rgb, 1.0);
        out_albedospec = vec4(texture(diffuse, uv).rgb, 1.0);
        out_info = vec4(texture(specular, uv).r, opacity, unlit ? 1.0 : 0.0, 1.0);
    } else {
        out_normal = vec4(normal, 1.0);
        out_albedospec = vec4(texture(diffuse, uv).rgb, 1.0);
        out_info = vec4(0.0, opacity, unlit ? 1.0 : 0.0, 1.0);
    }
}