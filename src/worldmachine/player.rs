use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;
use gfx_maths::*;
use serde::{Deserialize, Serialize};
use crate::{helpers, ht_renderer, keyboard, mouse};
use crate::helpers::lerp;
use crate::keyboard::HTKey;
use crate::physics::{ClimbingMode, Materials, PhysicsCharacterController, PhysicsSystem};
use crate::server::server_player::{DEFAULT_HEIGHT, DEFAULT_MOVESPEED, DEFAULT_RADIUS, DEFAULT_SPRINTSPEED, DEFAULT_STEPHEIGHT};
use crate::worldmachine::components::COMPONENT_TYPE_PLAYER;
use crate::worldmachine::ecs::*;
use crate::worldmachine::{ClientUpdate, EntityId, WorldMachine};

pub const DEFAULT_FOV: f32 = 120.0;
pub const SPRINT_FOV: f32 = 140.0;

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
        parameters.insert("sprinting".to_string(), Parameter::new("sprinting", ParameterValue::Bool(false)));
        parameters.insert("speed".to_string(), Parameter::new("speed", ParameterValue::Float(0.0)));
        parameters.insert("strafe".to_string(), Parameter::new("strafe", ParameterValue::Float(0.0)));

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
    yaw: f64,
    pub scale: Vec3,
    pub speed: f64,
    pub strafe: f64,
    sprinting: bool,
    last_mouse_pos: Option<Vec2>,
    physics_controller: Option<PhysicsCharacterController>,
    movement_speed: f32,
    last_move_call: std::time::Instant,
    wasd: [bool; 4],
    jump: bool,
    head_rotation_changed: bool,
    locked_mouse: bool,
    first_run: bool,
    was_moving: bool,
    bob_t: f32,
    bob_on: bool,
    pub has_camera_control: bool,
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
            yaw: 0.0,
            scale: Vec3::new(1.0, 1.0, 1.0),
            speed: 0.0,
            strafe: 0.0,
            sprinting: false,
            last_mouse_pos: None,
            physics_controller: None,
            movement_speed: DEFAULT_MOVESPEED,
            last_move_call: std::time::Instant::now(),
            wasd: [false; 4],
            jump: false,
            head_rotation_changed: false,
            locked_mouse: true,
            first_run: true,
            was_moving: false,
            bob_t: 0.0,
            bob_on: true,
            has_camera_control: true,
        }
    }
}

#[derive(Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MovementInfo {
    pub jumped: bool,
    pub sprinting: bool,
    pub speed: f32,
    pub strafe: f32,
}

impl Player {
    pub fn init(&mut self, physics_system: PhysicsSystem, uuid: String, name: String, position: Vec3, rotation: Quaternion, scale: Vec3) {
        self.physics_controller = physics_system.create_character_controller(DEFAULT_RADIUS, DEFAULT_HEIGHT, DEFAULT_STEPHEIGHT, Materials::Player);
        self.calculate_pitch_and_yaw_from_rotation(rotation);
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
        if !self.locked_mouse {
            return None;
        }

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
        let mut yaw = self.yaw;
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

        if yaw > 360.0 {
            yaw -= 360.0;
        }

        self.pitch = pitch;
        self.yaw = yaw;

        yaw -= original_yaw;
        pitch -= original_pitch;


        let horiz = Quaternion::from_euler_angles_zyx(&Vec3::new(0.0, yaw as f32, 0.0));
        let vert = Quaternion::from_euler_angles_zyx(&Vec3::new(pitch as f32, 0.0, 0.0));

        let new_camera_rotation = vert * camera_rotation * horiz;

        camera.set_rotation(new_camera_rotation);

        self.set_head_rotation(new_camera_rotation);
        let rotation_no_pitch = self.rotation * -horiz;
        self.set_rotation(rotation_no_pitch);
        self.head_rotation_changed = false;

        if camera.get_rotation() != camera_rotation {
            Some(camera.get_rotation())
        } else {
            None
        }
    }

    fn handle_keyboard_movement(&mut self, renderer: &mut ht_renderer, jump: bool, delta_time: f32) -> Option<(Vec3, MovementInfo)> {
        let mut movement = Vec3::new(0.0, 0.0, 0.0);
        let camera = &mut renderer.camera;
        let camera_rotation = camera.get_rotation();
        let camera_forward = camera.get_forward_no_pitch();
        let camera_right = camera.get_right();
        let camera_up = camera.get_up();
        let mut speed = self.movement_speed;
        //let speed = 10.0; // uncomment to cheat!

        let mut info = MovementInfo::default();

        if keyboard::check_key_pressed(HTKey::W) {
            self.wasd[0] = true;
        }
        if keyboard::check_key_released(HTKey::W) {
            self.wasd[0] = false;
        }
        if keyboard::check_key_pressed(HTKey::A) {
            self.wasd[1] = true;
        }
        if keyboard::check_key_released(HTKey::A) {
            self.wasd[1] = false;
        }
        if keyboard::check_key_pressed(HTKey::S) {
            self.wasd[2] = true;
        }
        if keyboard::check_key_released(HTKey::S) {
            self.wasd[2] = false;
        }
        if keyboard::check_key_pressed(HTKey::D) {
            self.wasd[3] = true;
        }
        if keyboard::check_key_released(HTKey::D) {
            self.wasd[3] = false;
        }
        if keyboard::check_key_down(HTKey::LeftShift) {
            self.sprinting = true;
        }
        if keyboard::check_key_released(HTKey::LeftShift) {
            self.sprinting = false;
        }
        self.speed = 0.0;
        self.strafe = 0.0;
        if self.wasd[0] {
            self.speed = lerp(0.0, 1.0, 1.0) as f64;
            movement += camera_forward;
        }
        if self.wasd[1] {
            self.strafe = lerp(0.0, -1.0, 1.0) as f64;
            movement += camera_right;
        }
        if self.wasd[2] {
            self.speed = lerp(0.0, -1.0, 1.0) as f64;
            movement -= camera_forward;
        }
        if self.wasd[3] {
            self.strafe = lerp(0.0, 1.0, 1.0) as f64;
            movement -= camera_right;
        }
        if self.sprinting {
            info.sprinting = false;
            speed = DEFAULT_SPRINTSPEED;
        } else {
            info.sprinting = false;
        }
        info.speed = self.speed as f32;
        info.strafe = self.strafe as f32;
        movement = helpers::clamp_magnitude(movement, 1.0);

        if self.sprinting && movement.magnitude() > 0.0 {
            camera.set_fov(lerp(camera.get_fov(), DEFAULT_FOV + 10.0, 0.1));
        } else {
            camera.set_fov(lerp(camera.get_fov(), DEFAULT_FOV, 0.1));
        }

        movement *= speed;

        movement.y = 0.0;
        //let delta_time = std::time::Instant::now().duration_since(self.last_move_call).as_secs_f32();
        self.physics_controller.as_mut().unwrap().move_by(movement, jump, false,false, delta_time);
        // uncomment next three lines for FLIGHT
        //let mut position = self.physics_controller.as_ref().unwrap().get_position();
        //position.y += 5.0;
        //self.physics_controller.as_mut().unwrap().set_position(position);

        *crate::ui::DEBUG_LOCATION.lock().unwrap() = self.physics_controller.as_ref().unwrap().get_position();

        self.last_move_call = std::time::Instant::now();
        //camera.set_position_from_player_position(self.physics_controller.as_ref().unwrap().get_position());
        if movement != Vec3::new(0.0, 0.0, 0.0) {
            self.was_moving = true;
            Some((movement, info))
        } else if self.was_moving {
            self.was_moving = false;
            Some((movement, info))
        } else {
            None
        }
    }

    fn handle_jump(&mut self, renderer: &mut ht_renderer, delta_time: f32) -> bool {
        if keyboard::check_key_down(HTKey::Space) {
            return true;
        }
        false
    }

    pub fn handle_input(&mut self, renderer: &mut ht_renderer, delta_time: f32) -> Option<Vec<ClientUpdate>> {
        if self.first_run {
            self.first_run = false;
            self.locked_mouse = true;
            renderer.lock_mouse(true);
        }

        let jump = self.handle_jump(renderer, delta_time);
        let look = self.handle_mouse_movement(renderer, delta_time);
        let movement = self.handle_keyboard_movement(renderer, jump, delta_time);

        // FOR DEBUGGING, REMOVE LATER
        if keyboard::check_key_released(HTKey::Comma) {
            debug!("unlocking mouse");
            renderer.lock_mouse(false);
            self.locked_mouse = false;
        } else if keyboard::check_key_released(HTKey::Period) {
            debug!("locking mouse");
            renderer.lock_mouse(true);
            self.locked_mouse = true;
        }

        let mut updates = Vec::new();
        if jump {
            updates.push(ClientUpdate::IJumped);
            if let Some(movement) = movement {
                let mut new_movement = movement.1;
                new_movement.jumped = true;
                updates.push(ClientUpdate::IDisplaced((movement.0, Some(new_movement)))); // using displaced as the returned value is a displacement vector for the physics engine
            } else {
                updates.push(ClientUpdate::IDisplaced((Vec3::zero(), None)));
            }
        }
        if let Some(look) = look {
            updates.push(ClientUpdate::ILooked(look));
        }

        // how much do we bob?
        let mut bob_mag = 0.0;

        if let Some(movement) = movement {
            let mut new_movement = movement.1;
            new_movement.jumped = jump;
            updates.push(ClientUpdate::IDisplaced((movement.0, Some(movement.1)))); // using displaced as the returned value is a displacement vector for the physics engine
            bob_mag = movement.0.magnitude() * 5.0;
            debug!("bob mag: {}", bob_mag);
        }

        // for lerp
        self.bob_t += delta_time;

        // head bob
        if self.bob_on {
            let initial_head = self.get_position() + Vec3::new(0.0, 1.68, 0.0);
            let bob = if bob_mag != 0.0 { initial_head + Vec3::new(0.0, 0.1 * ((self.bob_t  * 17.0).sin() * bob_mag), 0.0) } else {
                self.bob_t = 0.0;
                helpers::lerp_vec3(initial_head, renderer.camera.get_position(), self.bob_t)
            };
            renderer.camera.set_position(bob);
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

    fn calculate_pitch_and_yaw_from_rotation(&mut self, rotation: Quaternion) {
        let rotation = rotation.to_euler_angles_zyx();
        // todo! make this do something
    }

    pub fn set_rotation(&mut self, rotation: Quaternion) {
        self.rotation = rotation;
        self.calculate_pitch_and_yaw_from_rotation(rotation);
    }

    pub fn get_head_rotation(&mut self) -> Quaternion {
        self.head_rotation
    }

    pub fn set_head_rotation(&mut self, head_rotation: Quaternion) {
        self.head_rotation = head_rotation;
    }
}