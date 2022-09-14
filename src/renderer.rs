use std::any::Any;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::iter::Map;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::os::raw::{c_int, c_uint, c_ulong};
use std::ptr::{null, null_mut};
use gfx_maths::*;
use crate::helpers;
use crate::shaders::*;
use crate::camera::*;
#[cfg(feature = "glfw")]
use libsex::bindings::*;
use crate::light::Light;
use crate::meshes::Mesh;
use crate::textures::Texture;
use crate::uimesh::UiMesh;

pub static MAX_LIGHTS: usize = 100;
pub static SHADOW_SIZE: usize = 1024;

#[derive(Clone, Copy)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy)]
pub enum RenderType {
    GLX,
}

#[cfg(feature = "glfw")]
#[derive(Clone)]
pub struct GLFWBackend {
    pub window: *mut GLFWwindow,
    pub active_vbo: Option<GLuint>,
    pub current_shader: Option<usize>,
    pub shaders: Option<Vec<Shader>>,
    pub ui_master: Option<UiMesh>,
}

#[derive(Clone)]
pub struct ht_renderer {
    pub type_: RenderType,
    pub window_size: Vec2,
    pub camera: Camera,
    pub textures: HashMap<String, Texture>,
    pub meshes: HashMap<String, Mesh>,
    pub shaders: HashMap<String, usize>,
    pub lights: Vec<Light>,
    #[cfg(feature = "glfw")]
    pub backend: GLFWBackend,
}

impl ht_renderer {
    pub fn init() -> Result<ht_renderer, String> {
        // some constants we can later change (todo: make these configurable?)
        let window_width = 1280;
        let window_height = 720;

        let camera = Camera::new(Vec2::new(window_width as f32, window_height as f32), 45.0, 0.1, 100.0);

        #[cfg(feature = "glfw")]{
            let backend = {
                info!("running on linux, using glfw as backend");
                unsafe {
                    let result = glfwInit();
                    if result == 0 {
                        return Err("glfwInit failed".to_string());
                    }
                    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR as c_int, 3);
                    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR as c_int, 3);
                    glfwWindowHint(GLFW_OPENGL_PROFILE as c_int, GLFW_OPENGL_CORE_PROFILE as c_int);
                    glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT as c_int, GL_TRUE as c_int);

                    let window = glfwCreateWindow(window_width as c_int,
                                                  window_height as c_int,
                                                  CString::new("huskyTech2").unwrap().as_ptr(),
                                                  null_mut(), null_mut());
                    if window.is_null() {
                        glfwTerminate();
                        return Err("glfwCreateWindow failed".to_string());
                    }
                    glfwMakeContextCurrent(window);
                    glfwSetInputMode(window, GLFW_STICKY_KEYS as c_int, GL_TRUE as c_int);


                    // Configure culling
                    glEnable(GL_CULL_FACE);
                    glCullFace(GL_FRONT);
                    glEnable(GL_DEPTH_TEST);
                    glDepthFunc(GL_LESS);

                    // enable blending
                    glEnable(GL_BLEND);
                    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

                    glViewport(0, 0, window_width as i32, window_height as i32);
                    // make top left corner as origin

                    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

                    GLFWBackend {
                        window,
                        current_shader: Option::None,
                        active_vbo: Option::None,
                        shaders: Option::None,
                        ui_master: Option::None,
                    }
                }
            };

            Ok(ht_renderer {
                type_: RenderType::GLX,
                window_size: Vec2::new(window_width as f32, window_height as f32),
                camera,
                textures: Default::default(),
                meshes: Default::default(),
                shaders: Default::default(),
                lights: Vec::new(),
                backend,
            })
        }
    }

    pub fn initialise_basic_resources(&mut self) {
        // load rainbow shader
        self.load_shader("rainbow").expect("failed to load rainbow shader");
        // load basic shader
        let basic = self.load_shader("basic").expect("failed to load basic shader");
        // load master uimesh
        let ui_master = UiMesh::new_master(self, basic).expect("failed to load master uimesh");
        self.backend.ui_master = Some(ui_master);
        // load default texture
        self.load_texture_if_not_already_loaded("default").expect("failed to load default texture");
    }

    pub fn load_texture_if_not_already_loaded(&mut self, name: &str) -> Result<(), crate::textures::TextureError> {
        if !self.textures.contains_key(name) {
            Texture::load_texture(name, format!("{}/{}", name, name).as_str(), self)?;
        }
        Ok(())
    }

    pub fn load_mesh_if_not_already_loaded(&mut self, name: &str) -> Result<(), crate::meshes::MeshError> {
        if !self.meshes.contains_key(name) {
            let mesh = Mesh::new(format!("base/models/{}.glb", name).as_str(), name,
                                 *self.shaders.get("basic").unwrap(), self)?;
            self.meshes.insert(name.to_string(), mesh);
        }
        Ok(())
    }

    /*pub fn load_terrain_if_not_already_loaded(&mut self, name: &str) -> Result<(), String> {
        if !self.terrains.contains_key(name) {
            let terrain = Terrain::new_from_name(name, self)?;
            self.terrains.as_mut().unwrap().insert(name.to_string(), terrain);
        }
        Ok(())
    }
     */

    // closes the window if it needs to, etc.
    // returns true if the window should close
    pub fn manage_window(&mut self) -> bool {
        #[cfg(feature = "glfw")]{
            unsafe {
                glfwPollEvents();
                if glfwWindowShouldClose(self.backend.window) == 1 {
                    glfwTerminate();
                    return true;
                }
            }
            false
        }
    }

    pub fn load_shader(&mut self, shader_name: &str) -> Result<usize, String> {
        // read the files
        let vert_source = helpers::load_string_from_file(format!("base/shaders/{}.vert", shader_name)).expect("failed to load vertex shader");
        let frag_source = helpers::load_string_from_file(format!("base/shaders/{}.frag", shader_name)).expect("failed to load fragment shader");

        // convert strings to c strings
        let vert_source_c = CString::new(vert_source).unwrap();
        let frag_source_c = CString::new(frag_source).unwrap();

        // create the shaders
        let vert_shader = unsafe { glCreateShader(GL_VERTEX_SHADER) };
        let frag_shader = unsafe { glCreateShader(GL_FRAGMENT_SHADER) };

        // set the source
        unsafe {
            glShaderSource(vert_shader, 1, &vert_source_c.as_ptr(), null_mut());
            glShaderSource(frag_shader, 1, &frag_source_c.as_ptr(), null_mut());
        }

        // compile the shaders
        unsafe {
            glCompileShader(vert_shader);
            glCompileShader(frag_shader);
        }

        // check if the shaders compiled
        let mut status = 0;
        unsafe {
            glGetShaderiv(vert_shader, GL_COMPILE_STATUS, &mut status);
            if status == 0 {
                let mut len = 255;
                glGetShaderiv(vert_shader, GL_INFO_LOG_LENGTH, &mut len);
                let mut log = Vec::with_capacity(len as usize);
                glGetShaderInfoLog(vert_shader, len, null_mut(), log.as_mut_ptr() as *mut GLchar);
                return Err(format!("failed to compile vertex shader: {}", std::str::from_utf8(&log).unwrap()));
            }
            glGetShaderiv(frag_shader, GL_COMPILE_STATUS, &mut status);
            if status == 0 {
                let mut len = 255;
                glGetShaderiv(frag_shader, GL_INFO_LOG_LENGTH, &mut len);
                let mut log = Vec::with_capacity(len as usize);
                glGetShaderInfoLog(frag_shader, len, null_mut(), log.as_mut_ptr() as *mut GLchar);
                return Err(format!("failed to compile fragment shader: {}", std::str::from_utf8(&log).unwrap()));
            }
        }

        // link the shaders
        let shader_program = unsafe { glCreateProgram() };
        unsafe {
            glAttachShader(shader_program, vert_shader);
            glAttachShader(shader_program, frag_shader);
            glLinkProgram(shader_program);
        }

        // check if the shaders linked
        unsafe {
            glGetProgramiv(shader_program, GL_LINK_STATUS, &mut status);
            if status == 0 {
                let mut len = 0;
                glGetProgramiv(shader_program, GL_INFO_LOG_LENGTH, &mut len);
                let mut log = Vec::with_capacity(len as usize);
                glGetProgramInfoLog(shader_program, len, null_mut(), log.as_mut_ptr() as *mut GLchar);
                return Err(format!("failed to link shader program: {}", std::str::from_utf8(&log).unwrap()));
            }
        }

        // clean up
        unsafe {
            glDeleteShader(vert_shader);
            glDeleteShader(frag_shader);
        }

        // add shader to list
        if self.backend.shaders.is_none() {
            self.backend.shaders = Option::Some(Vec::new());
        }
        self.backend.shaders.as_mut().unwrap().push(Shader {
            name: shader_name.parse().unwrap(),
            program: shader_program,
        });

        // return the index of the shader
        Ok(self.backend.shaders.as_mut().unwrap().len() - 1)
    }

    pub fn swap_buffers(&mut self) {
        #[cfg(target_os = "linux")]
        {
            unsafe {
                glfwSwapBuffers(self.backend.window);
                glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
            }
        }
    }

}