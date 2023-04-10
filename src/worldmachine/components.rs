#![allow(clippy::new_ret_no_self)]

use std::collections::BTreeMap;
use gfx_maths::*;
use crate::worldmachine::ecs::*;

lazy_static! {
    pub static ref COMPONENT_TYPE_PLAYER: ComponentType = ComponentType::create_if_not_exists("Player");
    pub static ref COMPONENT_TYPE_TRANSFORM: ComponentType = ComponentType::create_if_not_exists("Transform");
    pub static ref COMPONENT_TYPE_MESH_RENDERER: ComponentType = ComponentType::create_if_not_exists("MeshRenderer");
    pub static ref COMPONENT_TYPE_TERRAIN: ComponentType = ComponentType::create_if_not_exists("Terrain");
    pub static ref COMPONENT_TYPE_LIGHT: ComponentType = ComponentType::create_if_not_exists("Light");
    pub static ref COMPONENT_TYPE_BOX_COLLIDER: ComponentType = ComponentType::create_if_not_exists("BoxCollider");
    pub static ref COMPONENT_TYPE_JUKEBOX: ComponentType = ComponentType::create_if_not_exists("Jukebox");
    pub static ref COMPONENT_TYPE_TRIGGER: ComponentType = ComponentType::create_if_not_exists("Trigger");

    pub static ref COMPONENTS_INITIALISED: bool = {
        register_component_types();
        true
    };
}

pub fn register_component_types() {
    // this is kinda dumb, but in order to get all component types registered, we need to make them all be referenced
    let _ = COMPONENT_TYPE_PLAYER.clone();
    let _ = COMPONENT_TYPE_TRANSFORM.clone();
    let _ = COMPONENT_TYPE_MESH_RENDERER.clone();
    let _ = COMPONENT_TYPE_TERRAIN.clone();
    let _ = COMPONENT_TYPE_LIGHT.clone();
    let _ = COMPONENT_TYPE_BOX_COLLIDER.clone();
    let _ = COMPONENT_TYPE_JUKEBOX.clone();
    let _ = COMPONENT_TYPE_TRIGGER.clone();
}

// player component is defined in src/worldmachine/player.rs

pub struct Transform {}

impl Transform {
    pub fn new(position: Vec3, rotation: Quaternion, scale: Vec3) -> Component {
        let mut parameters = BTreeMap::new();
        parameters.insert("position".to_string(), Parameter::new("position", ParameterValue::Vec3(position)));
        parameters.insert("rotation".to_string(), Parameter::new("rotation", ParameterValue::Quaternion(rotation)));
        parameters.insert("scale".to_string(), Parameter::new("scale", ParameterValue::Vec3(scale)));

        Component {
            name: "Transform".to_string(),
            parameters,
            component_type: COMPONENT_TYPE_TRANSFORM.clone(),
        }
    }
    pub fn default() -> Component {
        Self::new(Vec3::new(0.0, 0.0, 0.0), Quaternion::new(0.0, 0.0, 0.0, 1.0), Vec3::new(1.0, 1.0, 1.0))
    }
}

pub struct MeshRenderer {}

impl MeshRenderer {
    pub fn new(mesh: String, shader: String, texture: String) -> Component {
        let mut parameters = BTreeMap::new();
        parameters.insert("mesh".to_string(), Parameter::new("mesh", ParameterValue::String(mesh)));
        parameters.insert("shader".to_string(), Parameter::new("shader", ParameterValue::String(shader)));
        parameters.insert("texture".to_string(), Parameter::new("texture", ParameterValue::String(texture)));

        Component {
            name: "MeshRenderer".to_string(),
            parameters,
            component_type: COMPONENT_TYPE_MESH_RENDERER.clone(),
        }
    }
    pub fn default() -> Component {
        Self::new("ht2".to_string(), "gbuffer".to_string(), "default".to_string())
    }
}

pub struct Light {}

impl Light {
    pub fn new(position: Vec3, colour: Vec3, intensity: f64) -> Component {
        let mut parameters = BTreeMap::new();
        parameters.insert("position".to_string(), Parameter::new("position", ParameterValue::Vec3(position)));
        parameters.insert("colour".to_string(), Parameter::new("colour", ParameterValue::Vec3(colour)));
        parameters.insert("intensity".to_string(), Parameter::new("intensity", ParameterValue::Float(intensity)));

        Component {
            name: "Light".to_string(),
            parameters,
            component_type: COMPONENT_TYPE_LIGHT.clone(),
        }
    }
    pub fn default() -> Component {
        Self::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0), 1.0)
    }
}

pub struct Terrain {}

impl Terrain {
    pub fn new(name: &str) -> Component {
        let mut parameters = BTreeMap::new();
        parameters.insert("name".to_string(), Parameter::new("name", ParameterValue::String(name.to_string())));

        Component {
            name: "Terrain".to_string(),
            parameters,
            component_type: COMPONENT_TYPE_TERRAIN.clone(),
        }
    }
    pub fn default() -> Component {
        Self::new("default")
    }
}

pub struct Jukebox {}

impl Jukebox {
    pub fn new() -> Component {
        let mut parameters = BTreeMap::new();
        parameters.insert("volume".to_string(), Parameter::new("volume", ParameterValue::Float(1.0)));
        parameters.insert("playing".to_string(), Parameter::new("playing", ParameterValue::Bool(false)));
        parameters.insert("track".to_string(), Parameter::new("track", ParameterValue::String("".to_string())));
        parameters.insert("uuid".to_string(), Parameter::new("uuid", ParameterValue::String("".to_string())));

        Component {
            name: "Jukebox".to_string(),
            parameters,
            component_type: COMPONENT_TYPE_JUKEBOX.clone(),
        }
    }
    pub fn default() -> Component {
        Self::new()
    }
}