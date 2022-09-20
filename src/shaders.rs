use glad_gl::gl::*;

#[derive(Clone)]
pub struct Shader {
    pub name: String,
    pub program: GLuint,
}