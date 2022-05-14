use std::borrow::BorrowMut;
use dae_parser::Document;
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

    // wait 2 seconds
    std::thread::sleep(std::time::Duration::from_millis(2000));

    //sunlust_intro::animate(renderer);
    test_render(renderer);

    loop {}
}


// for testing (:
fn test_render(mut renderer: ht_renderer) {
    // load the dae file
    let document = Document::from_file("base/models/ht2.dae").unwrap();
    let mesh = renderer.initMesh(document, "Cube_001-mesh").unwrap();

    // render the mesh
    renderer.render_mesh(mesh);
    renderer.swap_buffers();
    println!("rendered mesh");
}