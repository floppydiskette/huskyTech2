#version 330 core

layout (location = 0) out vec3 out_depth;

in vec2 uv;
in mat3 TBN;
in vec3 frag_pos;
in vec3 normal;

float near = 0.1;
float far = 100.0;

uniform sampler2D scene_depth;
uniform isampler2D backface_mask;
uniform int light_num_plus_one;
uniform vec3 u_camera_pos;
uniform int pass;
uniform mat4 u_model;
uniform vec3 light_pos; // position of the current light

void main() {
    out_depth = vec3(1.0);
}