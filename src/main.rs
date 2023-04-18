#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
extern crate core;

use std::borrow::BorrowMut;
use std::{process, thread};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use egui_glfw_gl::egui;
use fyrox_sound::context::SoundContext;
use fyrox_sound::engine::SoundEngine;
use gfx_maths::{Quaternion, Vec3};
use kira::manager::{AudioManager, AudioManagerSettings};
use kira::manager::backend::cpal::CpalBackend;
use glad_gl::gl::*;
use glfw::WindowEvent;
use tokio::sync::Mutex;
use crate::keyboard::{HTKey, Keyboard};
use crate::renderer::{ht_renderer, RGBA};
use crate::server::ConnectionClientside;
use crate::server::lan::ClientLanConnection;
use crate::worldmachine::player::DEFAULT_FOV;

pub trait Thingy {
    fn get_x(&self) -> i32;
    fn get_y(&self) -> i32;
    fn get_z(&self) -> i32;
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
    fn get_depth(&self) -> i32;
}

pub mod sunlust_intro;
pub mod renderer;
pub mod helpers;
pub mod animation;
pub mod shaders;
pub mod camera;
pub mod meshes;
pub mod textures;
pub mod map;
pub mod light;
pub mod worldmachine;
pub mod physics;
pub mod server;
pub mod keyboard;
pub mod mouse;
pub mod optimisations;
pub mod skeletal_animation;
pub mod animgraph;
pub mod ui;
pub mod audio;
pub mod common_anim;
pub mod maps;

#[tokio::main]
#[allow(unused_must_use)]
async fn main() {
    env_logger::init();

    // get args
    let mut args = std::env::args();
    let mut skip_intro = false;
    let mut level_to_load = Option::None;
    let mut run_as_lan_server = false;
    let mut connect_to_lan_server = Option::None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--skip-intro" => skip_intro = true,
            "--level" => {
                level_to_load = Option::Some(args.next().expect("expected level name after --level"));
            }
            "--lan-server" => {
                run_as_lan_server = true;
            }
            "--connect-to-lan-server" => {
                connect_to_lan_server = Option::Some(args.next().expect("expected ip after --connect-to-lan-server"));
            }
            _ => {}
        }
    }

    let start_time = Instant::now();

    if run_as_lan_server {
        info!("good day! running as lan server");

        let mut physics = physics::PhysicsSystem::init();
        info!("initialised physics");

        let mut server = server::Server::new_host_lan_server("test", physics, 25566, 25567, "0.0.0.0").await;
        let mut server_clone = server.clone();
        info!("initialised server");
        server_clone.run().await;
    } else {
        info!("good day! initialising huskyTech2");
        let sengine = SoundEngine::new();
        let scontext = SoundContext::new();

        sengine.lock().unwrap().add_context(scontext.clone());

        let mut audio = crate::audio::AudioBackend::new();
        info!("initialised audio subsystem");
        let renderer = ht_renderer::init();
        if renderer.is_err() {
            error!("failed to initialise renderer");
            error!("{:?}", renderer.err());
            return;
        }
        let mut renderer = renderer.unwrap();
        renderer.initialise_basic_resources();
        info!("initialised renderer");

        let mut physics = physics::PhysicsSystem::init();
        info!("initialised physics");

        let mut worldmachine = worldmachine::WorldMachine::default();
        worldmachine.initialise(physics.clone(), false);

        info!("initialised worldmachine");

        if let Some(ip) = connect_to_lan_server {
            let server_connection = ClientLanConnection::connect(ip.as_str(), 25566, 25567).await.expect("failed to connect to server");
            worldmachine.connect_to_server(ConnectionClientside::Lan(server_connection.clone()));
            let the_clone = server_connection.clone();
            tokio::spawn(async move {
                the_clone.udp_listener_thread().await;
            });
            let the_clone = server_connection.clone();
            tokio::spawn(async move {
                the_clone.tcp_listener_thread().await;
            });
        } else {
            let mut server = server::Server::new("test", physics.clone());
            let mut server_clone = server.clone();
            tokio::spawn(async move {
                server_clone.run().await;
            });
            let server_connection = server.join_local_server().await;
            worldmachine.connect_to_server(ConnectionClientside::Local(server_connection.clone()));
        }

        debug!("connected to server");

        renderer.load_mesh_if_not_already_loaded("player");
        if !skip_intro { sunlust_intro::animate(&mut renderer, &scontext) }
        renderer.backend.clear_colour.store(RGBA { r: 0, g: 75, b: 75, a: 255 }, Ordering::SeqCst);
        crate::ui::SHOW_UI.store(true, Ordering::SeqCst);

        renderer.camera.set_fov(DEFAULT_FOV);

        loop {
            if let Ok(res) = renderer.load_mesh_if_not_already_loaded("player") {
                if res {
                    break;
                }
            } else {
                panic!("failed to load player mesh");
            }
        }

        ui::debug_log("haiii! :3 :3 :3");

        let mut last_frame_time = std::time::Instant::now();
        loop {
            let delta = (last_frame_time.elapsed().as_millis() as f64 / 1000.0) as f32;
            last_frame_time = Instant::now();

            // calculate fps based on delta
            let fps = 1.0 / delta;
            *crate::ui::FPS.lock().unwrap() = fps;

            renderer.backend.input_state.lock().unwrap().input.time = Some(start_time.elapsed().as_secs_f64());
            renderer.backend.egui_context.lock().unwrap().begin_frame(renderer.backend.input_state.lock().unwrap().input.take());
            let mut updates = worldmachine.client_tick(&mut renderer, physics.clone(), delta).await; // physics ticks are also simulated here clientside
            worldmachine.tick_connection(&mut updates).await;
            worldmachine.handle_audio(&renderer, &audio, &scontext);
            worldmachine.render(&mut renderer, None);
            renderer.clear_all_shadow_buffers();
            let light_count = renderer.lights.len();
            for i in 0..light_count {
                worldmachine.render(&mut renderer, Some((1, i)));
                worldmachine.render(&mut renderer, Some((2, i)));
                renderer.next_light();
            }
            //worldmachine.render(&mut renderer, Some(2));

            renderer.swap_buffers();
            renderer.backend.window.lock().unwrap().glfw.poll_events();
            keyboard::reset_keyboard_state();
            mouse::reset_mouse_state();
            for (_, event) in glfw::flush_messages(renderer.backend.events.lock().unwrap().deref()) {
                egui_glfw_gl::handle_event(event.clone(), &mut renderer.backend.input_state.lock().unwrap());
                keyboard::tick_keyboard(event.clone());
                mouse::tick_mouse(event);
            }
            if renderer.manage_window() || keyboard::check_key_released(HTKey::Escape) {
                process::exit(0);
            }
        }
    }
}