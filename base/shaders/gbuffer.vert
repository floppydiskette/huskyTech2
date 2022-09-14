#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;

out vec2 uv;
out vec3 normal;
out vec3 frag_pos;

uniform mat4 u_mvp;
uniform mat4 u_model;

void main()
{
    gl_Position = u_mvp * vec4(in_pos, 1.0);
    frag_pos = vec3(u_model * vec4(in_pos, 1.0));

    uv = in_uv;
    normal = in_normal;
}