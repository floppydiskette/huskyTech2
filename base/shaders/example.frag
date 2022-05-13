// stole from wikipedia (:

#version 150 // Specify which version of GLSL we are using.

precision highp float; // Video card drivers require this line to function properly

out vec4 fragColor;

void main()
{
    fragColor = vec4(1.0,1.0,1.0,1.0); //Set colour of each fragment to WHITE
}