// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use std::ffi::CString;
use std::ptr::null;
use std::time::SystemTime;
use dae_parser::Document;
use gfx_maths::{Vec2, Vec3};
use kira::manager::AudioManager;
use kira::manager::backend::cpal::CpalBackend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use libsex::bindings::*;
use crate::animation::Animation;
use crate::helpers::gen_rainbow;
use crate::renderer::{Colour, ht_renderer};

pub fn animate(renderer: &mut ht_renderer, sss: &mut AudioManager<CpalBackend>) {
    // load rainbow shader
    let rainbow_shader = renderer.load_shader("rainbow").expect("failed to load rainbow shader");
    // load ht2 logo model
    let document = Document::from_file("base/models/ht2.dae").expect("failed to load dae file");
    let mut mesh = renderer.initMesh(document, "Cube_001-mesh", rainbow_shader).unwrap();

    let mut sunlust_sfx = StaticSoundData::from_file("base/snd/sunlust.wav", StaticSoundSettings::default()).expect("failed to load sunlust.wav");
    sss.play(sunlust_sfx.clone());
    println!("playing sunlust.wav");
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
        renderer.render_mesh(mesh, rainbow_shader, true);
        // swap buffers
        renderer.swap_buffers();
    }

    let normal_time = 9119.0 - rainbow_time; // in milliseconds
    let normal_anim = Animation::new(Vec3::new(0.0, 0.25, 2.0), Vec3::new(0.0, 0.35, 1.7), normal_time);

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
        // draw the mesh
        renderer.render_mesh(mesh, rainbow_shader, false);
        // swap buffers
        renderer.swap_buffers();
    }
}