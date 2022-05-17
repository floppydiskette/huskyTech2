#version 330 core

layout(location = 0) in vec3 in_pos;

void main()
{
    gl_Position.xyz = in_pos;
    gl_Position.w = 1.0;
}