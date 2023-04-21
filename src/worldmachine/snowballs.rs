use gfx_maths::Vec3;
use crate::physics::{Materials, PhysicsSphereColliderDynamic, PhysicsSystem};

pub struct Snowball {
    pub uuid: String,
    pub position: Vec3,
    pub initial_velocity: Vec3,
    pub time_to_live: f32,
    physics_object: PhysicsSphereColliderDynamic,
}

impl Snowball {
    pub fn new(position: Vec3, initial_velocity: Vec3, physics: &PhysicsSystem) -> Self {
        info!("creating snowball at {:?}", position);
        let phys = physics.create_sphere_actor(position, 0.05, Materials::Player).unwrap();
        phys.add_self_to_scene(physics.clone());
        phys.set_velocity(initial_velocity);
        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            position,
            initial_velocity,
            time_to_live: 20.0,
            physics_object: phys,
        }
    }
    pub fn new_with_uuid(uuid: String, position: Vec3, initial_velocity: Vec3, physics: &PhysicsSystem) -> Self {
        info!("creating snowball (clientside) at {:?}", position);
        let mut oneshots = crate::audio::ONESHOTS.lock().unwrap();
        oneshots.push(("donk.wav".to_string(), position));
        let phys = physics.create_sphere_actor(position, 0.05, Materials::Player).unwrap();
        phys.add_self_to_scene(physics.clone());
        phys.set_velocity(initial_velocity);
        Self {
            uuid,
            position,
            initial_velocity,
            time_to_live: 20.0,
            physics_object: phys,
        }
    }

    pub fn get_position(&mut self) -> Vec3 {
        let position = self.physics_object.get_position();
        self.position = position;
        position
    }
}