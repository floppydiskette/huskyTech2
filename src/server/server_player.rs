use std::sync::{Arc};
use mutex_timeouts::tokio::MutexWithTimeoutAuto as Mutex;
use std::sync::atomic::AtomicBool;
use gfx_maths::*;
use tokio::time::Instant;
use crate::helpers;
use crate::physics::{ClimbingMode, Materials, PhysicsCharacterController, PhysicsSystem};
use crate::server::{Connection, Server};
use crate::worldmachine::{EntityId, WorldMachine, WorldUpdate};
use crate::worldmachine::components::COMPONENT_TYPE_PLAYER;
use crate::worldmachine::ecs::ParameterValue;
use crate::worldmachine::player::MovementInfo;

pub const DEFAULT_MOVESPEED: f32 = 8.15;
pub const DEFAULT_SPRINTSPEED: f32 = 14.4;
pub const DEFAULT_RADIUS: f32 = 1.3;
pub const DEFAULT_HEIGHT: f32 = 1.7;
pub const DEFAULT_STEPHEIGHT: f32 = 0.5;

pub const ERROR_MARGIN: f32 = 5.0;
pub const MAX_HEIGHT_BEFORE_FLIGHT: f32 = 15.0;

#[derive(Clone)]
pub struct ServerPlayerContainer {
    pub player: ServerPlayer,
    pub entity_id: Option<EntityId>,
    pub connection: Connection,
}

struct PlayerPhysics {
    position: Vec3,
    head_rotation: Quaternion,
    rotation: Quaternion,
    pub scale: Vec3,
    physics_controller: Option<PhysicsCharacterController>,
    movement_speed: f32,
    last_move_call: Instant,
    height_gained_since_grounded: f32,
    last_height: f32,
}

impl Default for PlayerPhysics {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 0.0),
            head_rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
            last_move_call: Instant::now(),
            height_gained_since_grounded: 0.0,
            last_height: 0.0,
        }
    }
}

#[derive(Clone)]
pub struct ServerPlayer {
    pub uuid: Arc<String>,
    pub name: Arc<Mutex<String>>,
    physics: Arc<Mutex<PlayerPhysics>>,
    pub snowball_cooldown: Arc<Mutex<f32>>,
    pub pinging: Arc<AtomicBool>,
    pub respawning: Arc<AtomicBool>,
}

impl Default for ServerPlayer {
    fn default() -> Self {
        Self {
            uuid: Arc::new("".to_string()),
            name: Arc::new(Mutex::new("".to_string())),
            physics: Arc::new(Mutex::new(PlayerPhysics::default())),
            snowball_cooldown: Arc::new(Mutex::new(0.0)),
            pinging: Arc::new(AtomicBool::new(false)),
            respawning: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ServerPlayer {
    pub fn new(uuid: &str, name: &str, position: Vec3, rotation: Quaternion, scale: Vec3) -> Self {
        Self {
            uuid: Arc::new(uuid.to_string()),
            name: Arc::new(Mutex::new(name.to_string())),
            physics: Arc::new(Mutex::new(PlayerPhysics {
                position,
                rotation,
                scale,
                ..Default::default()
            })),
            snowball_cooldown: Arc::new(Mutex::new(0.0)),
            pinging: Arc::new(AtomicBool::new(false)),
            respawning: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn init(&self, physics_system: PhysicsSystem) {
        let mut physics = self.physics.lock().await;
        physics.physics_controller = physics_system.create_character_controller(DEFAULT_RADIUS, DEFAULT_HEIGHT, DEFAULT_STEPHEIGHT, Materials::Player);
        if physics.physics_controller.is_none() {
            warn!("failed to create physics controller for player");
        }
    }

    /// attempts to move the player to the given position, returning true if the move was successful, or false if the move was too fast.
    pub async fn attempt_position_change(&self, new_position: Vec3, displacement_vector: Vec3, new_rotation: Quaternion, new_head_rotation: Quaternion, movement_info: MovementInfo, entity_id: Option<EntityId>, worldmachine: Arc<mutex_timeouts::tokio::MutexWithTimeoutAuto<WorldMachine>>) -> (bool, Option<Vec3>) {
        // TODO!! IMPORTANT!! remember to check that the player is not trying to move vertically, or through a wall! displacement_vector should not contain a y value, and the new_position should be checked against the world to make sure it is not inside a wall.

        let last_position = self.get_position(None, None).await;

        // if any of the values are NaN, return false
        if new_position.x.is_nan() || new_position.y.is_nan() || new_position.z.is_nan() {
            return (false, Some(last_position));
        }

        if new_rotation.x.is_nan() || new_rotation.y.is_nan() || new_rotation.z.is_nan() || new_rotation.w.is_nan() {
            return (false, Some(last_position));
        }

        if new_head_rotation.x.is_nan() || new_head_rotation.y.is_nan() || new_head_rotation.z.is_nan() || new_head_rotation.w.is_nan() {
            return (false, Some(last_position));
        }

        if displacement_vector.x.is_nan() || displacement_vector.y.is_nan() || displacement_vector.z.is_nan() {
            return (false, Some(last_position));
        }

        if movement_info.speed.is_nan() || movement_info.strafe.is_nan() {
            return (false, Some(last_position));
        }

        let mut physics = self.physics.lock().await;

        if movement_info.sprinting {
            physics.movement_speed = DEFAULT_SPRINTSPEED;
        } else {
            physics.movement_speed = DEFAULT_MOVESPEED;
        }

        let mut displacement_vector = displacement_vector;
        displacement_vector.y = 0.0;
        displacement_vector = helpers::clamp_magnitude(displacement_vector, 1.0 * physics.movement_speed);

        let current_time = Instant::now();
        let last_move_call = physics.last_move_call;
        let delta = current_time.duration_since(last_move_call.clone()).as_secs_f32();
        displacement_vector *= delta;
        if delta >= 0.01 {
            physics.last_move_call = current_time;
            let _final_movement = physics.physics_controller.as_mut().unwrap().move_by(displacement_vector, movement_info.jumped, Some(false), false, delta, delta);
        }
        let current_time = Instant::now();
        let delta = current_time.duration_since(Instant::from(worldmachine.lock().await.last_physics_update)).as_secs_f32();
        if delta >= 0.01 {
            worldmachine.lock().await.physics.lock().unwrap().as_mut().unwrap().tick(delta);
            worldmachine.lock().await.last_physics_update = std::time::Instant::from(current_time);
        }
        let new_position_calculated = physics.physics_controller.as_mut().unwrap().get_foot_position();
        let distance = helpers::distance(new_position_calculated, new_position);
        if !physics.physics_controller.as_ref().unwrap().is_on_ground() {
            physics.height_gained_since_grounded += physics.last_height - new_position_calculated.y;
        } else {
            physics.height_gained_since_grounded = 0.0;
        }
        physics.last_height = new_position_calculated.y;

        if physics.height_gained_since_grounded > MAX_HEIGHT_BEFORE_FLIGHT {
            warn!("player {} is flying", self.uuid);
            return (false, Some(last_position));
        }

        drop(physics);

        if distance < ERROR_MARGIN {
            let mut wm = worldmachine.lock().await;
            self.set_position(new_position, entity_id, &mut wm).await;
            drop(wm);
            let mut wm = worldmachine.lock().await;
            self.set_rotation(new_rotation, entity_id, &mut wm).await;
            drop(wm);
            let mut wm = worldmachine.lock().await;
            self.set_head_rotation(new_head_rotation, entity_id, &mut wm).await;
            drop(wm);
            (true, None)
        } else {
            let mut wm = worldmachine.lock().await;
            self.set_position(new_position_calculated, entity_id, &mut wm).await;
            drop(wm);
            let mut wm = worldmachine.lock().await;
            self.set_rotation(new_rotation, entity_id, &mut wm).await;
            drop(wm);
            let mut wm = worldmachine.lock().await;
            self.set_head_rotation(new_head_rotation, entity_id, &mut wm).await;
            drop(wm);
            let position = self.get_position(None, None).await;
            (false, Some(position))
        }
    }

    pub async fn gravity_tick(&self) -> bool {
        let mut physics = self.physics.lock().await;
        let last_move_call = physics.last_move_call;
        let now = Instant::now();
        let mut previous_position = Vec3::default();
        let mut new_position = Vec3::default();
        if let Some(physics_controller) = physics.physics_controller.as_mut() {
            let delta = now.duration_since(last_move_call).as_secs_f32();
            if delta < 0.9 {
                return false;
            }
            previous_position = physics_controller.get_foot_position();
            physics_controller.move_by(Vec3::zero(), false, Some(true), false, delta, delta);
            new_position = physics_controller.get_foot_position();
            physics.last_move_call = now;
        }
        previous_position != new_position
    }

    pub async fn set_position(&self, position: Vec3, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        let mut physics = self.physics.lock().await;
        physics.position = position;
        if let Some(physics_controller) = physics.physics_controller.as_ref() {
            physics_controller.set_foot_position(position);
        }
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set position of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "position", ParameterValue::Vec3(position));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, position, physics.rotation, physics.head_rotation)).await;
            }
        }
    }

    pub async fn set_rotation(&self, rotation: Quaternion, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        let mut physics = self.physics.lock().await;
        physics.rotation = rotation;
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set rotation of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "rotation", ParameterValue::Quaternion(rotation));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, physics.position, rotation, physics.head_rotation)).await;
            }
        }
    }

    pub async fn set_head_rotation(&self, rotation: Quaternion, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        let mut physics = self.physics.lock().await;
        physics.head_rotation = rotation;
        if let Some(entity_id) = entity_id {
            let entity_index = worldmachine.get_entity_index(entity_id);
            if let None = entity_index {
                warn!("failed to set head rotation of entity: {}", entity_id);
            } else {
                let entity_index = entity_index.unwrap();
                worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "head_rotation", ParameterValue::Quaternion(rotation));
                worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, physics.position, physics.rotation, rotation)).await;
            }
        }
    }

    pub async fn set_scale(&self, scale: Vec3, entity_id: Option<EntityId>, worldmachine: &mut WorldMachine) {
        let mut physics = self.physics.lock().await;
        physics.scale = scale;
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

    pub async fn get_position(&self, entity_id: Option<EntityId>, worldmachine: Option<&mut WorldMachine>) -> Vec3 {
        let mut physics = self.physics.lock().await;
        let position = if let Some(physics_controller) = physics.physics_controller.as_ref() {
            physics_controller.get_foot_position()
        } else {
            physics.position
        };
        physics.position = position;
        if let Some(entity_id) = entity_id {
            if let Some(worldmachine) = worldmachine {
                let entity_index = worldmachine.get_entity_index(entity_id);
                if let None = entity_index {
                    warn!("failed to get position of entity: {}", entity_id);
                } else {
                    let entity_index = entity_index.unwrap();
                    worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "position", ParameterValue::Vec3(position));
                    worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, position, physics.rotation, physics.head_rotation)).await;
                }
            }
        }
        position
    }

    pub async fn get_rotation(&self, entity_id: Option<EntityId>, worldmachine: Option<&mut WorldMachine>) -> Quaternion {
        let physics = self.physics.lock().await;
        if let Some(entity_id) = entity_id {
            if let Some(worldmachine) = worldmachine {
                let entity_index = worldmachine.get_entity_index(entity_id);
                if let None = entity_index {
                    warn!("failed to get rotation of entity: {}", entity_id);
                } else {
                    let entity_index = entity_index.unwrap();
                    worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "rotation", ParameterValue::Quaternion(physics.rotation));
                    worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, physics.position, physics.rotation, physics.head_rotation)).await;
                }
            }
        }
        physics.rotation
    }

    pub async fn get_head_rotation(&self, entity_id: Option<EntityId>, worldmachine: Option<&mut WorldMachine>) -> Quaternion {
        let physics = self.physics.lock().await;
        if let Some(entity_id) = entity_id {
            if let Some(worldmachine) = worldmachine {
                let entity_index = worldmachine.get_entity_index(entity_id);
                if let None = entity_index {
                    warn!("failed to get head rotation of entity: {}", entity_id);
                } else {
                    let entity_index = entity_index.unwrap();
                    worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "head_rotation", ParameterValue::Quaternion(physics.head_rotation));
                    worldmachine.queue_update(WorldUpdate::MovePlayerEntity(entity_id, physics.position, physics.rotation, physics.head_rotation)).await;
                }
            }
        }
        physics.head_rotation
    }

    pub async fn get_scale(&self, entity_id: Option<EntityId>, worldmachine: Option<&mut WorldMachine>) -> Vec3 {
        let physics = self.physics.lock().await;
        if let Some(entity_id) = entity_id {
            if let Some(worldmachine) = worldmachine {
                let entity_index = worldmachine.get_entity_index(entity_id);
                if let None = entity_index {
                    warn!("failed to get scale of entity: {}", entity_id);
                } else {
                    let entity_index = entity_index.unwrap();
                    worldmachine.world.entities[entity_index].set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "scale", ParameterValue::Vec3(physics.scale));
                    worldmachine.queue_update(WorldUpdate::SetScale(entity_id, physics.scale)).await;
                }
            }
        }
        physics.scale
    }
}