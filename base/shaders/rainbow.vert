#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_colour;

out vec3 i_colour;

void main()
{
    gl_Position.xyz = in_pos;
    gl_Position.w = 1.0;
    i_colour = in_colour;
}