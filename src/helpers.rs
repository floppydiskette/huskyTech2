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

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn glmatrix_mat4_to_gfx_maths_mat4(a: gl_matrix::common::Mat4) -> gfx_maths::Mat4 {
    let mut b = gfx_maths::Mat4::identity();
    b.values[0] = a[0];
    b.values[1] = a[1];
    b.values[2] = a[2];
    b.values[3] = a[3];
    b.values[4] = a[4];
    b.values[5] = a[5];
    b.values[6] = a[6];
    b.values[7] = a[7];
    b.values[8] = a[8];
    b.values[9] = a[9];
    b.values[10] = a[10];
    b.values[11] = a[11];
    b.values[12] = a[12];
    b.values[13] = a[13];
    b.values[14] = a[14];
    b.values[15] = a[15];
    b
}

pub fn gfx_maths_mat4_to_glmatrix_mat4(a: gfx_maths::Mat4) -> gl_matrix::common::Mat4 {
    let mut b = gl_matrix::common::Mat4::default();
    b[0] = a.values[0];
    b[1] = a.values[1];
    b[2] = a.values[2];
    b[3] = a.values[3];
    b[4] = a.values[4];
    b[5] = a.values[5];
    b[6] = a.values[6];
    b[7] = a.values[7];
    b[8] = a.values[8];
    b[9] = a.values[9];
    b[10] = a.values[10];
    b[11] = a.values[11];
    b[12] = a.values[12];
    b[13] = a.values[13];
    b[14] = a.values[14];
    b[15] = a.values[15];
    b
}

pub fn gltf_matrix_to_gfx_maths_mat4(a: [[f32; 4]; 4]) -> gfx_maths::Mat4 {
    let mut b = gfx_maths::Mat4::identity();
    b.values[0] = a[0][0];
    b.values[1] = a[0][1];
    b.values[2] = a[0][2];
    b.values[3] = a[0][3];
    b.values[4] = a[1][0];
    b.values[5] = a[1][1];
    b.values[6] = a[1][2];
    b.values[7] = a[1][3];
    b.values[8] = a[2][0];
    b.values[9] = a[2][1];
    b.values[10] = a[2][2];
    b.values[11] = a[2][3];
    b.values[12] = a[3][0];
    b.values[13] = a[3][1];
    b.values[14] = a[3][2];
    b.values[15] = a[3][3];
    b
}

pub fn interpolate_mats(a: Mat4, b: Mat4, t: f64) -> Mat4 {
    let mut a = a;
    let mut b = b;
    let mut t = t;
    if t < 0.0 {
        t = 0.0;
    }
    if t > 1.0 {
        t = 1.0;
    }
    let mut result = Mat4::identity();
    for i in 0..4 {
        for j in 0..4 {
            result.values[i * 4 + j] = a.values[i * 4 + j] + (b.values[i * 4 + j] - a.values[i * 4 + j]) * t as f32;
        }
    }
    result
}