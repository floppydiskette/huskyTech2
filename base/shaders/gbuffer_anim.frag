#version 330 core

layout (location = 0) out vec4 out_pos;
layout (location = 1) out vec4 out_normal;
layout (location = 2) out vec4 out_albedospec;
layout (location = 3) out vec4 out_info;
layout (location = 4) out vec4 out_info2;

in vec2 uv;
in mat3 TBN;
in vec3 frag_pos;

uniform sampler2D diffuse;
uniform sampler2D specular;
uniform sampler2D normalmap;

uniform float opacity = 1.0;
uniform bool unlit = false;

float near = 0.1;
float far = 10000.0;

// from learnopengl.com
float LinearizeDepth(float depth)
{
    float z = depth * 2.0 - 1.0; // back to NDC
    return (2.0 * near * far) / (far + near - z * (far - near));
}

void main() {
    float depth = gl_FragCoord.z;
    depth = LinearizeDepth(depth) / far;

    if (!unlit) {
        vec3 normal = texture(normalmap, uv).rgb * 2.0 - 1.0;
        normal = normalize(TBN * normal) * 0.5 + 0.5;

        out_pos = vec4(frag_pos, opacity);
        out_normal = vec4(normal, 1.0);
        out_albedospec = vec4(texture(diffuse, uv).rgb, 1.0);
        out_info = vec4(texture(specular, uv).r, opacity, unlit ? 1.0 : 0.0, 1.0);
        out_info2 = vec4(depth, 0.0, 0.0, 1.0);
    } else {
        out_normal = vec4(0.0, 0.0, 0.0, 1.0);
        out_albedospec = vec4(texture(diffuse, uv).rgb, 1.0);
        out_info = vec4(0.0, opacity, unlit ? 1.0 : 0.0, 1.0);
        out_info2 = vec4(depth, 0.0, 0.0, 1.0);
    }
}