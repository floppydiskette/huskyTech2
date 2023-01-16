#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;

out vec2 uv;
out vec3 normal;
out vec3 frag_pos;

uniform mat4 u_mvp;
uniform mat4 u_view;
uniform mat4 u_projection;
uniform mat4 u_model;

const int MAX_BONES = 100;
const int MAX_BONER_INFLUENCE = 4; // (:
uniform mat4 bone_matrices[MAX_BONES];
uniform bool care_about_animation;

void main()
{
    vec4 world_pos = u_model * vec4(in_pos, 1.0);
    frag_pos = world_pos.xyz;

    uv = in_uv;
    mat3 normal_mat = transpose(inverse(mat3(u_model)));
    normal = normal_mat * in_normal;
    gl_Position = u_projection * u_view * world_pos;
}