// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use std::ffi::CString;
use std::ptr::null;
use std::time::SystemTime;
use dae_parser::Document;
use gfx_maths::Vec2;
use kira::manager::AudioManager;
use kira::manager::backend::cpal::CpalBackend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use libsex::bindings::*;
use crate::animation::Animation2D;
use crate::helpers::gen_rainbow;
use crate::renderer::{Colour, ht_renderer};

pub fn animate(renderer: &mut ht_renderer, sss: &mut AudioManager<CpalBackend>) {
    // load rainbow shader
    let rainbow_shader = renderer.load_shader("rainbow").expect("failed to load rainbow shader");
    // load ht2 logo model
    let document = Document::from_file("base/models/ht2.dae").expect("failed to load dae file");
    let mesh = renderer.initMesh(document, "Cube_001-mesh", rainbow_shader).unwrap();

    let mut sunlust_sfx = StaticSoundData::from_file("base/snd/sunlust.wav", StaticSoundSettings::default()).expect("failed to load sunlust.wav");
    sss.play(sunlust_sfx.clone());
    println!("playing sunlust.wav");
    let time_of_start = SystemTime::now(); // when the animation started

    loop {

    }
}