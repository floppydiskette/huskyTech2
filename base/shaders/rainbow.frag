#version 330 core

uniform f32vec4 u_color;
out vec4 f_color;

void main(){
    f_color = u_color;
}