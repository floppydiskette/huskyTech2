use gfx_maths::*;

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
        self.view = Mat4::rotate(self.rotation) * Mat4::translate(self.position);
    }

    // getters and setters
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
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