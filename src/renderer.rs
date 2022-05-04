use std::os::raw::c_uint;
use std::ptr::null_mut;
use crate::helpers::*;
#[cfg(target_os = "linux")]
use libsex::bindings::*;

pub enum RenderType {
    GLX,
}

#[cfg(target_os = "linux")]
pub struct X11backend {
    pub display: *mut Display,
    pub window: Window,
    pub ctx: GLXContext,
}

pub struct ht_renderer {
    pub type_: RenderType,
    #[cfg(target_os = "linux")] // X11 specifics (todo: add native wayland support)
    pub backend: X11backend,
}

impl ht_renderer {
    pub fn init() -> Result<ht_renderer, String> {
        // some constants we can later change (todo: make these configurable?)
        let window_width = 640;
        let window_height = 480;


        let backend = #[cfg(target_os = "linux")] {
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
                glViewport(0, 0, window_width as i32, window_height as i32);
            }

            X11backend {
                display,
                window,
                ctx,
            }
        };

        Err("not implemented".to_string())
    }
}