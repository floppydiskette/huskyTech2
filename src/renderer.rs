use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_uint;
use std::ptr::null_mut;
use dae_parser::{Document, Geometry};
use crate::helpers::*;
#[cfg(target_os = "linux")]
use libsex::bindings::*;

#[derive(Copy, Clone)]
pub struct loc {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy)]
pub struct colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct Mesh {
    pub vbo: GLuint,
    pub data: Vec<f32>,
}

#[derive(Clone, Copy)]
pub enum RenderType {
    GLX,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub struct X11backend {
    pub display: *mut Display,
    pub window: Window,
    pub ctx: GLXContext,
    pub current_mode: Option<GLenum>,
    pub active_vbo: Option<GLuint>,
}

#[derive(Clone, Copy)]
pub struct ht_renderer {
    pub type_: RenderType,
    pub window_size: loc,
    #[cfg(target_os = "linux")] // X11 specifics (todo: add native wayland support)
    pub backend: X11backend,
}

impl ht_renderer {
    pub fn init() -> Result<ht_renderer, String> {
        // some constants we can later change (todo: make these configurable?)
        let window_width = 640;
        let window_height = 480;

        #[cfg(target_os = "linux")]{
            let backend = {
                println!("running on linux, using glx as backend");
                let display = unsafe { XOpenDisplay(null_mut()) };
                if display.is_null() {
                    return Err("failed to open display".to_string());
                }
                let root = unsafe { XDefaultRootWindow(display) };
                // get size of root window so we can centre our window
                let mut root_size: XWindowAttributes = unsafe { std::mem::zeroed() };
                unsafe { XGetWindowAttributes(display, root, &mut root_size) };
                let width = root_size.width;
                let height = root_size.height;

                let window_x = width / 2 - window_width / 2;
                let window_y = height / 2 - window_height / 2;

                let window = unsafe { // todo: make it so that the window is not resizable
                    XCreateSimpleWindow(display, root, window_x, window_y, window_width as c_uint, window_height as c_uint, 0, 0, 0)
                };

                let screen = unsafe { XDefaultScreenOfDisplay(display) }; // we need this to get the fbconfig
                let fbconfig = unsafe { get_window_fb_config(window, display, screen) };
                let visinfo = unsafe { glXGetVisualFromFBConfig(display, fbconfig) };
                let ctx = unsafe { glXCreateContext(display, visinfo, null_mut(), 1i32) };
                if ctx.is_null() {
                    return Err("failed to create context".to_string());
                }
                unsafe {
                    XMapWindow(display, window);
                    XSync(display, 0);
                    glXMakeCurrent(display, window, ctx);


                    glViewport(0, 0, window_width as i32, window_height as i32);
                    glMatrixMode(GL_PROJECTION);
                    glLoadIdentity();
                    // make top left corner as origin
                    //glOrtho(0.0, src_width as f64, src_height as f64, 0.0, -1.0, 1.0);
                    //gluOrtho2D(0.0, window_width as f64, window_height as f64, 0.0);

                    glLineWidth(2.0);

                    // load an example shader
                    let vert_source = include_str!("../base/shaders/example.vert");
                    let frag_source = include_str!("../base/shaders/example.frag");

                    // convert strings to c strings
                    let vert_source_c = CString::new(vert_source).unwrap();
                    let frag_source_c = CString::new(frag_source).unwrap();

                    let vert_shader = glCreateShader(GL_VERTEX_SHADER);
                    let frag_shader = glCreateShader(GL_FRAGMENT_SHADER);

                    glShaderSource(vert_shader, 1, &vert_source_c.as_ptr(), null_mut());
                    glShaderSource(frag_shader, 1, &frag_source_c.as_ptr(), null_mut());

                    glCompileShader(vert_shader);
                    glCompileShader(frag_shader);

                    let shader_program = glCreateProgram();

                    glAttachShader(shader_program, vert_shader);
                    glAttachShader(shader_program, frag_shader);

                    glBindAttribLocation(shader_program, 0, CString::new("in_Position").unwrap().as_ptr());

                    glLinkProgram(shader_program);

                    // todo: for testing
                    glUseProgram(shader_program);

                    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
                }

                X11backend {
                    display,
                    window,
                    ctx,
                    current_mode: Option::None,
                    active_vbo: Option::None,
                }
            };

            Ok(ht_renderer {
                type_: RenderType::GLX,
                window_size: loc { x: window_width, y: window_height },
                backend,
            })
        }
        // if backend is null, we're on windows (error for now)
        #[cfg(not(target_os = "linux"))]
        {
            return Err("not implemented on windows".to_string());
        }
    }

    pub fn swap_buffers(&mut self) {
        #[cfg(target_os = "linux")]
        {
            unsafe {
                if self.backend.current_mode != Option::None {
                    glEnd();
                    self.backend.current_mode = Option::None;
                }
                glXSwapBuffers(self.backend.display, self.backend.window);
            }
        }
    }

    pub fn put_line(&mut self, point1: loc, point2: loc, c: colour) {
        #[cfg(target_os = "linux")]
        {
            unsafe {
                // check if we're already in GL_LINES mode
                if self.backend.current_mode != Option::Some(GL_LINES) {
                    if self.backend.current_mode != Option::None {
                        glEnd();
                    }
                    glBegin(GL_LINES);
                    self.backend.current_mode = Option::Some(GL_LINES);
                }
                glColor4ub(c.r, c.g, c.b, c.a);
                glVertex2i(point1.x, point1.y);
                glVertex2i(point2.x, point2.y);
            }
        }
    }
    pub fn put_pixel(&mut self, point: loc, c: colour) {
        #[cfg(target_os = "linux")]
        {
            // this is a bit of a hack,
            // we use put_line to draw a single pixel by setting the end point to the same point
            // + 1x cause it doesn't render unless the end point is different
            self.put_line(point, loc { x: point.x + 10, y: point.y + 10 }, c);
        }
    }

    pub fn to_gl_coord(&self, point: loc) -> loc {
        let mut ret = point;
        // we use coords from top left, but opengl uses bottom left
        //ret.y = self.window_size.y - ret.y;
        ret
    }

    pub fn initMesh(&mut self, doc: Document, mesh_name: &str) -> Result<Mesh, String> {
        let mesh = doc.local_map::<Geometry>().expect("mesh not found").get_str(&*mesh_name).unwrap();
        let mesh = mesh.element.as_mesh().expect("NO MESH?"); // this is a reference to the no bitches meme
        let vertices = mesh.elements[0].vertices.clone();

        // get the u32 data from the mesh
        let data = triangles.data.as_deref().unwrap();
        let mut vbo = 0 as GLuint;
        unsafe {
            glGenBuffers(1, &mut vbo);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            glBufferData(GL_ARRAY_BUFFER, data.len() as GLsizeiptr, data.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
            // stuff for shaders (following wikipedia code for now)
            glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null_mut());
            glEnableVertexAttribArray(0);
            glBindBuffer(GL_ARRAY_BUFFER, 0); // not sure if this is needed
        }
        Ok(Mesh {
            vbo,
            data: data.to_vec(),
        })
    }

    pub fn render_mesh(&mut self, mesh: Mesh) {
        if self.backend.active_vbo != Some(mesh.vbo) {
            unsafe {
                glBindBuffer(GL_ARRAY_BUFFER, mesh.vbo);
                self.backend.active_vbo = Some(mesh.vbo);
            }
        }
        unsafe {
            glDrawArrays(GL_TRIANGLES, 0, 3);
        }
    }

    // creates a vbo with a single triangle for testing
    pub fn gen_testing_triangle(&mut self) -> Mesh {
        let mut vbo = 0 as GLuint;
        let buffer_data = [
            -1.0, -1.0, 0.0,
            1.0, -1.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        unsafe {
            glGenBuffers(1, &mut vbo);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);
            glBufferData(GL_ARRAY_BUFFER, buffer_data.len() as GLsizeiptr, buffer_data.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
            // stuff for shaders (following wikipedia code for now)
            glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null_mut());
            glEnableVertexAttribArray(0);
            glBindBuffer(GL_ARRAY_BUFFER, 0); // not sure if this is needed
        };
        Mesh {
            vbo,
            data: buffer_data.to_vec(),
        }
    }
}