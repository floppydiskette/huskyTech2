use std::ops::Deref;
use gfx_maths::*;
use crate::helpers;
use crate::helpers::gfx_maths_mat4_to_glmatrix_mat4;

pub const EYE_HEIGHT: f32 = 1.36;

#[derive(Clone)]
pub struct Camera {
    position: Vec3,
    rotation: Quaternion,
    projection: Mat4,
    view: Mat4,
    window_size: Vec2,
    fov: f32,
    near: f32,
    far: f32,
}

fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * std::f32::consts::PI / 180.0
}

impl Camera {
    pub fn new(window_size: Vec2, fov: f32, near: f32, far: f32) -> Camera {
        let mut camera = Camera {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            projection: Mat4::identity(),
            view: Mat4::identity(),
            window_size,
            fov,
            near,
            far,
        };

        camera.recalculate_projection();
        camera.recalculate_view();
        camera
    }

    pub fn get_front(&self) -> Vec3 {
        // forward vector is third column of view matrix
        let view = self.view;
        let view = gfx_maths_mat4_to_glmatrix_mat4(view);
        let view = gl_matrix::mat4::invert(&mut gl_matrix::common::Mat4::default(), &view).unwrap();
        let view = helpers::glmatrix_mat4_to_gfx_maths_mat4(view);
        *helpers::column_mat_to_vec(view, 2).normalize().deref()
    }

    pub fn get_forward_no_pitch(&self) -> Vec3 {
        let mut front = Vec3::new(0.0, 0.0, 1.0);
        front = helpers::rotate_vector_by_quaternion(front, self.rotation);
        front.y = 0.0;
        *front.normalize().deref()
    }

    pub fn get_right(&self) -> Vec3 {
        // right vector is first column of view matrix
        let view = self.view;
        let view = gfx_maths_mat4_to_glmatrix_mat4(view);
        let view = gl_matrix::mat4::invert(&mut gl_matrix::common::Mat4::default(), &view).unwrap();
        let view = helpers::glmatrix_mat4_to_gfx_maths_mat4(view);
        -helpers::column_mat_to_vec(view, 0)
    }

    pub fn get_up(&self) -> Vec3 {
        // cross product of right and forward vectors
        let right = self.get_right();
        let front = self.get_front();
        right.cross(front)
    }

    // sets rotation to be looking at the target, with the up vector being up, and recalculates the view matrix
    pub fn look_at(&mut self, target: Vec3) {
        // subtract the target from the camera position and take atan2 of the difference
        let diff = self.position - target;
        let yaw = f32::atan2(diff.x, diff.z);
        let pitch = f32::atan2(diff.y, diff.z);
        self.rotation = Quaternion::from_euler_angles_zyx(&Vec3::new(pitch, yaw, 0.0));
        self.recalculate_view();
    }

    // calculates the projection matrix from the camera's perspective
    fn recalculate_projection(&mut self) {
        let aspect_ratio = self.window_size.x as f32 / self.window_size.y as f32;
        self.projection = Mat4::perspective_opengl(degrees_to_radians(self.fov), self.near, self.far, aspect_ratio);
    }

    // calculates the view matrix from the camera's position and rotation
    fn recalculate_view(&mut self) {
        self.view = Mat4::rotate(self.rotation) * Mat4::translate(-self.position);
    }

    // getters and setters
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.recalculate_view();
    }

    // DEPRECATED
    pub fn set_position_from_player_position(&mut self, player_position: Vec3) {
        self.position = player_position + Vec3::new(0.0, EYE_HEIGHT, 0.0);
        self.recalculate_view();
    }

    pub fn get_rotation(&self) -> Quaternion {
        self.rotation
    }

    pub fn set_rotation(&mut self, rotation: Quaternion) {
        self.rotation = rotation;
        self.recalculate_view();
    }

    pub fn get_projection(&self) -> Mat4 {
        self.projection
    }

    pub fn get_view(&self) -> Mat4 {
        self.view
    }


    pub fn get_fov(&self) -> f32 {
        self.fov
    }

    pub fn set_fov(&mut self, fov: f32) {
        self.fov = fov;
        self.recalculate_projection();
    }
}