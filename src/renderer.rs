use std::os::raw::c_uint;
use std::ptr::null_mut;
use crate::helpers::*;
#[cfg(target_os = "linux")]
use libsex::bindings::*;

#[derive(Copy, Clone)]
pub struct loc {
    pub x: i32,
    pub y: i32,
}

pub struct colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub enum RenderType {
    GLX,
}

#[cfg(target_os = "linux")]
pub struct X11backend {
    pub display: *mut Display,
    pub window: Window,
    pub ctx: GLXContext,
    pub current_mode: Option<GLenum>,
}

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

                let window = unsafe {
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


                    glClear(GL_COLOR_BUFFER_BIT);
                    glMatrixMode( GL_PROJECTION );
                    glLoadIdentity();
                    glViewport(0, 0, window_width as i32, window_height as i32);
                    gluOrtho2D(0.0, window_width as f64, 0.0, window_height as f64);

                    glLineWidth(10.0);
                }

                X11backend {
                    display,
                    window,
                    ctx,
                    current_mode: Option::None,
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
            self.put_line(point, loc { x: point.x + 1, y: point.y }, c);
        }
    }

    pub fn to_gl_coord(&self, point: loc) -> loc {
        let mut ret = point;
        // we use coords from top left, but opengl uses bottom left
        ret.y = self.window_size.y - ret.y;
        ret
    }
}