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
use crate::helpers::set_shader_if_not_already;
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
    pub framebuffers: Framebuffers,
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

#[derive(Debug, Clone)]
pub struct Framebuffers {
    pub original: usize,

    pub postbuffer: usize,
    pub postbuffer_texture: usize,
    pub postbuffer_rbuffer: usize,

    pub depthbuffer: usize,
    pub depthbuffer_texture: usize,

    pub screenquad_vao: usize,

    // gbuffer
    pub gbuffer: usize,
    pub gbuffer_position: usize,
    pub gbuffer_normal: usize,
    pub gbuffer_albedo: usize, // or colour, call it what you want
    pub gbuffer_info: usize, // specular, lighting, etc
    pub gbuffer_rbuffer: usize,

}

impl ht_renderer {
    pub fn init() -> Result<ht_renderer, String> {
        // some constants we can later change (todo: make these configurable?)
        let window_width = 1280;
        let window_height = 720;

        let camera = Camera::new(Vec2::new(window_width as f32, window_height as f32), 45.0, 0.1, 10000.0);

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

                    let mut framebuffers = Framebuffers {
                        original: 0,
                        postbuffer: 0,
                        postbuffer_texture: 0,
                        postbuffer_rbuffer: 0,
                        depthbuffer: 0,
                        depthbuffer_texture: 0,
                        screenquad_vao: 0,
                        gbuffer: 0,
                        gbuffer_position: 0,
                        gbuffer_normal: 0,
                        gbuffer_albedo: 0,
                        gbuffer_info: 0,
                        gbuffer_rbuffer: 0
                    };

                    // get the number of the current framebuffer
                    let mut original: i32 = 0;
                    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &mut original);
                    framebuffers.original = original as usize;
                    debug!("original framebuffer: {}", framebuffers.original);

                    // Configure culling
                    glEnable(GL_CULL_FACE);
                    glCullFace(GL_FRONT);
                    glEnable(GL_DEPTH_TEST);
                    glDepthFunc(GL_LESS);

                    // configure stencil test
                    glEnable(GL_STENCIL_TEST);

                    // enable blending
                    glEnable(GL_BLEND);
                    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

                    // create the postprocessing framebuffer
                    let mut postbuffer = 0;
                    glGenFramebuffers(1, &mut postbuffer);
                    glBindFramebuffer(GL_FRAMEBUFFER, postbuffer);
                    let mut posttexture = 0;
                    glGenTextures(1, &mut posttexture);
                    glBindTexture(GL_TEXTURE_2D, posttexture);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGB as i32, window_width as i32, window_height as i32, 0, GL_RGB, GL_UNSIGNED_BYTE, std::ptr::null());
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, posttexture, 0);
                    // create a renderbuffer object for depth and stencil attachment (we won't be sampling these)
                    let mut renderbuffer = 0;
                    glGenRenderbuffers(1, &mut renderbuffer);
                    glBindRenderbuffer(GL_RENDERBUFFER, renderbuffer);
                    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, window_width as i32, window_height as i32);
                    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_RENDERBUFFER, renderbuffer);

                    // check if framebuffer is complete
                    if glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
                        panic!("framebuffer is not complete!");
                    }
                    framebuffers.postbuffer = postbuffer as usize;
                    framebuffers.postbuffer_texture = posttexture as usize;
                    framebuffers.postbuffer_rbuffer = renderbuffer as usize;

                    // create a simple quad that fills the screen
                    let mut screenquad_vao = 0;
                    glGenVertexArrays(1, &mut screenquad_vao);
                    glBindVertexArray(screenquad_vao);
                    let mut screenquad_vbo = 0;
                    glGenBuffers(1, &mut screenquad_vbo);
                    glBindBuffer(GL_ARRAY_BUFFER, screenquad_vbo);
                    // just stealing this from the learnopengl.com tutorial (it's a FUCKING QUAD, HOW ORIGINAL CAN IT BE?)
                    let quad_vertices: [f32; 30] = [
                        // positions        // texture Coords
                        -1.0,  1.0, 0.0,    0.0, 1.0,
                        -1.0, -1.0, 0.0,    0.0, 0.0,
                        1.0, -1.0, 0.0,    1.0, 0.0,

                        -1.0,  1.0, 0.0,    0.0, 1.0,
                        1.0, -1.0, 0.0,    1.0, 0.0,
                        1.0,  1.0, 0.0,    1.0, 1.0,
                    ];
                    glBufferData(GL_ARRAY_BUFFER, (quad_vertices.len() * std::mem::size_of::<f32>()) as GLsizeiptr, quad_vertices.as_ptr() as *const c_void, GL_STATIC_DRAW);
                    // as this is such a simple quad, we're not gonna bother with indices
                    glEnableVertexAttribArray(0);
                    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as GLboolean, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
                    glEnableVertexAttribArray(1);
                    glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE as GLboolean, 5 * std::mem::size_of::<f32>() as i32, (3 * std::mem::size_of::<f32>()) as *const c_void);
                    framebuffers.screenquad_vao = screenquad_vao as usize;

                    // create the depth framebuffer
                    let mut depthbuffer = 0;
                    glGenFramebuffers(1, &mut depthbuffer);
                    glBindFramebuffer(GL_FRAMEBUFFER, depthbuffer);
                    let mut depthtexture = 0;
                    glGenTextures(1, &mut depthtexture);
                    glBindTexture(GL_TEXTURE_2D, depthtexture);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_DEPTH_COMPONENT as i32, SHADOW_SIZE as i32, SHADOW_SIZE as i32, 0, GL_DEPTH_COMPONENT, GL_FLOAT, std::ptr::null());
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as i32);
                    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_TEXTURE_2D, depthtexture, 0);
                    glDrawBuffer(GL_NONE);
                    glReadBuffer(GL_NONE);
                    if glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
                        panic!("framebuffer is not complete (depth buffer)!");
                    }

                    framebuffers.depthbuffer = depthbuffer as usize;
                    framebuffers.depthbuffer_texture = depthtexture as usize;

                    // create the gbuffer
                    let mut gbuffer = 0;
                    glGenFramebuffers(1, &mut gbuffer);
                    glBindFramebuffer(GL_FRAMEBUFFER, gbuffer);
                    let mut gbuffer_textures = [0; 4];
                    glGenTextures(4, gbuffer_textures.as_mut_ptr());

                    // position
                    glBindTexture(GL_TEXTURE_2D, gbuffer_textures[0]);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA16F as i32, window_width as i32, window_height as i32, 0, GL_RGBA, GL_FLOAT, std::ptr::null());
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as i32);
                    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, gbuffer_textures[0], 0);
                    // normal
                    glBindTexture(GL_TEXTURE_2D, gbuffer_textures[1]);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA16F as i32, window_width as i32, window_height as i32, 0, GL_RGBA, GL_FLOAT, std::ptr::null());
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as i32);
                    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT1, GL_TEXTURE_2D, gbuffer_textures[1], 0);
                    // color
                    glBindTexture(GL_TEXTURE_2D, gbuffer_textures[2]);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, window_width as i32, window_height as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, std::ptr::null());
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as i32);
                    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT2, GL_TEXTURE_2D, gbuffer_textures[2], 0);
                    // info
                    glBindTexture(GL_TEXTURE_2D, gbuffer_textures[3]);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, window_width as i32, window_height as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, std::ptr::null());
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as i32);
                    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT3, GL_TEXTURE_2D, gbuffer_textures[3], 0);

                    let attachments = [GL_COLOR_ATTACHMENT0, GL_COLOR_ATTACHMENT1, GL_COLOR_ATTACHMENT2, GL_COLOR_ATTACHMENT3];
                    glDrawBuffers(4, attachments.as_ptr());

                    if glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
                        panic!("framebuffer is not complete (gbuffer)!");
                    }

                    framebuffers.gbuffer = gbuffer as usize;
                    framebuffers.gbuffer_position = gbuffer_textures[0] as usize;
                    framebuffers.gbuffer_normal = gbuffer_textures[1] as usize;
                    framebuffers.gbuffer_albedo = gbuffer_textures[2] as usize;
                    framebuffers.gbuffer_info = gbuffer_textures[3] as usize;

                    // renderbuffer for gbuffer
                    let mut gbuffer_renderbuffer = 0;
                    glGenRenderbuffers(1, &mut gbuffer_renderbuffer);
                    glBindRenderbuffer(GL_RENDERBUFFER, gbuffer_renderbuffer);
                    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT, window_width as i32, window_height as i32);
                    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, gbuffer_renderbuffer);

                    glViewport(0, 0, window_width as i32, window_height as i32);
                    // make top left corner as origin

                    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

                    // print opengl errors
                    let mut error = glGetError();
                    while error != GL_NO_ERROR {
                        error!("OpenGL error while initialising render subsystem: {}", error);
                        error = glGetError();
                    }

                    GLFWBackend {
                        window,
                        current_shader: Option::None,
                        active_vbo: Option::None,
                        shaders: Option::None,
                        ui_master: Option::None,
                        framebuffers
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
        // load postbuffer shader
        self.load_shader("postbuffer").expect("failed to load postbuffer shader");
        // load gbuffer shader
        self.load_shader("gbuffer").expect("failed to load gbuffer shader");
        // load lighting shader
        self.load_shader("lighting").expect("failed to load lighting shader");
        // load rainbow shader
        self.load_shader("rainbow").expect("failed to load rainbow shader");
        // load basic shader
        let basic = self.load_shader("basic").expect("failed to load basic shader");
        // load unlit shader
        let unlit = self.load_shader("unlit").expect("failed to load unlit shader");
        // load master uimesh
        let ui_master = UiMesh::new_master(self, unlit).expect("failed to load master uimesh");
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
                let log = vec![0; len as usize + 1];
                let log_c = CString::from_vec_unchecked(log);
                let log_p = log_c.into_raw();
                glGetShaderInfoLog(vert_shader, len, null_mut(), log_p);
                return Err(format!("failed to compile vertex shader: {}", CString::from_raw(log_p).to_string_lossy()));
            }
            glGetShaderiv(frag_shader, GL_COMPILE_STATUS, &mut status);
            if status == 0 {
                let mut len = 255;
                glGetShaderiv(frag_shader, GL_INFO_LOG_LENGTH, &mut len);
                let log = vec![0; len as usize + 1];
                let log_c = CString::from_vec_unchecked(log);
                let log_p = log_c.into_raw();
                glGetShaderInfoLog(frag_shader, len, null_mut(), log_p);
                return Err(format!("failed to compile fragment shader: {}", CString::from_raw(log_p).to_string_lossy()));
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

        // add shader index to list
        self.shaders.insert(shader_name.to_string(), self.backend.shaders.as_ref().unwrap().len() - 1);

        // return the index of the shader
        Ok(self.backend.shaders.as_mut().unwrap().len() - 1)
    }

    pub fn set_lights(&mut self, lights: Vec<Light>) {
        self.lights = lights;
    }

    pub fn swap_buffers(&mut self) {
        self.setup_pass_two();
        self.setup_pass_three();
        unsafe {
            glfwSwapBuffers(self.backend.window);
            let mut width = 0;
            let mut height = 0;
            glfwGetFramebufferSize(self.backend.window, &mut width, &mut height);
            self.window_size = Vec2::new(width as f32, height as f32);
        }
        self.setup_pass_one();
    }

    // geometry pass
    fn setup_pass_one(&mut self) {
        let gbuffer_shader = *self.shaders.get("gbuffer").unwrap();

        set_shader_if_not_already(self, gbuffer_shader);

        unsafe {
            glViewport(0, 0, self.window_size.x as i32, self.window_size.y as i32);

            // set framebuffer to the post processing framebuffer
            glBindFramebuffer(GL_FRAMEBUFFER, self.backend.framebuffers.gbuffer as GLuint);

            glEnable(GL_CULL_FACE);
            glCullFace(GL_FRONT);
            glEnable(GL_DEPTH_TEST);
            glDepthFunc(GL_LESS);

            // disable gamma correction
            glDisable(GL_FRAMEBUFFER_SRGB);

            // set the clear color to black
            glClearColor(0.0, 0.0, 0.0, 1.0);
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        }
    }

    // lighting pass
    fn setup_pass_two(&mut self) {
        let lighting_shader = *self.shaders.get("lighting").unwrap();

        set_shader_if_not_already(self, lighting_shader);

        let lighting_shader = self.backend.shaders.as_ref().unwrap().get(lighting_shader).unwrap();

        unsafe {
            // set framebuffer to the post processing framebuffer
            glBindFramebuffer(GL_FRAMEBUFFER, self.backend.framebuffers.postbuffer as GLuint);
            glViewport(0, 0, self.window_size.x as GLsizei, self.window_size.y as GLsizei);

            // set the clear color to black
            glClearColor(0.0, 0.0, 0.0, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);

            // send the lights to the shader
            let light_count = self.lights.len();
            let light_count = if light_count > MAX_LIGHTS { MAX_LIGHTS } else { light_count };
            let light_count_c = CString::new("u_light_count").unwrap();
            let light_count_loc = glGetUniformLocation(lighting_shader.program, light_count_c.as_ptr());
            glUniform1i(light_count_loc, light_count as i32);
            for (i, light) in self.lights.iter().enumerate() {
                if i >= MAX_LIGHTS { break; }
                let light_pos_c = CString::new(format!("u_lights[{}].position", i)).unwrap();
                let light_pos = glGetUniformLocation(lighting_shader.program, light_pos_c.as_ptr());
                let light_colour_c = CString::new(format!("u_lights[{}].colour", i)).unwrap();
                let light_color = glGetUniformLocation(lighting_shader.program, light_colour_c.as_ptr());
                let light_intensity_c = CString::new(format!("u_lights[{}].intensity", i)).unwrap();
                let light_intensity = glGetUniformLocation(lighting_shader.program, light_intensity_c.as_ptr());

                glUniform3f(light_pos, light.position.x, light.position.y, light.position.z);
                glUniform3f(light_color, light.color.x, light.color.y, light.color.z);
                glUniform1f(light_intensity, light.intensity as f32);
            }

            // bind the gbuffer textures
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, self.backend.framebuffers.gbuffer_position as GLuint);
            let gbuffer_position_c = CString::new("position").unwrap();
            let gbuffer_position_loc = glGetUniformLocation(lighting_shader.program, gbuffer_position_c.as_ptr());
            glUniform1i(gbuffer_position_loc, 0);
            glActiveTexture(GL_TEXTURE1);
            glBindTexture(GL_TEXTURE_2D, self.backend.framebuffers.gbuffer_normal as GLuint);
            let gbuffer_normal_c = CString::new("normal").unwrap();
            let gbuffer_normal_loc = glGetUniformLocation(lighting_shader.program, gbuffer_normal_c.as_ptr());
            glUniform1i(gbuffer_normal_loc, 1);
            glActiveTexture(GL_TEXTURE2);
            glBindTexture(GL_TEXTURE_2D, self.backend.framebuffers.gbuffer_albedo as GLuint);
            let gbuffer_albedo_c = CString::new("albedospec").unwrap();
            let gbuffer_albedo_loc = glGetUniformLocation(lighting_shader.program, gbuffer_albedo_c.as_ptr());
            glUniform1i(gbuffer_albedo_loc, 2);
            glActiveTexture(GL_TEXTURE3);
            glBindTexture(GL_TEXTURE_2D, self.backend.framebuffers.gbuffer_info as GLuint);
            let gbuffer_info_c = CString::new("info").unwrap();
            let gbuffer_info_loc = glGetUniformLocation(lighting_shader.program, gbuffer_info_c.as_ptr());
            glUniform1i(gbuffer_info_loc, 3);

            // draw the quad
            glBindVertexArray(self.backend.framebuffers.screenquad_vao as GLuint);
            glDisable(GL_DEPTH_TEST);
            // make sure that gl doesn't cull the back face of the quad
            glDisable(GL_CULL_FACE);
            // draw the screen quad
            glDrawArrays(GL_TRIANGLES, 0, 6);
        }
    }

    // postprocessing pass
    fn setup_pass_three(&mut self) {
        let postbuffer_shader = *self.shaders.get("postbuffer").unwrap();

        set_shader_if_not_already(self, postbuffer_shader);
        unsafe {
            // set framebuffer to the default framebuffer
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            glViewport(0, 0, self.window_size.x as GLsizei, self.window_size.y as GLsizei);
            glClearColor(1.0, 0.0, 0.0, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);

            let shader  = self.backend.shaders.as_mut().unwrap().get_mut(postbuffer_shader).unwrap();
            // render the post processing framebuffer
            glBindVertexArray(self.backend.framebuffers.screenquad_vao as GLuint);
            glDisable(GL_DEPTH_TEST);

            // enable gamma correction
            glEnable(GL_FRAMEBUFFER_SRGB);

            // make sure that gl doesn't cull the back face of the quad
            glDisable(GL_CULL_FACE);

            // set texture uniform
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, self.backend.framebuffers.postbuffer_texture as GLuint);
            glUniform1i(glGetUniformLocation(shader.program, "u_texture\0".as_ptr() as *const GLchar), 0);
            // draw the screen quad
            glDrawArrays(GL_TRIANGLES, 0, 6);

            // unbind the texture
            glBindTexture(GL_TEXTURE_2D, 0);
            // unbind the vertex array
            glBindVertexArray(0);

            // print opengl errors
            let mut error = glGetError();
            while error != GL_NO_ERROR {
                error!("OpenGL error while rendering to postbuffer: {}", error);
                error = glGetError();
            }
        }
    }

}