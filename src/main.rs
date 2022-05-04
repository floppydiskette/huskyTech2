pub trait Thingy {
    fn get_x(&self) -> i32;
    fn get_y(&self) -> i32;
    fn get_z(&self) -> i32;
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
    fn get_depth(&self) -> i32;
}

pub mod renderer;
pub mod helpers;
pub mod cock_handler;
pub mod animation;

fn main() {
    println!("good day! initialising huskyTech2");

}
