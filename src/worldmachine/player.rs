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
    pitch: f64,
    pub scale: Vec3,
    last_mouse_pos: Option<Vec2>,
    physics_controller: Option<PhysicsCharacterController>,
    movement_speed: f32,
    wasd: [bool; 4],
    head_rotation_changed: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            uuid: "".to_string(),
            name: "".to_string(),
            position: Vec3::new(0.0, 0.0, 0.0),
            head_rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            pitch: 0.0,
            scale: Vec3::new(1.0, 1.0, 1.0),
            last_mouse_pos: None,
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
            wasd: [false; 4],
            head_rotation_changed: false
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

        let ang_x = -(mouse_pos.x as f64 - last_mouse_pos.x as f64);
        let ang_y = -(mouse_pos.y as f64 - last_mouse_pos.y as f64);
        self.last_mouse_pos = Some(mouse_pos);

        let camera = &mut renderer.camera;
        if self.head_rotation_changed {
            self.head_rotation_changed = false;
            //camera.set_rotation(self.get_head_rotation());
        }
        let camera_rotation = camera.get_rotation();
        let mut pitch = self.pitch;
        let mut yaw = 0.0;
        let original_yaw = yaw;
        let original_pitch = pitch;
        yaw += ang_x;
        pitch += ang_y;

        if pitch > 89.0 {
            pitch = 89.0;
        }
        if pitch < -89.0 {
            pitch = -89.0;
        }
        if pitch > 360.0 {
            pitch -= 360.0;
        }

        self.pitch = pitch;

        yaw -= original_yaw;
        pitch -= original_pitch;


        let horiz = Quaternion::from_euler_angles_zyx(&Vec3::new(0.0, yaw as f32, 0.0));
        let vert = Quaternion::from_euler_angles_zyx(&Vec3::new(pitch as f32, 0.0, 0.0));

        let new_camera_rotation = vert * camera_rotation * horiz;

        camera.set_rotation(new_camera_rotation);

        let head_rotation = horiz * camera_rotation * vert;
        self.set_head_rotation(head_rotation);
        let rotation_no_pitch = horiz * camera_rotation;
        self.set_rotation(rotation_no_pitch);
        self.head_rotation_changed = false;

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
        let speed = self.movement_speed;
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