use std::io::Read;
use std::os::raw::c_int;
use std::ptr::null_mut;
use gfx_maths::{Mat4, Quaternion, Vec3};
use glad_gl::gl::*;
use crate::renderer::Colour;
use crate::ht_renderer;

pub fn set_shader_if_not_already(renderer: &mut ht_renderer, shader_index: usize) {
    if renderer.backend.current_shader != Some(shader_index) {
        unsafe {
            UseProgram(renderer.backend.shaders.as_mut().unwrap()[shader_index].program);
            renderer.backend.current_shader = Some(shader_index);
        }
    }
}

pub fn gen_rainbow(time: f64) -> Colour {
    let frequency = 0.05;
    let r = ((frequency * (time as f64) + 0.0).sin() * 127.0f64 + 128.0f64);
    let g = ((frequency * (time as f64) + 2.0).sin() * 127.0f64 + 128.0f64);
    let b = ((frequency * (time as f64) + 4.0).sin() * 127.0f64 + 128.0f64);
    Colour { r: (r) as u8, g: (g) as u8, b: (b) as u8, a: 255 }
}

pub fn load_string_from_file(path: String) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|e| e.to_string())?;
    Ok(contents)
}

pub fn calculate_model_matrix(position: Vec3, rotation: Quaternion, scale: Vec3) -> Mat4 {
    let mut model_matrix = Mat4::identity();
    model_matrix = model_matrix * Mat4::translate(position);
    model_matrix = model_matrix * Mat4::rotate(rotation);
    model_matrix = model_matrix * Mat4::scale(scale);
    model_matrix
}

pub fn largest_angle_between(a: Vec3, b: Vec3) -> f64 {
    let dot = a.dot(b) as f64;
    let angle = dot.acos() as f64;
    angle
}

pub fn conjugate_quaternion(quat: Quaternion) -> Quaternion {
    Quaternion::new(-quat.x, -quat.y, -quat.z, quat.w)
}

pub fn rotate_vector_by_quaternion(vector: Vec3, quat: Quaternion) -> Vec3 {
    let mut quat_v = Quaternion::new(vector.x, vector.y, vector.z, 0.0);
    quat_v = quat_v * quat;
    quat_v = conjugate_quaternion(quat) * quat_v;
    Vec3::new(quat_v.x, quat_v.y, quat_v.z)
}

pub fn distance(a: Vec3, b: Vec3) -> f32 {
    let x = a.x - b.x;
    let y = a.y - b.y;
    let z = a.z - b.z;
    (x * x + y * y + z * z).abs().sqrt()
}

// make sure to preserve negative vectors
pub fn clamp_magnitude(vector: Vec3, max_magnitude: f32) -> Vec3 {
    let magnitude = vector.magnitude();
    if magnitude > max_magnitude {
        vector / magnitude * max_magnitude
    } else {
        vector
    }
}