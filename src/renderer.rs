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
use crate::camera::*;
#[cfg(feature = "glfw")]
use libsex::bindings::*;
use crate::meshes::Mesh;
use crate::textures::Texture;

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
}

#[derive(Clone)]
pub struct ht_renderer {
    pub type_: RenderType,
    pub window_size: Vec2,
    pub camera: Camera,
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
                    glCullFace(GL_BACK);
                    glEnable(GL_DEPTH_TEST);
                    glDepthFunc(GL_LESS);


                    glViewport(0, 0, window_width as i32, window_height as i32);
                    // make top left corner as origin

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
                camera,
                backend,
            })
        }
    }

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

    pub fn initMesh(&mut self, doc: Document, mesh_name: &str, shader_index: usize, load_textures: bool) -> Result<Mesh, String> {
        // loading the texture (todo: support multiple materials)
        let mut texture: Option<Texture> = Option::None;
        if load_textures { texture = Some(Texture::new_from_name(format!("{}/tex", mesh_name)).expect("failed to load texture")); }
        let geom = doc.local_map::<Geometry>().expect("mesh not found").get_str(&*mesh_name).unwrap();
        let mesh = geom.element.as_mesh().expect("NO MESH?"); // this is a reference to the no bitches meme
        let tris = mesh.elements[0].as_triangles().expect("NO TRIANGLES?");
        let vertices_map = doc.local_map::<Vertices>().expect("no vertices?");
        let vertices = vertices_map.get_raw(&tris.inputs[0].source).expect("no vertices? (2)");
        let source_map = doc.local_map::<Source>().expect("no sources?");
        let source = source_map.get_raw(&vertices.inputs[0].source).expect("no positions?");
        let uv_source = source_map.get_raw(&tris.inputs[2].source).expect("no uv source?");

        let array = source.array.clone().expect("NO ARRAY?");
        let uv_array = uv_source.array.clone().expect("NO UV ARRAY?");

        // get the u32 data from the mesh
        let mut vbo = 0 as GLuint;
        let mut vao = 0 as GLuint;
        let mut ebo = 0 as GLuint;
        let mut uvbo= 0 as GLuint;
        let mut indices = tris.data.clone().prim.expect("no indices?");
        // 9 accounts for the x3 needed to convert to triangles, and the x3 needed to skip the normals and tex coords
        let num_indices = tris.count * 9;

        // todo: this only counts for triangulated collada meshes made in blender, we cannot assume that everything else will act like this

        // indices for vertex positions are offset by the normal and texcoord indices
        // we need to skip the normal and texcoord indices and fill a new array with the vertex positions
        let mut new_indices = Vec::with_capacity(num_indices);
        let mut new_uv_indices = Vec::with_capacity(num_indices);
        // skip the normal (offset 1) and texcoord (offset 2) indices
        let mut i = 0;
        while i < num_indices {
            new_indices.push(indices[i] as u32);
            new_uv_indices.push(indices[i + 2] as u32);
            i += 3;
        }


        let indices = new_indices;
        let num_indices = indices.len();
        debug!("num indices: {}", num_indices);
        unsafe {
            // set the shader program
            if self.backend.current_shader != Some(shader_index) {
                unsafe {
                    glUseProgram(self.backend.shaders.as_mut().unwrap()[shader_index].program);
                    self.backend.current_shader = Some(shader_index);
                }
            }

            glGenVertexArrays(1, &mut vao);
            glBindVertexArray(vao);
            glGenBuffers(1, &mut vbo);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            // assuming that the world hasn't imploded, the array should be either a float array or an int array
            // the array is currently an ArrayElement enum, we need to get the inner value
            let mut size;
            if let ArrayElement::Float(a) = array {
                debug!("len: {}", a.val.len());
                debug!("type: float");
                size = a.val.len() * std::mem::size_of::<f32>();
                glBufferData(GL_ARRAY_BUFFER, size as GLsizeiptr, a.val.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
            } else if let ArrayElement::Int(a) = array {
                debug!("len: {}", a.val.len());
                debug!("type: int");
                size = a.val.len() * std::mem::size_of::<i32>();
            } else {
                panic!("unsupported array type");
            }
            // vertex positions for vertex shader
            let pos = glGetAttribLocation(self.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_pos").unwrap().as_ptr());
            glVertexAttribPointer(pos as GLuint, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null());
            glEnableVertexAttribArray(0);

            // uvs
            glGenBuffers(1, &mut uvbo);
            glBindBuffer(GL_ARRAY_BUFFER, uvbo);
            if let ArrayElement::Float(a) = uv_array {
                debug!("len: {}", a.val.len());
                debug!("type: float");
                size = a.val.len() * std::mem::size_of::<f32>();
                glBufferData(GL_ARRAY_BUFFER, size as GLsizeiptr, a.val.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
            } else {
                panic!("unsupported array type for uvs");
            }
            // vertex uvs for fragment shader
            let uv = glGetAttribLocation(self.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_uv").unwrap().as_ptr());
            glVertexAttribPointer(uv as GLuint, 2, GL_FLOAT, GL_FALSE as GLboolean, 0, null());
            glEnableVertexAttribArray(1);


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
                position: Vec3::new(0.0, 0.0, 0.0),
                rotation: Quaternion::identity(),
                scale: Vec3::new(1.0, 1.0, 1.0),
                vbo,
                vao,
                ebo,
                uvbo,
                num_vertices,
                num_indices,
                texture
            })
        } else if let ArrayElement::Int(array) = array {
            let num_vertices = array.val.len();
            Ok(Mesh {
                position: Vec3::new(0.0, 0.0, 0.0),
                rotation: Quaternion::identity(),
                scale: Vec3::new(1.0, 1.0, 1.0),
                vbo,
                vao,
                ebo,
                uvbo,
                num_vertices,
                num_indices,
                texture
            })
        } else {
            Err("unsupported array type".to_string())
        }
    }

    pub fn render_mesh(&mut self, mesh: Mesh, shader_index: usize, as_lines: bool, pass_texture: bool) {
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
            if pass_texture {
                glActiveTexture(GL_TEXTURE0);
                glBindTexture(GL_TEXTURE_2D, mesh.texture.unwrap().diffuse_texture);
                glUniform1i(glGetUniformLocation(self.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("u_texture").unwrap().as_ptr()), 0);
                // DON'T PRINT OPEN GL ERRORS HERE! BIGGEST MISTAKE OF MY LIFE
            }

            // transformation time!
            let camera_projection = self.camera.get_projection();
            let camera_view = self.camera.get_view();

            // calculate the model matrix
            let model_matrix = self.calculate_model_matrix(mesh.position, mesh.rotation, mesh.scale);

            // calculate the mvp matrix
            let mvp = camera_projection * camera_view * model_matrix;

            // send the mvp matrix to the shader
            let mvp_loc = glGetUniformLocation(self.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("u_mvp").unwrap().as_ptr());
            glUniformMatrix4fv(mvp_loc, 1, GL_FALSE as GLboolean, mvp.as_ptr());

            if !as_lines {
                glDrawElements(GL_TRIANGLES, mesh.num_indices as GLsizei, GL_UNSIGNED_INT, null());
            } else {
                glDrawElements(GL_LINES, mesh.num_indices as GLsizei, GL_UNSIGNED_INT, null());
            }
            glDisableVertexAttribArray(0);
        }
    }

    fn calculate_model_matrix(&self, position: Vec3, rotation: Quaternion, scale: Vec3) -> Mat4 {
        let mut model_matrix = Mat4::identity();
        model_matrix = model_matrix * Mat4::translate(position);
        model_matrix = model_matrix * Mat4::rotate(rotation);
        model_matrix = model_matrix * Mat4::scale(scale);
        model_matrix
    }
}