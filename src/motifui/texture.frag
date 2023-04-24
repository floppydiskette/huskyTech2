#version 330 core

layout(location = 1) in vec2 in_uv;

uniform sampler2D u_texture;

void main() {
    gl_FragColor = texture(u_texture, in_uv);
}