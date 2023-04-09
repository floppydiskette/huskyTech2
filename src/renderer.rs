use halfbrown::HashMap;
use std::ffi::{c_void, CString};
use std::io::{BufReader, Read};
use std::ops::{DerefMut};
use std::os::raw::{c_uint, c_ulong};
use std::ptr::{null_mut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::Receiver;
use egui_glfw_gl::egui;
use egui_glfw_gl::egui::Rect;
use gfx_maths::*;
use glad_gl::gl::*;
use glfw::{Context, Window, WindowEvent};
use crate::shaders::*;
use crate::camera::*;
use crate::helpers::{load_string_from_file, set_shader_if_not_already};
use crate::light::Light;
use crate::meshes::{IntermidiaryMesh, Mesh};
use crate::textures::{IntermidiaryTexture, Texture};
use crate::uimesh::UiMesh;

pub static MAX_LIGHTS: usize = 100;
pub static SHADOW_SIZE: usize = 1024;

#[derive(Clone, Copy)]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct AtomicRGBA {
    pub r: AtomicU8,
    pub g: AtomicU8,
    pub b: AtomicU8,
    pub a: AtomicU8,
}

impl AtomicRGBA {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: AtomicU8::new(r),
            g: AtomicU8::new(g),
            b: AtomicU8::new(b),
            a: AtomicU8::new(a),
        }
    }

    pub fn load(&self, order: std::sync::atomic::Ordering) -> RGBA {
        RGBA {
            r: self.r.load(order),
            g: self.g.load(order),
            b: self.b.load(order),
            a: self.a.load(order),
        }
    }

    pub fn store(&self, val: RGBA, order: std::sync::atomic::Ordering) {
        self.r.store(val.r, order);
        self.g.store(val.g, order);
        self.b.store(val.b, order);
        self.a.store(val.a, order);
    }
}

#[derive(Clone, Copy)]
pub enum RenderType {
    GLX,
}


#[derive(Clone)]
pub struct GLFWBackend {
    pub window: Arc<Mutex<Window>>,
    pub events: Arc<Mutex<Receiver<(f64, WindowEvent)>>>,
    pub clear_colour: Arc<AtomicRGBA>,
    pub active_vbo: Option<GLuint>,
    pub current_shader: Option<usize>,
    pub shaders: Option<Vec<Shader>>,
    pub ui_master: Arc<Mutex<Option<UiMesh>>>,
    pub framebuffers: Framebuffers,
    pub egui_context: Arc<Mutex<egui::Context>>,
    pub painter: Arc<Mutex<egui_glfw_gl::Painter>>,
    pub input_state: Arc<Mutex<egui_glfw_gl::EguiInputState>>,
}

#[derive(Clone)]
pub struct ht_renderer {
    pub type_: RenderType,
    pub window_size: Vec2,
    pub render_size: Vec2,
    pub camera: Camera,
    pub textures: HashMap<String, Texture>,
    pub loading_textures: HashMap<String, (Arc<AtomicBool>, Arc<Mutex<Option<IntermidiaryTexture>>>)>,
    pub meshes: HashMap<String, Mesh>,
    pub loading_meshes: HashMap<String, (Arc<AtomicBool>, Arc<Mutex<Option<IntermidiaryMesh>>>)>,
    pub shaders: HashMap<String, usize>,
    pub lights: Vec<Light>,
    
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
        let window_width = 1920;
        let window_height = 1080;
        let render_width = 1280;
        let render_height = 720;

        let camera = Camera::new(Vec2::new(window_width as f32, window_height as f32), 45.0, 0.1, 10000.0);

        {
            let backend = {
                info!("running on linux, using glfw as backend");
                unsafe {
                    let result = glfw::init(glfw::FAIL_ON_ERRORS);
                    if result.is_err() {
                        return Err("glfwInit failed".to_string());
                    }
                    let mut glfw = result.unwrap();
                    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
                    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
                    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
                    glfw.window_hint(glfw::WindowHint::DoubleBuffer(true));
                    glfw.window_hint(glfw::WindowHint::Resizable(true));
                    glfw.window_hint(glfw::WindowHint::Samples(Some(0)));

                    let (mut window, events) = glfw.create_window(
                        window_width,
                        window_height,
                        "huskyTech2",
                        glfw::WindowMode::Windowed)
                        .expect("Failed to create GLFW window.");

                    window.make_current();
                    window.set_key_polling(true);
                    window.set_sticky_keys(true);
                    window.set_cursor_pos_polling(true);
                    window.set_mouse_button_polling(true);
                    window.set_size(window_width as i32, window_height as i32);

                    load(|s| window.get_proc_address(s) as *const _);

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

                    Viewport(0, 0, render_width as i32, render_height as i32);

                    // get the number of the current framebuffer
                    let mut original: i32 = 0;
                    GetIntegerv(FRAMEBUFFER_BINDING, &mut original);
                    framebuffers.original = original as usize;
                    debug!("original framebuffer: {}", framebuffers.original);

                    // Configure culling
                    Enable(CULL_FACE);
                    CullFace(FRONT);
                    Enable(DEPTH_TEST);
                    DepthFunc(LESS);

                    // disable multisampling
                    Disable(MULTISAMPLE);

                    // configure stencil test
                    Enable(STENCIL_TEST);

                    // enable blending
                    Enable(BLEND);
                    BlendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);

                    // create the postprocessing framebuffer
                    let mut postbuffer = 0;
                    GenFramebuffers(1, &mut postbuffer);
                    BindFramebuffer(FRAMEBUFFER, postbuffer);
                    let mut posttexture = 0;
                    GenTextures(1, &mut posttexture);
                    BindTexture(TEXTURE_2D, posttexture);
                    TexImage2D(TEXTURE_2D, 0, SRGB as i32, render_width, render_height, 0, RGB, UNSIGNED_BYTE, std::ptr::null());
                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    FramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, posttexture, 0);
                    // create a renderbuffer object for depth and stencil attachment (we won't be sampling these)
                    let mut renderbuffer = 0;
                    GenRenderbuffers(1, &mut renderbuffer);
                    BindRenderbuffer(RENDERBUFFER, renderbuffer);
                    RenderbufferStorage(RENDERBUFFER, DEPTH24_STENCIL8, render_width, render_height);
                    FramebufferRenderbuffer(FRAMEBUFFER, DEPTH_STENCIL_ATTACHMENT, RENDERBUFFER, renderbuffer);

                    // check if framebuffer is complete
                    if CheckFramebufferStatus(FRAMEBUFFER) != FRAMEBUFFER_COMPLETE {
                        panic!("framebuffer is not complete!");
                    }
                    framebuffers.postbuffer = postbuffer as usize;
                    framebuffers.postbuffer_texture = posttexture as usize;
                    framebuffers.postbuffer_rbuffer = renderbuffer as usize;

                    // create a simple quad that fills the screen
                    let mut screenquad_vao = 0;
                    GenVertexArrays(1, &mut screenquad_vao);
                    BindVertexArray(screenquad_vao);
                    let mut screenquad_vbo = 0;
                    GenBuffers(1, &mut screenquad_vbo);
                    BindBuffer(ARRAY_BUFFER, screenquad_vbo);
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
                    BufferData(ARRAY_BUFFER, (quad_vertices.len() * std::mem::size_of::<f32>()) as GLsizeiptr, quad_vertices.as_ptr() as *const c_void, STATIC_DRAW);
                    // as this is such a simple quad, we're not gonna bother with indices
                    EnableVertexAttribArray(0);
                    VertexAttribPointer(0, 3, FLOAT, FALSE as GLboolean, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
                    EnableVertexAttribArray(1);
                    VertexAttribPointer(1, 2, FLOAT, FALSE as GLboolean, 5 * std::mem::size_of::<f32>() as i32, (3 * std::mem::size_of::<f32>()) as *const c_void);
                    framebuffers.screenquad_vao = screenquad_vao as usize;

                    // create the depth framebuffer
                    let mut depthbuffer = 0;
                    GenFramebuffers(1, &mut depthbuffer);
                    BindFramebuffer(FRAMEBUFFER, depthbuffer);
                    let mut depthtexture = 0;
                    GenTextures(1, &mut depthtexture);
                    BindTexture(TEXTURE_2D, depthtexture);
                    TexImage2D(TEXTURE_2D, 0, DEPTH_COMPONENT as i32, SHADOW_SIZE as i32, SHADOW_SIZE as i32, 0, DEPTH_COMPONENT, FLOAT, std::ptr::null());
                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    FramebufferTexture2D(FRAMEBUFFER, DEPTH_ATTACHMENT, TEXTURE_2D, depthtexture, 0);
                    DrawBuffer(NONE);
                    ReadBuffer(NONE);
                    if CheckFramebufferStatus(FRAMEBUFFER) != FRAMEBUFFER_COMPLETE {
                        panic!("framebuffer is not complete (depth buffer)!");
                    }

                    framebuffers.depthbuffer = depthbuffer as usize;
                    framebuffers.depthbuffer_texture = depthtexture as usize;

                    // create the gbuffer
                    let mut gbuffer = 0;
                    GenFramebuffers(1, &mut gbuffer);
                    BindFramebuffer(FRAMEBUFFER, gbuffer);
                    let mut gbuffer_textures = [0; 4];
                    GenTextures(4, gbuffer_textures.as_mut_ptr());

                    // position
                    BindTexture(TEXTURE_2D, gbuffer_textures[0]);
                    TexImage2D(TEXTURE_2D, 0, RGBA16F as i32, render_width, render_height, 0, RGBA, FLOAT, std::ptr::null());
                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);
                    FramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, gbuffer_textures[0], 0);
                    // normal
                    BindTexture(TEXTURE_2D, gbuffer_textures[1]);
                    TexImage2D(TEXTURE_2D, 0, RGBA8 as i32, render_width, render_height, 0, RGBA, UNSIGNED_BYTE, std::ptr::null());
                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);
                    FramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT1, TEXTURE_2D, gbuffer_textures[1], 0);
                    // color
                    BindTexture(TEXTURE_2D, gbuffer_textures[2]);
                    TexImage2D(TEXTURE_2D, 0, RGBA as i32, render_width, render_height, 0, RGBA, UNSIGNED_BYTE, std::ptr::null());
                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);
                    FramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT2, TEXTURE_2D, gbuffer_textures[2], 0);
                    // info
                    BindTexture(TEXTURE_2D, gbuffer_textures[3]);
                    TexImage2D(TEXTURE_2D, 0, RGBA as i32, render_width, render_height, 0, RGBA, UNSIGNED_BYTE, std::ptr::null());
                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, NEAREST as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, NEAREST as i32);
                    FramebufferTexture2D(FRAMEBUFFER, COLOR_ATTACHMENT3, TEXTURE_2D, gbuffer_textures[3], 0);

                    let attachments = [COLOR_ATTACHMENT0, COLOR_ATTACHMENT1, COLOR_ATTACHMENT2, COLOR_ATTACHMENT3];
                    DrawBuffers(4, attachments.as_ptr());

                    if CheckFramebufferStatus(FRAMEBUFFER) != FRAMEBUFFER_COMPLETE {
                        panic!("framebuffer is not complete (gbuffer)!");
                    }

                    framebuffers.gbuffer = gbuffer as usize;
                    framebuffers.gbuffer_position = gbuffer_textures[0] as usize;
                    framebuffers.gbuffer_normal = gbuffer_textures[1] as usize;
                    framebuffers.gbuffer_albedo = gbuffer_textures[2] as usize;
                    framebuffers.gbuffer_info = gbuffer_textures[3] as usize;

                    // renderbuffer for gbuffer
                    let mut gbuffer_renderbuffer = 0;
                    GenRenderbuffers(1, &mut gbuffer_renderbuffer);
                    BindRenderbuffer(RENDERBUFFER, gbuffer_renderbuffer);
                    RenderbufferStorage(RENDERBUFFER, DEPTH_COMPONENT, render_width, render_height);
                    FramebufferRenderbuffer(FRAMEBUFFER, DEPTH_ATTACHMENT, RENDERBUFFER, gbuffer_renderbuffer);
                    // make top left corner as origin

                    Clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT);

                    // print opengl errors
                    let mut error = GetError();
                    while error != NO_ERROR {
                        error!("OpenGL error while initialising render subsystem: {}", error);
                        error = GetError();
                    }

                    // setup egui

                    let native_ppp = window.get_content_scale().0;
                    let mut painter = egui_glfw_gl::Painter::new(&mut window);
                    let egui_ctx = egui::Context::default();
                    let mut egui_input_state = egui_glfw_gl::EguiInputState::new(egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(window_width as f32, window_height as f32))),
                        pixels_per_point: Some(native_ppp),
                        ..Default::default()
                    });

                    GLFWBackend {
                        window: Arc::new(Mutex::new(window)),
                        events: Arc::new(Mutex::new(events)),
                        current_shader: Option::None,
                        clear_colour: Arc::new(AtomicRGBA::new(0, 0, 0, 255)),
                        active_vbo: Option::None,
                        shaders: Option::None,
                        ui_master: Arc::new(Mutex::new(None)),
                        framebuffers,
                        egui_context: Arc::new(Mutex::new(egui_ctx)),
                        painter: Arc::new(Mutex::new(painter)),
                        input_state: Arc::new(Mutex::new(egui_input_state)),
                    }
                }
            };

            Ok(ht_renderer {
                type_: RenderType::GLX,
                window_size: Vec2::new(window_width as f32, window_height as f32),
                render_size: Vec2::new(render_width as f32, render_height as f32),
                camera,
                textures: Default::default(),
                loading_textures: Default::default(),
                meshes: Default::default(),
                loading_meshes: Default::default(),
                shaders: Default::default(),
                lights: Vec::new(),
                backend,
            })
        }
    }

    pub fn lock_mouse(&mut self, lock: bool) {
        unsafe {
            
            {
                if lock {
                    self.backend.window.lock().unwrap().set_cursor_mode(glfw::CursorMode::Disabled);
                } else {
                    self.backend.window.lock().unwrap().set_cursor_mode(glfw::CursorMode::Normal);
                }
            }
        }
    }

    pub fn initialise_basic_resources(&mut self) {
        // load postbuffer shader
        self.load_shader("postbuffer").expect("failed to load postbuffer shader");
        // load gbuffer shader
        //self.load_shader("gbuffer").expect("failed to load gbuffer shader");
        // load gbuffer animation shader
        self.load_shader("gbuffer_anim").expect("failed to load gbuffer animation shader");
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
        self.backend.ui_master = ui_master;
        // load default texture
        self.load_texture_if_not_already_loaded_synch("default").expect("failed to load default texture");
    }

    pub fn load_texture_if_not_already_loaded(&mut self, name: &str) -> Result<bool, crate::textures::TextureError> {
        if !self.textures.contains_key(name) {
            let (texture_done, int_texture_container) = {
                if !self.loading_textures.contains_key(name) {
                    let (done, container) = Texture::new_from_name_asynch_begin(name);
                    self.loading_textures.insert(name.to_string(), (done.clone(), container.clone()));
                    (done, container)
                } else {
                    self.loading_textures.get(name).unwrap().clone()
                }
            };
            if texture_done.load(Ordering::Relaxed) {
                let final_texture = int_texture_container.lock().unwrap().take();
                let final_texture = Texture::load_from_intermidiary(final_texture)?;
                self.textures.insert(name.to_string(), final_texture);
                self.loading_meshes.remove(name);
                return Ok(true)
            } else {
                return Ok(false)
            }
        }
        Ok(true)
    }

    pub fn load_texture_if_not_already_loaded_synch(&mut self, name: &str) -> Result<(), crate::textures::TextureError> {
        if !self.textures.contains_key(name) {
            let texture = Texture::new_from_name(name)?;
            self.textures.insert(name.to_string(), texture);
        }
        Ok(())
    }

    /// returns true if the mesh was loaded, false if it is still loading
    pub fn load_mesh_if_not_already_loaded(&mut self, name: &str) -> Result<bool, crate::meshes::MeshError> {
        if !self.meshes.contains_key(name) {
            let (mesh_done, int_mesh_container) = {
                if !self.loading_meshes.contains_key(name) {
                    let (done, container) = Mesh::new_from_name_asynch_begin(format!("base/models/{}.glb", name).as_str(), name);
                    self.loading_meshes.insert(name.to_string(), (done.clone(), container.clone()));
                    (done, container)
                } else {
                    self.loading_meshes.get(name).unwrap().clone()
                }
            };
            // unlikely, but check if the mesh is already done
            if mesh_done.load(Ordering::Relaxed) {
                let final_mesh = int_mesh_container.lock().unwrap().take();
                let final_mesh = Mesh::load_from_intermidiary(final_mesh, self)?;
                self.meshes.insert(name.to_string(), final_mesh);
                self.loading_meshes.remove(name);
                return Ok(true);
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn load_mesh_if_not_already_loaded_synch(&mut self, name: &str) -> Result<(), crate::meshes::MeshError> {
        if !self.meshes.contains_key(name) {
            let mesh = Mesh::new(format!("base/models/{}.glb", name).as_str(), name, self)?;
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
        {
            unsafe {
                self.backend.window.lock().unwrap().glfw.poll_events();
                if self.backend.window.lock().unwrap().should_close() {
                    return true;
                }
            }
            false
        }
    }

    pub fn load_shader(&mut self, shader_name: &str) -> Result<usize, String> {
        // read the files
        let vert_source = load_string_from_file(format!("base/shaders/{}.vert", shader_name)).expect("failed to load vertex shader");
        let frag_source = load_string_from_file(format!("base/shaders/{}.frag", shader_name)).expect("failed to load fragment shader");

        // convert strings to c strings
        let vert_source_c = CString::new(vert_source).unwrap();
        let frag_source_c = CString::new(frag_source).unwrap();

        // create the shaders
        let vert_shader = unsafe { CreateShader(VERTEX_SHADER) };
        let frag_shader = unsafe { CreateShader(FRAGMENT_SHADER) };

        // set the source
        unsafe {
            ShaderSource(vert_shader, 1, &vert_source_c.as_ptr(), null_mut());
            ShaderSource(frag_shader, 1, &frag_source_c.as_ptr(), null_mut());
        }

        // compile the shaders
        unsafe {
            CompileShader(vert_shader);
            CompileShader(frag_shader);
        }

        // check if the shaders compiled
        let mut status = 0;
        unsafe {
            GetShaderiv(vert_shader, COMPILE_STATUS, &mut status);
            if status == 0 {
                let mut len = 255;
                GetShaderiv(vert_shader, INFO_LOG_LENGTH, &mut len);
                let log = vec![0; len as usize + 1];
                let log_c = CString::from_vec_unchecked(log);
                let log_p = log_c.into_raw();
                GetShaderInfoLog(vert_shader, len, null_mut(), log_p);
                return Err(format!("failed to compile vertex shader: {}", CString::from_raw(log_p).to_string_lossy()));
            }
            GetShaderiv(frag_shader, COMPILE_STATUS, &mut status);
            if status == 0 {
                let mut len = 255;
                GetShaderiv(frag_shader, INFO_LOG_LENGTH, &mut len);
                let log = vec![0; len as usize + 1];
                let log_c = CString::from_vec_unchecked(log);
                let log_p = log_c.into_raw();
                GetShaderInfoLog(frag_shader, len, null_mut(), log_p);
                return Err(format!("failed to compile fragment shader: {}", CString::from_raw(log_p).to_string_lossy()));
            }
        }

        // link the shaders
        let shader_program = unsafe { CreateProgram() };
        unsafe {
            AttachShader(shader_program, vert_shader);
            AttachShader(shader_program, frag_shader);
            LinkProgram(shader_program);
        }

        // check if the shaders linked
        unsafe {
            GetProgramiv(shader_program, LINK_STATUS, &mut status);
            if status == 0 {
                let mut len = 0;
                GetProgramiv(shader_program, INFO_LOG_LENGTH, &mut len);
                let mut log = Vec::with_capacity(len as usize);
                GetProgramInfoLog(shader_program, len, null_mut(), log.as_mut_ptr() as *mut GLchar);
                return Err(format!("failed to link shader program: {}", std::str::from_utf8(&log).unwrap()));
            }
        }

        // clean up
        unsafe {
            DeleteShader(vert_shader);
            DeleteShader(frag_shader);
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
        /* egui */

        crate::ui::render(self);

        unsafe {
            self.backend.window.lock().unwrap().swap_buffers();
            let mut width = 0;
            let mut height = 0;
            (width, height) = self.backend.window.lock().unwrap().get_framebuffer_size();
            self.window_size = Vec2::new(width as f32, height as f32);
            self.backend.input_state.lock().unwrap().input.screen_rect = Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width as f32, height as f32)))
        }
        self.setup_pass_one();
    }

    // geometry pass
    fn setup_pass_one(&mut self) {
        let gbuffer_shader = *self.shaders.get("gbuffer_anim").unwrap();

        set_shader_if_not_already(self, gbuffer_shader);

        unsafe {
            Viewport(0, 0, self.render_size.x as i32, self.render_size.y as i32);

            // set framebuffer to the post processing framebuffer
            BindFramebuffer(FRAMEBUFFER, self.backend.framebuffers.gbuffer as GLuint);

            Enable(CULL_FACE);
            CullFace(FRONT);
            Enable(DEPTH_TEST);
            DepthFunc(LESS);

            // disable gamma correction
            Disable(FRAMEBUFFER_SRGB);

            // set the clear color to black
            ClearColor(0.0, 0.0, 0.0, 1.0);
            Clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT);
        }
    }

    // lighting pass
    fn setup_pass_two(&mut self) {
        let lighting_shader = *self.shaders.get("lighting").unwrap();

        set_shader_if_not_already(self, lighting_shader);

        let lighting_shader = self.backend.shaders.as_ref().unwrap().get(lighting_shader).unwrap();

        unsafe {
            // set framebuffer to the post processing framebuffer
            BindFramebuffer(FRAMEBUFFER, self.backend.framebuffers.postbuffer as GLuint);
            Viewport(0, 0, self.render_size.x as i32, self.render_size.y as i32);

            // set the clear color to preferred color
            let colour = self.backend.clear_colour.load(Ordering::Relaxed);
            ClearColor(colour.r as f32 / 255.0, colour.g as f32 / 255.0, colour.b as f32 / 255.0, 1.0);
            Clear(COLOR_BUFFER_BIT);

            // disable srgb
            Enable(FRAMEBUFFER_SRGB);

            // send the lights to the shader
            let light_count = self.lights.len();
            let light_count = if light_count > MAX_LIGHTS { MAX_LIGHTS } else { light_count };
            let light_count_c = CString::new("u_light_count").unwrap();
            let light_count_loc = GetUniformLocation(lighting_shader.program, light_count_c.as_ptr());
            Uniform1i(light_count_loc, light_count as i32);
            for (i, light) in self.lights.iter().enumerate() {
                if i >= MAX_LIGHTS { break; }
                let light_pos_c = CString::new(format!("u_lights[{}].position", i)).unwrap();
                let light_pos = GetUniformLocation(lighting_shader.program, light_pos_c.as_ptr());
                let light_colour_c = CString::new(format!("u_lights[{}].colour", i)).unwrap();
                let light_color = GetUniformLocation(lighting_shader.program, light_colour_c.as_ptr());
                let light_intensity_c = CString::new(format!("u_lights[{}].intensity", i)).unwrap();
                let light_intensity = GetUniformLocation(lighting_shader.program, light_intensity_c.as_ptr());

                Uniform3f(light_pos, light.position.x, light.position.y, light.position.z);
                Uniform3f(light_color, light.color.x, light.color.y, light.color.z);
                Uniform1f(light_intensity, light.intensity as f32);
            }

            // bind the gbuffer textures
            ActiveTexture(TEXTURE0);
            BindTexture(TEXTURE_2D, self.backend.framebuffers.gbuffer_position as GLuint);
            let gbuffer_position_c = CString::new("position").unwrap();
            let gbuffer_position_loc = GetUniformLocation(lighting_shader.program, gbuffer_position_c.as_ptr());
            Uniform1i(gbuffer_position_loc, 0);
            ActiveTexture(TEXTURE1);
            BindTexture(TEXTURE_2D, self.backend.framebuffers.gbuffer_normal as GLuint);
            let gbuffer_normal_c = CString::new("normal").unwrap();
            let gbuffer_normal_loc = GetUniformLocation(lighting_shader.program, gbuffer_normal_c.as_ptr());
            Uniform1i(gbuffer_normal_loc, 1);
            ActiveTexture(TEXTURE2);
            BindTexture(TEXTURE_2D, self.backend.framebuffers.gbuffer_albedo as GLuint);
            let gbuffer_albedo_c = CString::new("albedospec").unwrap();
            let gbuffer_albedo_loc = GetUniformLocation(lighting_shader.program, gbuffer_albedo_c.as_ptr());
            Uniform1i(gbuffer_albedo_loc, 2);
            ActiveTexture(TEXTURE3);
            BindTexture(TEXTURE_2D, self.backend.framebuffers.gbuffer_info as GLuint);
            let gbuffer_info_c = CString::new("info").unwrap();
            let gbuffer_info_loc = GetUniformLocation(lighting_shader.program, gbuffer_info_c.as_ptr());
            Uniform1i(gbuffer_info_loc, 3);
            // send camera position to the shader
            let camera_pos_c = CString::new("u_camera_pos").unwrap();
            let camera_pos_loc = GetUniformLocation(lighting_shader.program, camera_pos_c.as_ptr());
            let pos = self.camera.get_position();
            Uniform3f(camera_pos_loc, pos.x, pos.y, pos.z);

            // draw the quad
            BindVertexArray(self.backend.framebuffers.screenquad_vao as GLuint);
            Disable(DEPTH_TEST);
            // make sure that gl doesn't cull the back face of the quad
            Disable(CULL_FACE);
            // draw the screen quad
            DrawArrays(TRIANGLES, 0, 6);
        }
    }

    // postprocessing pass
    fn setup_pass_three(&mut self) {
        let postbuffer_shader = *self.shaders.get("postbuffer").unwrap();

        set_shader_if_not_already(self, postbuffer_shader);
        unsafe {
            // set framebuffer to the default framebuffer
            BindFramebuffer(FRAMEBUFFER, 0);
            Viewport(0, 0, self.window_size.x as GLsizei, self.window_size.y as GLsizei);
            ClearColor(1.0, 0.0, 0.0, 1.0);
            Clear(COLOR_BUFFER_BIT);

            let shader  = self.backend.shaders.as_mut().unwrap().get_mut(postbuffer_shader).unwrap();
            // render the post processing framebuffer
            BindVertexArray(self.backend.framebuffers.screenquad_vao as GLuint);
            Disable(DEPTH_TEST);

            // enable gamma correction
            Enable(FRAMEBUFFER_SRGB);

            // make sure that gl doesn't cull the back face of the quad
            Disable(CULL_FACE);

            // set texture uniform
            ActiveTexture(TEXTURE0);
            BindTexture(TEXTURE_2D, self.backend.framebuffers.postbuffer_texture as GLuint);
            Uniform1i(GetUniformLocation(shader.program, "u_texture\0".as_ptr() as *const GLchar), 0);
            // draw the screen quad
            DrawArrays(TRIANGLES, 0, 6);

            // unbind the texture
            BindTexture(TEXTURE_2D, 0);
            // unbind the vertex array
            BindVertexArray(0);

            // print open gl errors
            let mut error = GetError();
            while error != NO_ERROR {
                error!("OpenGL error while rendering to postbuffer: {}", error);
                error = GetError();
            }
        }
    }

}