use std::borrow::BorrowMut;
use dae_parser::Document;
use gfx_maths::{Quaternion, Vec3};
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

fn main() {
    println!("good day! initialising huskyTech2");
    let renderer = ht_renderer::init();
    if renderer.is_err() {
        println!("failed to initialise renderer");
        println!("{:?}", renderer.err());
        return;
    }
    let mut renderer = renderer.unwrap();
    println!("initialised renderer");

    // load example shader
    let example_index = renderer.load_shader("basic").expect("failed to load example shader");

    // wait 2 seconds

    //sunlust_intro::animate(renderer.clone());
    //std::thread::sleep(std::time::Duration::from_millis(2000));
    test_render(renderer.clone(), example_index);

    loop {
        unsafe {
            glfwPollEvents();
        }
    }
}


// for testing (:
fn test_render(mut renderer: ht_renderer, shader: usize) {
    // load the dae file
    let document = Document::from_file("base/models/ht2.dae").expect("failed to load dae file");
    let mut mesh = renderer.initMesh(document, "Cube_001-mesh", shader).unwrap();
    //let mesh = renderer.gen_testing_triangle();

    println!("{}", mesh.vao);
    //println!("{}", mesh2.vbo);

    renderer.camera.set_position(Vec3::new(0.0, 0.0, 2.0));
    renderer.camera.set_fov(45.0);

    // render the mesh

    let mut r = 0.0;
    loop {
        mesh.rotation = Quaternion::axis_angle(Vec3::new(1.0, 1.0, 0.0), r);
        r += 0.01;

        renderer.render_mesh(mesh, shader);
        renderer.swap_buffers();
    }
}