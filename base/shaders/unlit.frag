#version 330 core

in vec2 uv;

out vec4 o_colour;

uniform sampler2D u_texture_a;

uniform float u_opacity = 1.0;

void main() {
    o_colour = texture(u_texture_a, uv);
    o_colour.a = u_opacity;
}