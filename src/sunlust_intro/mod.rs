// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use std::ffi::CString;
use std::process;
use std::ptr::null;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use fyrox_sound::buffer::{DataSource, SoundBufferResource};
use fyrox_sound::context::SoundContext;
use fyrox_sound::futures::executor::block_on;
use fyrox_sound::source::{SoundSourceBuilder, Status};
use gfx_maths::{Quaternion, Vec2, Vec3};
use kira::manager::AudioManager;
use kira::manager::backend::cpal::CpalBackend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use glad_gl::gl::*;
use crate::animation::Animation;
use crate::helpers::{gen_rainbow, set_shader_if_not_already};
use crate::light::Light;
use crate::renderer::{RGBA, ht_renderer};
use crate::textures::Texture;
use crate::uimesh::UiMesh;

pub fn animate(renderer: &mut ht_renderer, sss: &SoundContext) {
    renderer.backend.clear_colour.store(RGBA { r: 0, g: 0, b: 0, a: 255 }, Ordering::SeqCst);
    // load ht2-mesh logo model
    renderer.load_texture_if_not_already_loaded("ht2").expect("failed to load ht2-mesh texture");
    renderer.load_mesh_if_not_already_loaded("ht2").expect("failed to load ht2 mesh");

    let mut mesh = renderer.meshes.get("ht2").expect("failed to get ht2 mesh").clone();
    let mut texture = renderer.textures.get("ht2").expect("failed to get ht2-mesh texture").clone();

    let ui_master = renderer.backend.ui_master.lock().unwrap().clone().unwrap().clone();
    let basic_shader = renderer.shaders.get("gbuffer").unwrap().clone();
    let rainbow_shader = renderer.shaders.get("rainbow").unwrap().clone();
    let unlit_shader = renderer.shaders.get("unlit").unwrap().clone();
    // load poweredby uimesh
    let mut ui_poweredby = UiMesh::new_element_from_name("poweredby", &ui_master, renderer, basic_shader).expect("failed to load poweredby uimesh");
    // load developedby uimesh
    let mut ui_developedby = UiMesh::new_element_from_name("developedby", &ui_master, renderer, basic_shader).expect("failed to load developedby uimesh");

    let mut light_a = Light {
        position: Vec3::new(0.5, 0.0, 1.6),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 1000.0
    };
    let mut light_b = Light {
        position: Vec3::new(-0.5, 0.0, 1.6),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 1000.0
    };

    let window_size = renderer.window_size.clone();

    let poweredby_width = window_size.y / 2.0;
    let poweredby_height = poweredby_width / 2.0;
    let poweredby_x = 15.0;
    let poweredby_y = window_size.y - poweredby_height - 15.0;
    ui_poweredby.position = Vec2::new(poweredby_x, poweredby_y);
    ui_poweredby.scale = Vec2::new(poweredby_width, poweredby_height);
    ui_poweredby.opacity = 0.0;

    ui_developedby.scale = window_size;

    let mut sunlust_sfx = SoundBufferResource::new_generic(block_on(DataSource::from_file("base/snd/sunlust.wav")).unwrap()).unwrap();


    let source = SoundSourceBuilder::new()
        .with_buffer(sunlust_sfx)
        .with_looping(false)
        .with_status(Status::Playing)
        .build().unwrap();

    let source_handle = sss.state().add_source(source);
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
            set_shader_if_not_already(renderer, rainbow_shader.clone());
            let colour = gen_rainbow(time_since_start as f64);
            // get uniform location

            let colour_c = CString::new("i_colour").unwrap();
            let colour_loc = GetUniformLocation(renderer.backend.shaders.as_mut().unwrap()[rainbow_shader.clone()].program, colour_c.as_ptr());
            Uniform4f(colour_loc, colour.r as f32 / 255.0, colour.g as f32 / 255.0, colour.b as f32 / 255.0, 1.0);
        }


        // get the point at the current time
        let point = rainbow_anim.get_point_at_time(time_since_start as f64);
        // set the position of the mesh
        mesh.position = point;
        // draw the mesh
        mesh.render_basic_lines(renderer, rainbow_shader.clone());
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
    let mut intensity_timer = 0.0;
    let mut intensity_downtimer = 0.0;

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

        // send the lights to the renderer
        renderer.set_lights(vec![light_a, light_b]);

        // draw the mesh
        mesh.render(renderer, Some(&texture));
        // draw the powered by text
        ui_poweredby.render_at(ui_master, renderer);

        if opacity_timer < opacity_delay {
            opacity_timer += current_time.duration_since(last_time).expect("failed to get time since last frame").as_millis() as f32;
        } else if ui_poweredby.opacity < 1.0 {
            ui_poweredby.opacity += current_time.duration_since(last_time).unwrap().as_secs_f32() / 10.0;
        }

        // increase light intensity
        if intensity_downtimer < 100.0 {
            intensity_downtimer += current_time.duration_since(last_time).expect("failed to get time since last frame").as_millis() as f32;
            light_a.intensity = (-intensity_downtimer / 100.0) * 888.0;
            light_b.intensity = (-intensity_downtimer / 100.0) * 888.0;
        } else if intensity_timer < 1000.0 {
            intensity_timer += current_time.duration_since(last_time).expect("failed to get time since last frame").as_millis() as f32;
            light_a.intensity = (intensity_timer / 1000.0) * 0.3;
            light_b.intensity = (intensity_timer / 1000.0) * 0.3;
            light_a.color.y = (-intensity_timer / 1000.0) * 0.01;
            light_b.color.x = (-intensity_timer / 1000.0) * 0.01;
        }

        light_a.position = mesh.position + Vec3::new(-0.5, 0.0, -1.2);
        light_b.position = mesh.position + Vec3::new(0.5, 0.0, -1.2);

        // swap buffers
        renderer.swap_buffers();

        // poll events
        if renderer.manage_window() {
            process::exit(0);
        }
        last_time = current_time;
    }
    let copyright_time = 2000.0 + normal_time; // in milliseconds

    loop {
        // check how long it's been
        current_time = SystemTime::now();
        let time_since_start = current_time.duration_since(time_of_start).expect("failed to get time since start");
        let time_since_start = time_since_start.as_millis() as f32;
        if time_since_start > copyright_time {
            break;
        }

        ui_developedby.render_at(ui_master, renderer);
        // swap buffers
        renderer.swap_buffers();

        // poll events
        if renderer.manage_window() {
            process::exit(0);
        }
    }

    sss.state().remove_source(source_handle);
    renderer.backend.clear_colour.store(RGBA { r: 0, g: 75, b: 75, a: 255 }, Ordering::SeqCst);
}