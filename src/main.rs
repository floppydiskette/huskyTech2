#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
extern crate core;

use std::borrow::BorrowMut;
use std::{process, thread};
use gfx_maths::{Quaternion, Vec3};
use kira::manager::{AudioManager, AudioManagerSettings};
use kira::manager::backend::cpal::CpalBackend;
use libsex::bindings::*;
use crate::keyboard::{Key, Keyboard};
use crate::renderer::ht_renderer;
use crate::server::ConnectionClientside;

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
pub mod uimesh;
pub mod map;
pub mod light;
pub mod worldmachine;
pub mod physics;
pub mod server;
pub mod keyboard;
pub mod mouse;

#[tokio::main]
#[allow(unused_must_use)]
async fn main() {
    env_logger::init();

    // get args
    let mut args = std::env::args();
    let mut skip_intro = false;
    let mut level_to_load = Option::None;

    while let Some(arg) = args.next() {
        if arg == "--skip-intro" {
            skip_intro = true;
        } else if arg == "--level" {
            level_to_load = Option::Some(args.next().expect("expected level name after --level"));
        }
    }

    info!("good day! initialising huskyTech2");
    let mut sss = AudioManager::<CpalBackend>::new(AudioManagerSettings::default()).expect("failed to initialise audio subsystem");
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

    let mut server = server::Server::new("test", physics.clone());
    let mut server_clone = server.clone();
    tokio::spawn(async move {
        server_clone.run().await;
    });

    let mut server_connection = server.join_local_server().await;
    worldmachine.connect_to_server(ConnectionClientside::Local(server_connection));

    debug!("connected to internal server");

    keyboard::init(&mut renderer);
    mouse::init(&mut renderer);

    debug!("initialised keyboard");

    if !skip_intro { sunlust_intro::animate(&mut renderer, &mut sss) }

    renderer.lock_mouse(true);

    let mut last_frame_time = std::time::Instant::now();
    loop {
        let delta = last_frame_time.elapsed().as_secs_f32();
        keyboard::tick_keyboard();
        mouse::tick_mouse();
        let mut updates = worldmachine.client_tick(&mut renderer, physics.clone(), delta); // physics ticks are also simulated here clientside
        worldmachine.tick_connection(&mut updates).await;
        worldmachine.render(&mut renderer);
        renderer.swap_buffers();
        if renderer.manage_window() || keyboard::check_key_released(Key::Escape) {
            process::exit(0);
        }
        last_frame_time = std::time::Instant::now();
    }
}