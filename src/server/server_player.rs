use gfx_maths::*;
use crate::helpers;
use crate::physics::{ClimbingMode, Materials, PhysicsCharacterController, PhysicsSystem};
use crate::server::Server;
use crate::worldmachine::{EntityId, WorldMachine, WorldUpdate};
use crate::worldmachine::components::COMPONENT_TYPE_PLAYER;
use crate::worldmachine::ecs::ParameterValue;
use crate::worldmachine::player::MovementInfo;

pub const DEFAULT_MOVESPEED: f32 = 0.2;
pub const DEFAULT_SPRINTSPEED: f32 = 0.6;
pub const DEFAULT_RADIUS: f32 = 0.5;
pub const DEFAULT_HEIGHT: f32 = 1.7;
pub const DEFAULT_STEPHEIGHT: f32 = 0.5;

pub const ERROR_MARGIN: f32 = 1.0;

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
    last_move_call: std::time::Instant,
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
            last_move_call: std::time::Instant::now(),
        }
    }
}

impl ServerPlayer {
    pub fn new(uuid: &str, name: &str, position: Vec3, rotation: Quaternion, scale: Vec3) -> Self {
        Self {
            uuid: uuid.to_string(),
            name: name.to_string(),
            position,
            head_rotation: rotation,
            rotation,
            scale,
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
            last_move_call: std::time::Instant::now(),
        }
    }

    pub fn init(&mut self, physics_system: PhysicsSystem) {
        self.physics_controller = physics_system.create_character_controller(DEFAULT_RADIUS, DEFAULT_HEIGHT, DEFAULT_STEPHEIGHT, Materials::Player);
        if self.physics_controller.is_none() {
            warn!("failed to create physics controller for player");
        }
    }

    /// attempts to move the player to the given position, returning true if the move was successful, or false if the move was too fast.
    pub async fn attempt_position_change(&mut self, new_position: Vec3, displacement_vector: Vec3, new_rotation: Quaternion, new_head_rotation: Quaternion, movement_info: MovementInfo, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) -> bool {
        // TODO!! IMPORTANT!! remember to check that the player is not trying to move vertically, or through a wall! displacement_vector should not contain a y value, and the new_position should be checked against the world to make sure it is not inside a wall.

        if movement_info.sprinting {
            self.movement_speed = DEFAULT_SPRINTSPEED;
        } else {
            self.movement_speed = DEFAULT_MOVESPEED;
        }

        let mut displacement_vector = displacement_vector;
        displacement_vector.y = 0.0;
        displacement_vector = helpers::clamp_magnitude(displacement_vector, 1.0 * self.movement_speed);

        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(self.last_move_call).as_secs_f32();
        self.physics_controller.as_mut().unwrap().move_by(displacement_vector, movement_info.jumped, false, delta);
        self.last_move_call = current_time;
        let current_time = std::time::Instant::now();
        let delta = current_time.duration_since(worldmachine.last_physics_update).as_secs_f32();
        worldmachine.physics.as_mut().unwrap().tick(delta);
        worldmachine.last_physics_update = current_time;
        let new_position_calculated = self.physics_controller.as_mut().unwrap().get_position();
        let distance = helpers::distance(new_position_calculated, new_position);
        if distance < ERROR_MARGIN {
            self.set_position(new_position, entity_id, worldmachine).await;
            self.set_rotation(new_rotation, entity_id, worldmachine).await;
            self.set_head_rotation(new_head_rotation, entity_id, worldmachine).await;
            true
        } else {
            self.set_position(new_position_calculated, entity_id, worldmachine).await;
            self.set_rotation(new_rotation, entity_id, worldmachine).await;
            self.set_head_rotation(new_head_rotation, entity_id, worldmachine).await;
            false
        }
    }

    pub async fn gravity_tick(&mut self, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        let delta = std::time::Instant::now().duration_since(self.last_move_call).as_secs_f32();
        let previous_position = self.physics_controller.as_mut().unwrap().get_position();
        self.physics_controller.as_mut().unwrap().move_by(Vec3::zero(), false, false, delta);
        let new_position = self.physics_controller.as_mut().unwrap().get_position();
        self.last_move_call = std::time::Instant::now();
        if previous_position != new_position {
            self.set_position(new_position, entity_id, worldmachine).await;
        }
    }

    pub async fn set_position(&mut self, position: Vec3, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        self.position = position;
        if let Some(physics_controller) = &self.physics_controller {
            physics_controller.set_position(position);
        }
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set position of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "position", ParameterValue::Vec3(position));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, position, self.rotation, self.head_rotation)).await;
            }
        }
    }

    pub async fn set_rotation(&mut self, rotation: Quaternion, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        self.rotation = rotation;
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set rotation of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "rotation", ParameterValue::Quaternion(rotation));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, self.position, rotation, self.head_rotation)).await;
            }
        }
    }

    pub async fn set_head_rotation(&mut self, rotation: Quaternion, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        self.head_rotation = rotation;
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set head rotation of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "head_rotation", ParameterValue::Quaternion(rotation));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, self.position, self.rotation, rotation)).await;
            }
        }
    }

    pub async fn set_scale(&mut self, scale: Vec3, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        self.scale = scale;
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set scale of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "scale", ParameterValue::Vec3(scale));
                worldmachine.queue_update(WorldUpdate::SetScale(entity_id, scale)).await;
            }
        }
    }

    pub async fn get_position(&mut self, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) -> Vec3 {
        let position = if let Some(physics_controller) = &self.physics_controller {
            physics_controller.get_position()
        } else {
            self.position
        };
        self.position = position;
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to get position of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "position", ParameterValue::Vec3(position));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, position, self.rotation, self.head_rotation)).await;
            }
        }
        position
    }

    pub async fn get_rotation(&self, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) -> Quaternion {
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to get rotation of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "rotation", ParameterValue::Quaternion(self.rotation));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, self.position, self.rotation, self.head_rotation)).await;
            }
        }
        self.rotation
    }

    pub async fn get_head_rotation(&self, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) -> Quaternion {
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to get head rotation of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "head_rotation", ParameterValue::Quaternion(self.head_rotation));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, self.position, self.rotation, self.head_rotation)).await;
            }
        }
        self.head_rotation
    }

    pub async fn get_scale(&self, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) -> Vec3 {
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to get scale of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "scale", ParameterValue::Vec3(self.scale));
                worldmachine.queue_update(WorldUpdate::SetScale(entity_id, self.scale)).await;
            }
        }
        self.scale
    }
}