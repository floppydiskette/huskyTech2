#version 330 core

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec3 in_normal;
layout(location = 7) in vec4 in_tangent;
layout(location = 5) in ivec4 a_joint;
layout(location = 6) in vec4 a_weight;

out vec2 uv;
out mat3 TBN;
out vec3 frag_pos;
out vec3 normal;

uniform mat4 u_mvp;
uniform mat4 u_view;
uniform mat4 u_projection;
uniform mat4 u_model;
uniform mat4 u_normal_matrix;

uniform vec3 light_pos; // position of the current light

const int MAX_BONES = 100;
const int MAX_BONER_INFLUENCE = 4; // (:
uniform mat4 joint_matrix[MAX_BONES];
uniform bool care_about_animation;

mat3 calculate_normals(vec3 in_normals) {
    mat3 normal_matrix = mat3(u_model);
    vec3 N = normalize(normal_matrix * in_normals);
    vec3 T = normalize(normal_matrix * in_tangent.xyz);
    vec3 B = cross(N, T);
    return mat3(T, B, N);
}

void main()
{
    float f = 1.0 / tan(radians(0.5 * 90));
    mat4 ipm = mat4(
        f  ,  0.0,   0.0,  0.0,
        0.0,  f  ,   0.0,  0.0,
        0.0,  0.0,  -1.0, -0.1,
        0.0,  0.0,  -1.0,  0.0
    );
    mat4 ipmr = mat4(
        f  ,  0.0,  0.0,  0.0,
        0.0,  f  ,  0.0,  0.0,
        0.0,  0.0,  0.0, 0.1,
        0.0,  0.0, -1.0,  0.0
    );

    mat4 view_model = u_view * u_model;
    vec4 total_position = vec4(0.0f);
    vec3 total_normal = vec3(0.0f);

    if (care_about_animation) {
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
        frag_pos = vec3(u_model * total_position);
        TBN = calculate_normals(total_normal);
    } else {
        total_position = vec4(in_pos, 1.0);
        total_normal = in_normal;
        frag_pos = vec3(u_model * total_position);
        TBN = calculate_normals(in_normal);
    }
    vec3 light_dir = normalize(light_pos - frag_pos); // vector from world position to world light position
    vec3 normal_w = normalize((mat3(u_model) * total_normal)); // normal vector in world space

    // facing towards the light, basically do as normal
    gl_Position = u_projection * u_view * vec4(frag_pos, 1.0);

    uv = in_uv;
}