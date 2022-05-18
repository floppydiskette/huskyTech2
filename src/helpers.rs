use std::io::Read;
use std::os::raw::c_int;
use std::ptr::null_mut;
use crate::renderer::Colour;

#[cfg(target_os = "linux")]
use libsex::bindings::*;

#[cfg(target_os = "linux")]
pub unsafe fn get_window_fb_config(window: Window, display: *mut Display, screen: *mut Screen) -> GLXFBConfig { //todo: handle errors better
    let mut attrib = XWindowAttributes {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        border_width: 0,
        depth: 0,
        visual: null_mut(),
        root: 0,
        class: 0,
        bit_gravity: 0,
        win_gravity: 0,
        backing_store: 0,
        backing_planes: 0,
        backing_pixel: 0,
        save_under: 0,
        colormap: 0,
        map_installed: 0,
        map_state: 0,
        all_event_masks: 0,
        your_event_mask: 0,
        do_not_propagate_mask: 0,
        override_redirect: 0,
        screen
    };
    XGetWindowAttributes(display, window, &mut attrib);
    let visualid = XVisualIDFromVisual(attrib.visual);
    let mut visinfo: *mut XVisualInfo = null_mut();
    let mut wanted_config = 0;
    let mut value: c_int = 0;
    let mut nfbconfigs: *mut c_int = Box::into_raw(Box::new(0)) as *mut c_int;
    let fbconfigs = glXGetFBConfigs(display, 0, nfbconfigs);
    XSync(display, 0);
    //println!("{}", *nfbconfigs);
    if fbconfigs.is_null() {
        panic!("could not get fbconfigs");
    }
    for i in 0..*nfbconfigs {
        visinfo = glXGetVisualFromFBConfig (display, *fbconfigs.offset(i as isize));
        if visinfo.is_null() || (*visinfo).visualid != visualid as u64 {
            continue;
        }

        // check if fbconfig supports drawing
        glXGetFBConfigAttrib(display, *fbconfigs.offset(i as isize), GLX_DRAWABLE_TYPE as c_int, &mut value);
        if value & GLX_PIXMAP_BIT as c_int == 0 {
            continue;
        }

        // check if fbconfig supports GLX_BIND_TO_TEXTURE_TARGETS_EXT
        glXGetFBConfigAttrib(display, *fbconfigs.offset(i as isize), GLX_BIND_TO_TEXTURE_TARGETS_EXT as c_int, &mut value);
        if value & GLX_TEXTURE_2D_BIT_EXT as c_int == 0 {
            continue;
        }

        // check if fbconfig supports GLX_BIND_TO_TEXTURE_RGBA_EXT
        glXGetFBConfigAttrib(display, *fbconfigs.offset(i as isize), GLX_BIND_TO_TEXTURE_RGBA_EXT as c_int, &mut value);
        if value & GLX_RGBA_BIT as c_int == 0 {
            // check if fbconfig supports GLX_BIND_TO_TEXTURE_RGB_EXT
            glXGetFBConfigAttrib(display, *fbconfigs.offset(i as isize), GLX_BIND_TO_TEXTURE_RGB_EXT as c_int, &mut value);
            if value & GLX_RGBA_BIT as c_int == 0 {
                continue;
            }
        }

        wanted_config = i;
        break;
    }

    // consume
    Box::from_raw(nfbconfigs);

    //println!("{}", wanted_config);

    *fbconfigs.offset(wanted_config as isize)
}

pub fn gen_rainbow(time: f64) -> Colour {
    let frequency = 0.05;
    let r = ((frequency * (time as f64) + 0.0).sin() * 127.0f64 + 128.0f64);
    let g = ((frequency * (time as f64) + 2.0).sin() * 127.0f64 + 128.0f64);
    let b = ((frequency * (time as f64) + 4.0).sin() * 127.0f64 + 128.0f64);
    Colour { r: (r) as u8, g: (g) as u8, b: (b) as u8, a: 255 }
}

pub fn load_string_from_file(path: String) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|e| e.to_string())?;
    Ok(contents)
}