use std::collections::BTreeMap;
use gfx_maths::*;
use crate::{helpers, ht_renderer, mouse};
use crate::worldmachine::components::COMPONENT_TYPE_PLAYER;
use crate::worldmachine::ecs::*;
use crate::worldmachine::{EntityId};

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

#[derive(Clone, Debug)]
pub struct PlayerContainer {
    pub player: Player,
    pub entity_id: Option<EntityId>,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub uuid: String,
    pub name: String,
    pub position: Vec3,
    pub head_rotation: Quaternion,
    pub rotation: Quaternion,
    pub scale: Vec3,
    last_mouse_pos: Option<Vec2>
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
            last_mouse_pos: None
        }
    }
}

impl Player {
    pub fn handle_input(&mut self, renderer: &mut ht_renderer, delta_time: f32) {
        let mouse_pos = mouse::get_mouse_pos();

        if self.last_mouse_pos.is_none() {
            self.last_mouse_pos = Some(mouse_pos);
        }
        let last_mouse_pos = self.last_mouse_pos.unwrap();
        let mouse_x_offset = mouse_pos.x - last_mouse_pos.x;
        let mouse_y_offset = mouse_pos.y - last_mouse_pos.y;

        let camera = &mut renderer.camera;

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
    }
}