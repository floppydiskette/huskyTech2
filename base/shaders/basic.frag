#version 330 core

in vec2 uv;

out vec4 o_colour;

uniform sampler2D u_texture;

void main() {
    o_colour = texture(u_texture, uv);
}