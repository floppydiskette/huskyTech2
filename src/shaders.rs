use libsex::bindings::*;

#[derive(Clone)]
pub struct Shader {
    pub name: String,
    pub program: GLuint,
}