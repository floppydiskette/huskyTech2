use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use gfx_maths::{Quaternion, Vec2, Vec3};
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeStruct;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub value: ParameterValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    Vec3(Vec3),
    Quaternion(Quaternion),
    Vec2(Vec2),
    Float(f64),
    Int(i32),
    UnsignedInt(u64),
    Bool(bool),
    String(String),
}

impl Parameter {
    pub(crate) fn new(name: &str, value: ParameterValue) -> Parameter {
        Self {
            name: name.to_string(),
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub parameters: BTreeMap<String, Parameter>,
    pub component_type: ComponentType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub uid: u64,
    pub components: Vec<Component>,
    pub children: Vec<Entity>,
    pub parent: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct System {
    pub name: String,
    pub uid: u64,
    pub affected_entities: Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    pub components: Vec<Component>,
}

impl Component {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_parameters(&self) -> &BTreeMap<String, Parameter> {
        &self.parameters
    }

    pub fn get_type(&self) -> ComponentType {
        self.component_type.clone()
    }

    pub fn get_parameter(&self, name: &str) -> Option<&Parameter> {
        self.parameters.get(name)
    }
}

impl Entity {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_id(&self) -> u64 {
        self.uid
    }

    pub fn get_components(&self) -> &Vec<Component> {
        &self.components
    }

    pub fn get_component(&self, component_type: ComponentType) -> Option<&Component> {
        for component in &self.components {
            if component.component_type == component_type {
                return Some(component);
            }
        }
        None
    }

    pub fn set_component_parameter(&mut self, component_type: ComponentType, parameter_name: &str, value: ParameterValue) {
        for component in self.components.iter_mut() {
            if component.component_type == component_type {
                if let Some(parameter) = component.parameters.get_mut(parameter_name) {
                    parameter.value = value.clone();
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentType {
    pub id: u64,
    pub name: String,
}

impl ComponentType {
    pub fn create(name: &str) {
        let id = COMPONENT_ID_MANAGER.lock().unwrap().get_id();
        let mut hashmap = COMPONENT_TYPES.lock().unwrap();
        let component_type = Self {
            id,
            name: name.to_string(),
        };
        hashmap.insert(name.to_string(), component_type);
    }

    pub fn create_if_not_exists(name: &str) -> Self {
        debug!("Creating component type {}", name);
        let mut hashmap = COMPONENT_TYPES.lock().unwrap();
        hashmap.entry(name.to_string()).or_insert_with(|| {
            let id = COMPONENT_ID_MANAGER.lock().unwrap().get_id();
            let component_type = Self {
                id,
                name: name.to_string(),
            };
            component_type.clone()
        }).deref().clone()
    }

    pub fn get(name: String) -> Option<Self> {
        COMPONENT_TYPES.lock().unwrap().get(&*name).cloned()
    }
}

impl System {
    pub fn create(hashmap: &mut HashMap<String, Self>, name: String) {
        let id = SYSTEM_ID_MANAGER.lock().unwrap().get_id();
        let system_type = Self {
            name: name.clone(),
            uid: id,
            affected_entities: vec![]
        };
        hashmap.insert(name, system_type);
    }

    pub fn get(name: String) -> Option<Self> {
        SYSTEM_TYPES.lock().unwrap().get(&*name).cloned()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComponentIDManager {
    pub id: u64,
}

#[derive(Clone, Debug, Default)]
pub struct SystemIDManager {
    pub id: u64,
}

#[derive(Clone, Debug, Default)]
pub struct EntityIDManager {
    pub id: u64,
}

impl ComponentIDManager {
    pub fn get_id(&mut self) -> u64 {
        self.id += 1;
        self.id
    }
}

impl SystemIDManager {
    pub fn get_id(&mut self) -> u64 {
        self.id += 1;
        self.id
    }
}

impl EntityIDManager {
    pub fn get_id(&mut self) -> u64 {
        self.id += 1;
        self.id
    }
}

lazy_static! {
    pub static ref COMPONENT_ID_MANAGER: Mutex<ComponentIDManager> = Mutex::new(ComponentIDManager::default());
    pub static ref COMPONENT_TYPES: Mutex<HashMap<String, ComponentType>> = {
        let mut m = HashMap::new();
        Mutex::new(m)
    };
    pub static ref SYSTEM_ID_MANAGER: Mutex<SystemIDManager> = Mutex::new(SystemIDManager::default());
    pub static ref SYSTEM_TYPES: Mutex<HashMap<String, System>> = {
        let mut m = HashMap::new();
        Mutex::new(m)
    };
    pub static ref ENTITY_ID_MANAGER: Mutex<EntityIDManager> = Mutex::new(EntityIDManager::default());
}