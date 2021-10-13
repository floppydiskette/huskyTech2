//globals
#include "globals.h"

//standard libraries
#include <stdlib.h>
#include <stdio.h>

//glew sticky
#define GLEW_STATIC
#include <GL/glew.h>

//glfw
#include <GLFW/glfw3.h>

//maybe include huskymath in the future?

//handle input
void input_update() {

	//handle exiting the game
	if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS || glfwWindowShouldClose(window) != 0) {
		alive = false;
	}


}

//render to the screen
void draw() {
}

int main() {

	glewExperimental = true;

	if (!glfwInit()) {
		//find a better way to handle this error in the future
		fprintf(stderr, "FAILED TO INITIALIZE GLFW!\n");
		return -1;
	}

	//window hints
	glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3); //maybe switch to a newer opengl version in the future?
	glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
	glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT, GL_TRUE);
	glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

	//open the window
	window = glfwCreateWindow(1280, 720, "huskyTech2", NULL, NULL);
	
	//did window work?
	if (window == NULL) {
		fprintf(stderr, "FAILED TO OPEN GLFW WINDOW!\n");
		glfwTerminate();
		return -1;
	}
	glfwMakeContextCurrent(window);
	glewExperimental = true;

	if (glewInit() != GLEW_OK) {
		fprintf(stderr, "FAILED TO INITIALIZE GLEW!\n");
		return -1;
	}

	//use sticky keys input mode, we may want to change this in the future
	glfwSetInputMode(window, GLFW_STICKY_KEYS, GL_TRUE);

	while (alive) {
		input_update();

		//the program may no longer be alive once we finish the previous methods
		if (alive) {
			draw();
		}
		else {
			//destroy the program
			explode();
			break;
		}
	}
}

void explode() {
	glfwDestroyWindow(window);
}