#pragma once

//is the program running? if not, we need to close it asap
bool alive = true;

//the glfw window context
GLFWwindow* window;

//destructor
void explode();