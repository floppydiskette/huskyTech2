// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use std::ffi::CString;
use std::process;
use std::ptr::null;
use std::time::SystemTime;
use dae_parser::Document;
use gfx_maths::{Quaternion, Vec2, Vec3};
use kira::manager::AudioManager;
use kira::manager::backend::cpal::CpalBackend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use libsex::bindings::*;
use crate::animation::Animation;
use crate::helpers::gen_rainbow;
use crate::renderer::{Colour, ht_renderer};
use crate::uimesh::UiMesh;

pub fn animate(renderer: &mut ht_renderer, sss: &mut AudioManager<CpalBackend>) {
    // load rainbow shader
    let rainbow_shader = renderer.load_shader("rainbow").expect("failed to load rainbow shader");
    // load red shader
    let red_shader = renderer.load_shader("red").expect("failed to load red shader");
    // load basic shader
    let basic_shader = renderer.load_shader("basic").expect("failed to load basic shader");
    // load ht2-mesh logo model
    let document = Document::from_file("base/models/ht2.dae").expect("failed to load dae file");
    let mut mesh = renderer.initMesh(document, "ht2-mesh", basic_shader, true).expect("failed to load ht2 mesh");
    // load master uimesh
    let mut ui_master = UiMesh::new_master(renderer, basic_shader).expect("failed to load master uimesh");
    // load poweredby uimesh
    let mut ui_poweredby = UiMesh::new_element_from_name("poweredby", &ui_master, renderer, basic_shader).expect("failed to load poweredby uimesh");
    // load developedby uimesh
    let mut ui_developedby = UiMesh::new_element_from_name("developedby", &ui_master, renderer, basic_shader).expect("failed to load developedby uimesh");

    let poweredby_width = renderer.window_size.y / 2.0;
    let poweredby_height = poweredby_width / 2.0;
    let poweredby_x = 15.0;
    let poweredby_y = renderer.window_size.y - poweredby_height - 15.0;
    ui_poweredby.position = Vec2::new(poweredby_x, poweredby_y);
    ui_poweredby.scale = Vec2::new(poweredby_width, poweredby_height);
    ui_poweredby.opacity = 0.0;

    ui_developedby.scale = renderer.window_size;

    let mut sunlust_sfx = StaticSoundData::from_file("base/snd/sunlust.wav", StaticSoundSettings::default()).expect("failed to load sunlust.wav");

    // wait 2 seconds
    std::thread::sleep(std::time::Duration::from_millis(1000));

    sss.play(sunlust_sfx.clone());
    debug!("playing sunlust.wav");
    let time_of_start = SystemTime::now(); // when the animation started
    let mut current_time = SystemTime::now(); // for later
    let rainbow_time = 1032.0; // in milliseconds
    let rainbow_anim = Animation::new(Vec3::new(0.0, 0.0, -10.0), Vec3::new(0.0, 0.25, 2.0), rainbow_time);

    loop {
        // check how long it's been
        current_time = SystemTime::now();
        let time_since_start = current_time.duration_since(time_of_start).expect("failed to get time since start");
        let time_since_start = time_since_start.as_millis() as f32;
        // has it been long enough?
        if time_since_start > rainbow_time {
            break;
        }

        // set colour of mesh
        #[cfg(feature = "glfw")]
        unsafe {
            let colour = gen_rainbow(time_since_start as f64);
            // get uniform location
            let colour_loc = glGetUniformLocation(renderer.backend.shaders.as_mut().unwrap()[rainbow_shader].program, CString::new("i_colour").unwrap().as_ptr());
            glUniform4f(colour_loc, colour.r as f32 / 255.0, colour.g as f32 / 255.0, colour.b as f32 / 255.0, 1.0);
        }


        // get the point at the current time
        let point = rainbow_anim.get_point_at_time(time_since_start as f64);
        // set the position of the mesh
        mesh.position = point;
        // draw the mesh
        renderer.render_mesh(mesh, rainbow_shader, true, false);
        // swap buffers
        renderer.swap_buffers();

        // poll events
        if renderer.manage_window() {
            process::exit(0);
        }
    }

    let normal_time = 10000.0 - rainbow_time; // in milliseconds
    let normal_anim = Animation::new(Vec3::new(0.0, 0.25, 2.0), Vec3::new(0.0, 0.35, 1.7), normal_time);

    let opacity_delay = 1000.0; // in milliseconds
    let mut opacity_timer = 0.0;

    let mut dutch = 0.0; // dutch angle or whatever this probably isn't the correct usage of that word

    let mut last_time = SystemTime::now();
    loop {
        // check how long it's been
        current_time = SystemTime::now();
        let time_since_start = current_time.duration_since(time_of_start).expect("failed to get time since start");
        let time_since_start = time_since_start.as_millis() as f32;
        // has it been long enough?
        if time_since_start > normal_time {
            break;
        }
        let time_since_start =  time_since_start + rainbow_time;
        // get the point at the current time
        let point = normal_anim.get_point_at_time(time_since_start as f64);
        // set the position of the mesh
        mesh.position = point;
        // set the rotation of the mesh
        mesh.rotation = Quaternion::from_euler_angles_zyx(&Vec3::new(0.0, 0.0, dutch));
        dutch += 0.01;
        // draw the mesh
        renderer.render_mesh(mesh, basic_shader, false, true);
        // draw the powered by text
        ui_poweredby.render_at(ui_master, renderer, basic_shader);

        if opacity_timer < opacity_delay {
            opacity_timer += current_time.duration_since(last_time).expect("failed to get time since last frame").as_millis() as f32;
        } else {
            if ui_poweredby.opacity < 1.0 {
                ui_poweredby.opacity += current_time.duration_since(last_time).unwrap().as_secs_f32() / 10.0;
            }
        }

        // swap buffers
        renderer.swap_buffers();

        // poll events
        if renderer.manage_window() {
            process::exit(0);
        }
        last_time = current_time;
    }

    loop {
        ui_developedby.render_at(ui_master, renderer, basic_shader);
        // swap buffers
        renderer.swap_buffers();

        // poll events
        if renderer.manage_window() {
            process::exit(0);
        }
    }
}