#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;
layout(location = 5) in vec4 a_joint;
layout(location = 6) in vec4 a_weight;

out vec2 uv;
out vec3 normal;
out vec3 frag_pos;

uniform mat4 u_mvp;
uniform mat4 u_model;

const int MAX_BONES = 100;
const int MAX_BONER_INFLUENCE = 4; // (:
uniform mat4 joint_matrix[MAX_BONES];
uniform bool care_about_animation;

void main()
{
    if (care_about_animation) {
        mat4 skin_matrix =  a_weight.x * joint_matrix[int(a_joint.x)] +
                            a_weight.y * joint_matrix[int(a_joint.y)] +
                            a_weight.z * joint_matrix[int(a_joint.z)] +
                            a_weight.w * joint_matrix[int(a_joint.w)];

        gl_Position = u_mvp * skin_matrix * vec4(in_pos, 1);
        frag_pos = vec3(u_model * skin_matrix * vec4(in_pos, 1));
    } else {
        gl_Position = u_mvp * vec4(in_pos, 1);
        frag_pos = vec3(u_model * vec4(in_pos, 1));
    }

    uv = in_uv;
    normal = in_normal;
}