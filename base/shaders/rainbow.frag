#version 330 core

out vec4 o_colour;

uniform vec4 i_colour;

void main(){
    o_colour = i_colour;
}