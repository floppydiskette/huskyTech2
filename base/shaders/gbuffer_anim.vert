#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;
layout(location = 5) in ivec4 a_joint;
layout(location = 6) in vec4 a_weight;

out vec2 uv;
out vec3 normal;
out vec3 frag_pos;

uniform mat4 u_mvp;
uniform mat4 u_view;
uniform mat4 u_projection;
uniform mat4 u_model;

const int MAX_BONES = 100;
const int MAX_BONER_INFLUENCE = 4; // (:
uniform mat4 joint_matrix[MAX_BONES];
uniform bool care_about_animation;

void main()
{
    if (care_about_animation) {
        vec4 total_position = vec4(0.0f);
        vec3 total_normal = vec3(0.0f);
        for (int i = 0; i < MAX_BONER_INFLUENCE; i++) {
            if (a_joint[i] >= MAX_BONES) {
                total_position = vec4(in_pos, 1.0f);
                total_normal = in_normal;
                 break;
            }
            vec4 local_pos = joint_matrix[a_joint[i]] * vec4(in_pos, 1.0f);
            total_position += local_pos * a_weight[i];
            vec3 local_normal = mat3(joint_matrix[a_joint[i]]) * in_normal;
            total_normal += local_normal * a_weight[i];
        }
        mat4 view_model = u_view * u_model;
        gl_Position = u_projection * view_model * total_position;
        frag_pos = vec3(u_model * total_position);
        mat3 normal_mat = transpose(inverse(mat3(u_model)));
        normal = normal_mat * in_normal;
    } else {
        gl_Position = u_projection * u_view * u_model * vec4(in_pos, 1.0);
        frag_pos = vec3(u_model * vec4(in_pos, 1.0));
        mat3 normal_mat = transpose(inverse(mat3(u_model)));
        normal = normal_mat * in_normal;
    }

    uv = in_uv;
}