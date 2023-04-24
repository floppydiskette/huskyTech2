#version 330 core

layout(location = 0) in vec3 in_pos;

uniform vec2 u_screen_size;
uniform vec2 u_xy;
uniform vec2 u_wh;
uniform float u_rotation_euler;

void main() {
    vec2 pos = in_pos.xy * u_wh;
    pos = vec2(
        pos.x * cos(u_rotation_euler) - pos.y * sin(u_rotation_euler),
        pos.x * sin(u_rotation_euler) + pos.y * cos(u_rotation_euler)
    );
    pos += u_xy;
    pos /= u_screen_size;
    pos = pos * 2.0 - 1.0;
    gl_Position = vec4(pos, 0.0, 1.0);
}