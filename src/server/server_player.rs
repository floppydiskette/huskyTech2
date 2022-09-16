use gfx_maths::*;
use crate::helpers;
use crate::physics::{ClimbingMode, Materials, PhysicsCharacterController, PhysicsSystem};
use crate::server::Server;
use crate::worldmachine::{EntityId, WorldMachine};

pub const DEFAULT_MOVESPEED: f32 = 10.0;
pub const DEFAULT_RADIUS: f32 = 0.5;
pub const DEFAULT_HEIGHT: f32 = 1.7;
pub const DEFAULT_STEPHEIGHT: f32 = 0.5;

pub const ERROR_MARGIN: f32 = 0.8;

#[derive(Clone)]
pub struct ServerPlayerContainer {
    pub player: ServerPlayer,
    pub entity_id: Option<EntityId>,
}

#[derive(Clone)]
pub struct ServerPlayer {
    pub uuid: String,
    pub name: String,
    position: Vec3,
    head_rotation: Quaternion,
    rotation: Quaternion,
    pub scale: Vec3,
    physics_controller: Option<PhysicsCharacterController>,
    movement_speed: f32,
}

impl Default for ServerPlayer {
    fn default() -> Self {
        Self {
            uuid: "".to_string(),
            name: "".to_string(),
            position: Vec3::new(0.0, 0.0, 0.0),
            head_rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
        }
    }
}

impl ServerPlayer {
    pub fn new(name: &str, position: Vec3, rotation: Quaternion, scale: Vec3) -> Self {
        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            position,
            head_rotation: rotation,
            rotation,
            scale,
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
        }
    }

    pub fn init(&mut self, physics_system: PhysicsSystem) {
        self.physics_controller = physics_system.create_character_controller(DEFAULT_RADIUS, DEFAULT_HEIGHT, DEFAULT_STEPHEIGHT, Materials::Player);
        if self.physics_controller.is_none() {
            warn!("failed to create physics controller for player");
        }
    }

    /// attempts to move the player to the given position, returning true if the move was successful, or false if the move was too fast.
    pub async fn attempt_position_change(&mut self, new_position: Vec3, displacement_vector: Vec3, new_rotation: Quaternion, new_head_rotation: Quaternion, worldmachine: &mut WorldMachine) -> bool {
        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(worldmachine.last_physics_update).as_secs_f32();
        debug!("delta_time: {}", delta);
        self.physics_controller.as_mut().unwrap().move_by(displacement_vector, delta);
        worldmachine.physics.as_mut().unwrap().tick(delta);
        worldmachine.last_physics_update = current_time;
        let new_position_calculated = self.physics_controller.as_mut().unwrap().get_position();
        let distance = helpers::distance(new_position_calculated, new_position);
        debug!("distance: {}", distance);
        if distance < ERROR_MARGIN {
            self.position = new_position;
            self.rotation = new_rotation;
            self.head_rotation = new_head_rotation;
            self.physics_controller.as_mut().unwrap().set_position(new_position);
            true
        } else {
            false
        }
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        if let Some(physics_controller) = &self.physics_controller {
            physics_controller.set_position(position);
        }
    }

    pub fn set_rotation(&mut self, rotation: Quaternion) {
        self.rotation = rotation;
        //if let Some(physics_controller) = &self.physics_controller {
        //    physics_controller.set_rotation(rotation);
        //}
    }

    pub fn set_head_rotation(&mut self, rotation: Quaternion) {
        self.head_rotation = rotation;
    }

    pub fn set_scale(&mut self, scale: Vec3) {
        self.scale = scale;
    }

    pub fn get_position(&self) -> Vec3 {
        let position = if let Some(physics_controller) = &self.physics_controller {
            physics_controller.get_position()
        } else {
            self.position
        };
        position
    }

    pub fn get_rotation(&self) -> Quaternion {
        self.rotation
    }

    pub fn get_head_rotation(&self) -> Quaternion {
        self.head_rotation
    }

    pub fn get_scale(&self) -> Vec3 {
        self.scale
    }
}