use std::any::Any;
use std::borrow::Borrow;
use std::ffi::{c_void, CStr, CString};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::iter::Map;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::os::raw::{c_int, c_uint, c_ulong};
use std::ptr::{null, null_mut};
use dae_parser::{ArrayElement, Document, FloatArray, Geometry, Source, Vertices};
use gfx_maths::*;
use crate::helpers;
use crate::shaders::*;
#[cfg(feature = "glfw")]
use libsex::bindings::*;

#[derive(Clone, Copy)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct Mesh {
    pub vbo: GLuint,
    pub vao: GLuint,
    pub ebo: GLuint,
    pub indices: Vec<u32>,
    pub num_vertices: usize,
    pub num_indices: usize,
}

pub struct Camera {
    pub position: Vec3,
    pub rotation: Vec3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
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
}

#[derive(Clone)]
pub struct ht_renderer {
    pub type_: RenderType,
    pub window_size: Vec2,
    #[cfg(feature = "glfw")]
    pub backend: GLFWBackend,
}

impl ht_renderer {
    pub fn init() -> Result<ht_renderer, String> {
        // some constants we can later change (todo: make these configurable?)
        let window_width = 640;
        let window_height = 480;

        #[cfg(feature = "glfw")]{
            let backend = {
                println!("running on linux, using glfw as backend");let display = unsafe { XOpenDisplay(null_mut()) };
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
                    //glEnable(GL_CULL_FACE);
                    //glCullFace(GL_BACK);
                    //glFrontFace(GL_CW);


                    glViewport(0, 0, window_width as i32, window_height as i32);
                    glMatrixMode(GL_PROJECTION);
                    glLoadIdentity();
                    // make top left corner as origin
                    //glOrtho(0.0, src_width as f64, src_height as f64, 0.0, -1.0, 1.0);
                    gluOrtho2D(0.0, window_width as f64, window_height as f64, 0.0);

                    glLineWidth(2.0);

                    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
                    GLFWBackend {
                        window,
                        current_shader: Option::None,
                        active_vbo: Option::None,
                        shaders: Option::None,
                    }
                }
            };

            Ok(ht_renderer {
                type_: RenderType::GLX,
                window_size: Vec2::new(window_width as f32, window_height as f32),
                backend,
            })
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
                let mut len = 0;
                glGetShaderiv(vert_shader, GL_INFO_LOG_LENGTH, &mut len);
                let mut log = Vec::with_capacity(len as usize);
                glGetShaderInfoLog(vert_shader, len, null_mut(), log.as_mut_ptr() as *mut GLchar);
                return Err(format!("failed to compile vertex shader: {}", std::str::from_utf8(&log).unwrap()));
            }
            glGetShaderiv(frag_shader, GL_COMPILE_STATUS, &mut status);
            if status == 0 {
                let mut len = 0;
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
            }
        }
    }

    pub fn initMesh(&mut self, doc: Document, mesh_name: &str, shader_index: usize) -> Result<Mesh, String> {
        let geom = doc.local_map::<Geometry>().expect("mesh not found").get_str(&*mesh_name).unwrap();
        let mesh = geom.element.as_mesh().expect("NO MESH?"); // this is a reference to the no bitches meme
        let tris = mesh.elements[0].as_triangles().expect("NO TRIANGLES?");
        let vertices_map = doc.local_map::<Vertices>().expect("no vertices?");
        let vertices = vertices_map.get_raw(&tris.inputs[0].source).expect("no vertices? (2)");
        let source_map = doc.local_map::<Source>().expect("no sources?");
        let source = source_map.get_raw(&vertices.inputs[0].source).expect("no positions?");

        let array = source.array.clone().expect("NO ARRAY?");

        // get the u32 data from the mesh
        let mut vbo = 0 as GLuint;
        let mut vao = 0 as GLuint;
        let mut ebo = 0 as GLuint;
        let mut indices = tris.data.clone().prim.expect("no indices?");
        // 9 accounts for the x3 needed to convert to triangles, and the x3 needed to skip the normals and tex coords
        let num_indices = tris.count * 9;

        // todo: this only counts for triangulated collada meshes made in blender, we cannot assume that everything else will act like this

        // indices for vertex positions are offset by the normal and texcoord indices
        // we need to skip the normal and texcoord indices and fill a new array with the vertex positions
        let mut new_indices = Vec::with_capacity(num_indices);
        // skip the normal (offset 1) and texcoord (offset 2) indices
        let mut i = 0;
        while i < num_indices {
            new_indices.push(indices[i] as u32);
            i += 3;
        }


        let indices = new_indices;
        println!("num indices: {}", num_indices);
        unsafe {
            println!("indices: {:?}", indices);

            glGenVertexArrays(1, &mut vao);
            glBindVertexArray(vao);
            glGenBuffers(1, &mut vbo);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            // assuming that the world hasn't imploded, the array should be either a float array or an int array
            // the array is currently an ArrayElement enum, we need to get the inner value
            let mut size;
            if let ArrayElement::Float(a) = array {
                println!("array: {:?}", a.val);
                println!("len: {}", a.val.len());
                println!("type: float");
                size = a.val.len() * std::mem::size_of::<f32>();
                glBufferData(GL_ARRAY_BUFFER, size as GLsizeiptr, a.val.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
            } else if let ArrayElement::Int(a) = array {
                println!("array: {:?}", a);
                println!("len: {}", a.val.len());
                println!("type: int");
                size = a.val.len() * std::mem::size_of::<i32>();
            } else {
                panic!("unsupported array type");
            }
            let pos = glGetAttribLocation(self.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_pos").unwrap().as_ptr());
            glVertexAttribPointer(pos as GLuint, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null());
            glEnableVertexAttribArray(0);

            // now the indices
            glGenBuffers(1, &mut ebo);
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, ebo);
            size = num_indices * std::mem::size_of::<i32>();
            glBufferData(GL_ELEMENT_ARRAY_BUFFER, size as GLsizeiptr, indices.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
        }

        let array = source.array.clone().expect("NO ARRAY?");

        if let ArrayElement::Float(array) = array {
            let num_vertices = array.val.len();
            Ok(Mesh {
                vbo,
                vao,
                ebo,
                indices: indices,
                num_vertices,
                num_indices,
            })
        } else if let ArrayElement::Int(array) = array {
            let num_vertices = array.val.len();
            Ok(Mesh {
                vbo,
                vao,
                ebo,
                indices: indices,
                num_vertices,
                num_indices,
            })
        } else {
            Err("unsupported array type".to_string())
        }
    }

    pub fn render_mesh(&mut self, mesh: Mesh, shader_index: usize) {
        // load the shader

        if self.backend.current_shader != Some(shader_index) {
            unsafe {
                glUseProgram(self.backend.shaders.as_mut().unwrap()[shader_index].program);
                self.backend.current_shader = Some(shader_index);
            }
        }
        unsafe {
            glEnableVertexAttribArray(0);
            glBindVertexArray(mesh.vao);
            glDrawElements(GL_TRIANGLES, mesh.num_indices as GLsizei, GL_UNSIGNED_INT, 0 as *const GLvoid);
            glDisableVertexAttribArray(0);
        }

        // print any errors
        let mut error = unsafe {
            glGetError()
        };
        while error != GL_NO_ERROR {
            println!("GL ERROR: {:?}", error);
            error = unsafe {
                glGetError()
            };
        }
    }

    // creates a vbo with a single triangle for testing
    pub fn gen_testing_triangle(&mut self) -> Mesh {
        let mut vbo = 0 as GLuint;
        let buffer_data: [f32; 9] = [
            -1.0, -1.0, 0.0,
            1.0, -1.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        println!("{:?}", buffer_data);
        unsafe {
            glGenBuffers(1, &mut vbo);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            glBufferData(GL_ARRAY_BUFFER, (buffer_data.len() * std::mem::size_of::<GLfloat>()) as GLsizeiptr, buffer_data.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
            // stuff for shaders (following wikipedia code for now)
            glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null_mut());
            glEnableVertexAttribArray(0);
            //glBindBuffer(GL_ARRAY_BUFFER, 0); // not sure if this is needed
        };
        let indices = [0, 1, 2];
        let num_vertices = 3;
        let mut ebo = 0 as GLuint;
        unsafe {
            glGenBuffers(1, &mut ebo);
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, ebo);
            glBufferData(GL_ELEMENT_ARRAY_BUFFER, (indices.len() * std::mem::size_of::<GLuint>()) as GLsizeiptr, indices.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
        };

        Mesh {
            vbo,
            vao: 0,
            ebo,
            indices: indices.to_vec(),
            num_vertices: 3,
            num_indices: 3,
        }
    }
}