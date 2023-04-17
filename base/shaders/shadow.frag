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

// from learnopengl.com
float LinearizeDepth(float depth)
{
    float z = depth * 2.0 - 1.0; // back to NDC
    return (2.0 * near * far) / (far + near - z * (far - near));
}

void main() {
    vec3 light_dir = (frag_pos - light_pos);
    // out_depth component 1 is the depth of polygons facing away from the camera
    // out_depth component 2 is the depth of polygons facing towards the camera
    // out_mask is light_num (1-254) converted to 0.0 - 1.0
    // out_mask is 0.0 if light_num is 0 (no light)

    // only render if the scene depth is less than or equal to the depth of polygons facing towards the camera
    // and greater than the depth of polygons facing away from the camera
    // this prevents the shadow from being rendered in the air

    float scene_depth = texture(scene_depth, gl_FragCoord.xy / textureSize(scene_depth, 0)).r;
    float depth = LinearizeDepth(gl_FragCoord.z) / far;
    bool front_on_ground = scene_depth <= depth;

    //if (front_on_ground) {
    out_depth = vec3(1.0);
    //} else {
    //    discard;
    //}
}