#version 330 core

layout(location = 0) in vec3 in_pos;

uniform mat4 u_mvp;

void main()
{
    gl_Position = u_mvp * vec4(in_pos, 1.0);
}