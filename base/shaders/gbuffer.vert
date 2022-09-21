#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;
layout(location = 5) in ivec4 in_bone_ids;
layout(location = 6) in vec4 in_bone_weights;

out vec2 uv;
out vec3 normal;
out vec3 frag_pos;

uniform mat4 u_mvp;
uniform mat4 u_model;

const int MAX_BONES = 100;
const int MAX_BONER_INFLUENCE = 4; // (:
uniform mat4 bone_matrices[MAX_BONES];
uniform bool care_about_animation;

void main()
{
    vec4 position_multiplier = vec4(0.0);

    if (care_about_animation) {
        for (int i = 0; i < MAX_BONER_INFLUENCE; i++) {
            if (in_bone_ids[i] == -1) { continue; }
            if (in_bone_ids[i] >= MAX_BONES) {
                position_multiplier = vec4(in_pos, 1.0);
                break;
            }
            vec4 local_pos = bone_matrices[in_bone_ids[i]] * vec4(pos, 1.0);
            position_multiplier += local_pos * in_bone_weights[i];
        }
    } else {
        position_multiplier = vec4(in_pos, 1.0);
    }

    gl_Position = u_mvp * vec4(in_pos, 1.0);
    frag_pos = vec3(u_model * vec4(in_pos, 1.0)) * position_multiplier;

    uv = in_uv;
    normal = in_normal;
}