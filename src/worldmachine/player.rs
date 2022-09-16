use std::collections::{BTreeMap, VecDeque};
use gfx_maths::*;
use crate::{helpers, ht_renderer, Key, keyboard, mouse};
use crate::physics::{ClimbingMode, Materials, PhysicsCharacterController, PhysicsSystem};
use crate::server::server_player::{DEFAULT_HEIGHT, DEFAULT_MOVESPEED, DEFAULT_RADIUS, DEFAULT_STEPHEIGHT};
use crate::worldmachine::components::COMPONENT_TYPE_PLAYER;
use crate::worldmachine::ecs::*;
use crate::worldmachine::{ClientUpdate, EntityId, WorldMachine};

pub struct PlayerComponent {}

#[allow(clippy::new_ret_no_self)]
impl PlayerComponent {
    pub fn new(name: &str, position: Vec3, rotation: Quaternion, scale: Vec3) -> Component {
        let mut parameters = BTreeMap::new();
        let uuid = uuid::Uuid::new_v4().to_string();
        parameters.insert("uuid".to_string(), Parameter::new("uuid", ParameterValue::String(uuid)));
        parameters.insert("name".to_string(), Parameter::new("name", ParameterValue::String(name.to_string())));
        parameters.insert("position".to_string(), Parameter::new("position", ParameterValue::Vec3(position)));
        parameters.insert("head_rotation".to_string(), Parameter::new("head_rotation", ParameterValue::Quaternion(rotation)));
        parameters.insert("rotation".to_string(), Parameter::new("rotation", ParameterValue::Quaternion(rotation)));
        parameters.insert("scale".to_string(), Parameter::new("scale", ParameterValue::Vec3(scale)));

        Component {
            name: "Player".to_string(),
            parameters,
            component_type: COMPONENT_TYPE_PLAYER.clone(),
        }
    }
    pub fn default() -> Component {
        Self::new("player", Vec3::new(0.0, 0.0, 0.0), Quaternion::new(0.0, 0.0, 0.0, 1.0), Vec3::new(1.0, 1.0, 1.0))
    }
}

#[derive(Clone)]
pub struct PlayerContainer {
    pub player: Player,
    pub entity_id: Option<EntityId>,
}

#[derive(Clone)]
pub struct Player {
    pub uuid: String,
    pub name: String,
    position: Vec3,
    head_rotation: Quaternion,
    rotation: Quaternion,
    pub scale: Vec3,
    last_mouse_pos: Option<Vec2>,
    physics_controller: Option<PhysicsCharacterController>,
    movement_speed: f32,
    wasd: [bool; 4],
}

impl Default for Player {
    fn default() -> Self {
        Self {
            uuid: "".to_string(),
            name: "".to_string(),
            position: Vec3::new(0.0, 0.0, 0.0),
            head_rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            last_mouse_pos: None,
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
            wasd: [false; 4],
        }
    }
}

impl Player {
    pub fn init(&mut self, physics_system: PhysicsSystem, uuid: String, name: String, position: Vec3, rotation: Quaternion, scale: Vec3) {
        self.physics_controller = physics_system.create_character_controller(DEFAULT_RADIUS, DEFAULT_HEIGHT, DEFAULT_STEPHEIGHT, Materials::Player);
        if self.physics_controller.is_none() {
            warn!("failed to create physics controller for player");
        }
        self.uuid = uuid;
        self.name = name;
        self.position = position;
        self.head_rotation = rotation;
        self.rotation = rotation;
        self.scale = scale;
    }

    fn handle_mouse_movement(&mut self, renderer: &mut ht_renderer, delta_time: f32) -> Option<Quaternion> {
        let mouse_pos = mouse::get_mouse_pos();

        if self.last_mouse_pos.is_none() {
            self.last_mouse_pos = Some(mouse_pos);
        }
        let last_mouse_pos = self.last_mouse_pos.unwrap();
        let mouse_x_offset = mouse_pos.x - last_mouse_pos.x;
        let mouse_y_offset = mouse_pos.y - last_mouse_pos.y;

        let camera = &mut renderer.camera;
        let camera_rotation = camera.get_rotation();

        let mut yaw = helpers::get_quaternion_yaw(camera.get_rotation());
        let mut pitch = helpers::get_quaternion_pitch(camera.get_rotation());
        yaw += -mouse_x_offset;
        pitch += -mouse_y_offset;
        if pitch > 89.0 {
            pitch = 89.0;
        }
        if pitch < -89.0 {
            pitch = -89.0;
        }
        let mut rotation = Quaternion::identity();
        rotation = Quaternion::from_euler_angles_zyx(&Vec3::new(pitch, 0.0, 0.0)) * rotation * Quaternion::from_euler_angles_zyx(&Vec3::new(0.0, yaw, 0.0));
        camera.set_rotation(rotation);

        if camera.get_rotation() != camera_rotation {
            Some(camera.get_rotation())
        } else {
            None
        }
    }

    fn handle_keyboard_movement(&mut self, renderer: &mut ht_renderer, delta_time: f32) -> Option<Vec3> {
        let mut movement = Vec3::new(0.0, 0.0, 0.0);
        let camera = &mut renderer.camera;
        let camera_rotation = camera.get_rotation();
        let camera_forward = camera.get_forward_no_pitch();
        let camera_right = camera.get_right();
        let camera_up = camera.get_up();
        let speed = 2.0;//self.movement_speed;
        if keyboard::check_key_pressed(Key::W) {
            self.wasd[0] = true;
        }
        if keyboard::check_key_released(Key::W) {
            self.wasd[0] = false;
        }
        if keyboard::check_key_pressed(Key::A) {
            self.wasd[1] = true;
        }
        if keyboard::check_key_released(Key::A) {
            self.wasd[1] = false;
        }
        if keyboard::check_key_pressed(Key::S) {
            self.wasd[2] = true;
        }
        if keyboard::check_key_released(Key::S) {
            self.wasd[2] = false;
        }
        if keyboard::check_key_pressed(Key::D) {
            self.wasd[3] = true;
        }
        if keyboard::check_key_released(Key::D) {
            self.wasd[3] = false;
        }
        if self.wasd[0] {
            movement += camera_forward;
        }
        if self.wasd[1] {
            movement += camera_right;
        }
        if self.wasd[2] {
            movement -= camera_forward;
        }
        if self.wasd[3] {
            movement -= camera_right;
        }
        movement.y = 0.0;
        movement = helpers::clamp_magnitude(movement, 1.0);
        movement *= speed;
        self.physics_controller.as_mut().unwrap().move_by(movement, delta_time);
        camera.set_position_from_player_position(self.physics_controller.as_ref().unwrap().get_position());
        if movement != Vec3::new(0.0, 0.0, 0.0) {
            Some(movement)
        } else {
            None
        }
    }

    pub fn handle_input(&mut self, renderer: &mut ht_renderer, delta_time: f32) -> Option<Vec<ClientUpdate>> {
        let look = self.handle_mouse_movement(renderer, delta_time);
        let movement = self.handle_keyboard_movement(renderer, delta_time);

        let mut updates = Vec::new();
        if let Some(look) = look {
            updates.push(ClientUpdate::ILooked(look));
        }
        if let Some(movement) = movement {
            updates.push(ClientUpdate::IDisplaced(movement)); // using displaced as the returned value is a displacement vector for the physics engine
        }

        if updates.is_empty() {
            None
        } else {
            Some(updates)
        }
    }

    pub fn get_position(&mut self) -> Vec3 {
        let position = self.physics_controller.as_mut().unwrap().get_position();
        self.position = position;
        position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.physics_controller.as_mut().unwrap().set_position(position);
        self.position = position;
    }

    pub fn get_rotation(&mut self) -> Quaternion {
        self.rotation
    }

    pub fn set_rotation(&mut self, rotation: Quaternion) {
        self.rotation = rotation;
    }

    pub fn get_head_rotation(&mut self) -> Quaternion {
        self.head_rotation
    }

    pub fn set_head_rotation(&mut self, head_rotation: Quaternion) {
        self.head_rotation = head_rotation;
    }
}