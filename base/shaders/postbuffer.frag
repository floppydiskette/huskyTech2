#version 330

in vec2 uv;

layout(location = 0) out vec4 o_colour;

uniform sampler2D u_texture;

void main() {
    o_colour = texture(u_texture, uv);
    // invert the colours!
    //o_colour = vec4(vec3(1.0 - texture(u_texture, uv)), 1.0);
}