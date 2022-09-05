#[macro_use]
extern crate log;

use std::borrow::BorrowMut;
use dae_parser::Document;
use gfx_maths::{Quaternion, Vec3};
use kira::manager::{AudioManager, AudioManagerSettings};
use kira::manager::backend::cpal::CpalBackend;
use libsex::bindings::*;
use crate::renderer::ht_renderer;

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
pub mod cock_handler;
pub mod animation;
pub mod shaders;
pub mod camera;
pub mod meshes;
pub mod textures;

fn main() {
    env_logger::init();

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
    info!("initialised renderer");

    sunlust_intro::animate(&mut renderer, &mut sss);

    loop {
        unsafe {
            glfwPollEvents();
        }
    }
}