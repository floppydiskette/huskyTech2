use crate::worldmachine::components::*;
use crate::worldmachine::ecs::*;
use crate::worldmachine::EntityId;

impl Entity {
    pub fn new(name: &str) -> Entity {
        Self {
            name: name.to_string(),
            uid: ENTITY_ID_MANAGER.lock().unwrap().get_id(),
            components: Vec::new(),
            children: Vec::new(),
            parent: None,
        }
    }

    // SHOULD ONLY BE USED IF YOU ARE __SURE__ THAT THIS ENTITY DOESN'T ALREADY EXIST
    pub unsafe fn new_with_id(name: &str, uid: EntityId) -> Entity {
        warn!("Creating entity with id: {}", uid);
        Self {
            name: name.to_string(),
            uid,
            components: Vec::new(),
            children: Vec::new(),
            parent: None,
        }
    }

    pub fn copy_data_from_other_entity(&mut self, other: &Entity) {
        self.components = other.components.clone();
        self.children = other.children.clone();
        self.parent = other.parent;
    }

    pub fn add_component(&mut self, component: Component) {
        // check if we already have a component of this type
        for existing_component in &self.components {
            if existing_component.component_type == component.component_type {
                return;
            }
        }
        self.components.push(component);
    }

    pub fn has_component(&self, component_type: ComponentType) -> bool {
        for existing_component in &self.components {
            if existing_component.component_type == component_type {
                return true;
            }
        }
        return false;
    }

    pub fn remove_component(&mut self, component_type: ComponentType) {
        self.components.retain(|component| component.component_type != component_type);
    }

    pub fn to_entity_def(&self) -> EntityDef {
        EntityDef {
            name: self.name.clone(),
            components: self.components.clone(),
        }
    }

    pub fn from_entity_def(entity_def: &EntityDef) -> Entity {
        Entity {
            name: entity_def.name.clone(),
            uid: ENTITY_ID_MANAGER.lock().unwrap().get_id(),
            components: entity_def.components.clone(),
            children: Vec::new(),
            parent: None,
        }
    }
}

pub fn new_ht2_entity() -> Entity {
    let mut entity = Entity::new("ht2");
    entity.add_component(Transform::default());
    entity.add_component(MeshRenderer::default());
    entity
}