#version 330 core

layout (location = 0) out float out_depth;
layout (location = 1) out vec3 out_mask;

in vec2 uv;
in mat3 TBN;
in vec3 frag_pos;

float near = 0.1;
float far = 1000.0;

uniform sampler2D scene_depth;
uniform sampler2D backface_depth;
uniform int light_num;
uniform vec3 u_camera_pos;
uniform int pass;

// from learnopengl.com
float LinearizeDepth(float depth)
{
    float z = depth * 2.0 - 1.0; // back to NDC
    return (2.0 * near * far) / (far + near - z * (far - near));
}

bool is_polygon_facing_camera() {
    // the tbn should give us the normals in world space
    // we can then compare the normal to the camera position
    // if the normal is facing the camera, then the polygon is facing the camera
    // if the normal is facing away from the camera, then the polygon is facing away from the camera
    vec3 normal = normalize(TBN[2]);
    vec3 camera_dir = normalize(vec3(0.0) - frag_pos);
    return dot(normal, camera_dir) > 0.0;
}

void main() {
    // out_depth component 1 is the depth of polygons facing away from the camera
    // out_depth component 2 is the depth of polygons facing towards the camera
    // out_mask is light_num (1-254) converted to 0.0 - 1.0
    // out_mask is 0.0 if light_num is 0 (no light)

    // only render if the scene depth is less than or equal to the depth of polygons facing towards the camera
    // and greater than the depth of polygons facing away from the camera
    // this prevents the shadow from being rendered in the air

    float scene_depth = texture(scene_depth, gl_FragCoord.xy / textureSize(scene_depth, 0)).r;
    float depth = 1.0 - LinearizeDepth(gl_FragCoord.z) / far;


    if (pass == 1) {
        if (depth >= scene_depth) {
            out_depth = 1.0;
        } else {
            discard;
        }
    } else if (pass == 2) {
        float backface_shadow = texture(backface_depth, gl_FragCoord.xy / textureSize(backface_depth, 0)).r;
        bool in_shadow = backface_shadow == 1.0 && depth <= scene_depth;
        if (in_shadow) {
            out_depth = 1.0;
        } else {
            discard;
        }
    }
}