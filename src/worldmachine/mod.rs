use std::any::Any;
use std::borrow::{Borrow, BorrowMut};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};
use gfx_maths::{Quaternion, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use crate::camera::Camera;
use crate::{ht_renderer, renderer};
use crate::physics::PhysicsSystem;
use crate::worldmachine::components::{COMPONENT_TYPE_LIGHT, COMPONENT_TYPE_MESH_RENDERER, COMPONENT_TYPE_TERRAIN, COMPONENT_TYPE_TRANSFORM, Light, MeshRenderer, Terrain, Transform};
use crate::worldmachine::ecs::*;
use crate::worldmachine::entities::new_ht2_entity;

pub mod ecs;
pub mod components;
pub mod entities;
pub mod helpers;

pub type EntityId = u64;

#[derive(Deserialize, Serialize)]
pub struct World {
    pub entities: Vec<Entity>,
    pub systems: Vec<System>,
    eid_manager: EntityId,
}

#[derive(Deserialize, Serialize)]
pub struct WorldDef {
    pub name: String,
    pub world: World,
}

impl Clone for World {
    fn clone(&self) -> Self {
        let mut entities = Vec::new();
        for entity in &self.entities {
            entities.push(entity.deref().clone());
        }
        let mut systems = Vec::new();
        for system in &self.systems {
            systems.push(system.deref().clone());
        }
        World {
            entities,
            systems,
            eid_manager: 0,
        }
    }
}

pub struct WorldMachine {
    pub world: World,
    pub physics: Option<PhysicsSystem>,
    pub game_data_path: String,
    pub counter: f32,
    pub entities_wanting_to_load_things: Vec<usize>, // index
    lights_changed: bool,
}

impl Default for WorldMachine {
    fn default() -> Self {
        let world = World {
            entities: Vec::new(),
            systems: Vec::new(),
            eid_manager: 0,
        };
        Self {
            world,
            physics: None,
            game_data_path: String::from(""),
            counter: 0.0,
            entities_wanting_to_load_things: Vec::new(),
            lights_changed: true,
        }
    }
}

impl WorldMachine {
    pub fn initialise(&mut self, physics: PhysicsSystem) {
        self.game_data_path = String::from("base");
        components::register_component_types();
        self.physics = Some(physics);

        self.blank_slate();
    }

    // resets the world to a blank slate
    pub fn blank_slate(&mut self) {
        {
            let mut eid_manager = ENTITY_ID_MANAGER.lock().unwrap();
            eid_manager.borrow_mut().id = 0;
        }
        self.world.entities.clear();
        self.world.systems.clear();
        self.counter = 0.0;
        self.lights_changed = true;
        let mut ht2 = new_ht2_entity();
        ht2.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "position", ParameterValue::Vec3(Vec3::new(0.0, 0.0, 2.0)));
        let light_component = Light::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 1.0), 1.0);
        ht2.add_component(light_component);
        self.world.entities.push(ht2);
        self.entities_wanting_to_load_things.push(0);
    }

    #[allow(clippy::borrowed_box)]
    pub fn get_entity(&self, entity_id: EntityId) -> Option<Arc<Mutex<&Entity>>> {
        for entity in self.world.entities.iter() {
            if entity.get_id() == entity_id {
                return Some(Arc::new(Mutex::new(entity)));
            }
        }
        None
    }

    pub fn get_entity_index(&self, entity_id: EntityId) -> Option<usize> {
        for (index, entity) in self.world.entities.iter().enumerate() {
            if entity.get_id() == entity_id {
                return Some(index);
            }
        }
        None
    }

    pub fn remove_entity_at_index(&mut self, index: usize) {
        self.world.entities.remove(index);
    }

    pub fn send_lights_to_renderer(&self) -> Option<Vec<crate::light::Light>> {
        if !self.lights_changed {
            return Option::None;
        }
        let mut lights = Vec::new();
        for entity in &self.world.entities {
            let components = entity.get_components();
            let mut light_component = Option::None;
            let mut transform_component = Option::None; // if we have a transform component, this will be added to the light's position
            for component in components {
                if component.get_type() == COMPONENT_TYPE_LIGHT.clone() {
                    light_component = Option::Some(component);
                }
                if component.get_type() == COMPONENT_TYPE_TRANSFORM.clone() {
                    transform_component = Option::Some(component);
                }
            }
            if let Some(light) = light_component {
                let mut light = light.clone();
                let position = light.get_parameter("position").unwrap();
                let mut position = match position.value {
                    ParameterValue::Vec3(v) => v,
                    _ => {
                        error!("send_lights_to_renderer: light position is not a vec3");
                        Vec3::new(0.0, 0.0, 0.0)
                    }
                };
                let color = light.get_parameter("colour").unwrap();
                let color = match color.value {
                    ParameterValue::Vec3(v) => v,
                    _ => {
                        error!("send_lights_to_renderer: light color is not a vec3");
                        Vec3::new(0.0, 0.0, 0.0)
                    }
                };
                let intensity = light.get_parameter("intensity").unwrap();
                let intensity = match intensity.value {
                    ParameterValue::Float(v) => v,
                    _ => {
                        error!("send_lights_to_renderer: light intensity is not a float");
                        0.0
                    }
                };
                if let Some(transform) = transform_component {
                    let transform = transform.clone();
                    let trans_position = transform.get_parameter("position").unwrap();
                    let trans_position = match trans_position.value {
                        ParameterValue::Vec3(v) => v,
                        _ => {
                            error!("send_lights_to_renderer: transform position is not a vec3");
                            Vec3::new(0.0, 0.0, 0.0)
                        }
                    };
                    position = position + trans_position;
                }
                lights.push(crate::light::Light{
                    position,
                    color,
                    intensity: intensity as f32,
                });
            }
        }
        Some(lights)
    }

    pub fn render(&mut self, renderer: &mut ht_renderer) {
        self.counter += 1.0;
        let lights = self.send_lights_to_renderer();
        if lights.is_some() {
            renderer.set_lights(lights.unwrap());
        }
        for index in self.entities_wanting_to_load_things.clone() {
            let entity = &self.world.entities[index];
            let components = entity.get_components();
            for component in components {
                match component.get_type() {
                    x if x == COMPONENT_TYPE_MESH_RENDERER.clone() => {
                        let mesh = component.get_parameter("mesh").unwrap();
                        let mesh = match &mesh.value {
                            ParameterValue::String(v) => Some(v),
                            _ => {
                                error!("render: mesh is not a string");
                                None
                            }
                        };
                        let mesh = mesh.unwrap();
                        let texture = component.get_parameter("texture").unwrap();
                        let texture = match &texture.value {
                            ParameterValue::String(v) => Some(v),
                            _ => {
                                error!("render: texture is not a string");
                                None
                            }
                        };
                        let texture = texture.unwrap();
                        let res = renderer.load_mesh_if_not_already_loaded(mesh);
                        if res.is_err() {
                            warn!("render: failed to load mesh: {:?}", res);
                        }
                        let res = renderer.load_texture_if_not_already_loaded(texture);
                        if res.is_err() {
                            warn!("render: failed to load texture: {:?}", res);
                        }
                    }
                    x if x == COMPONENT_TYPE_TERRAIN.clone() => {
                        let name = component.get_parameter("name").unwrap();
                        let name = match &name.value {
                            ParameterValue::String(v) => Some(v),
                            _ => {
                                error!("render: terrain name is not a string");
                                None
                            }
                        };
                        let name = name.unwrap();
                        /*let res = renderer.load_terrain_if_not_already_loaded(name);
                        if res.is_err() {
                            warn!("render: failed to load terrain: {:?}", res);
                        }
                         */
                    }
                    _ => {}
                }
            }
        }
        self.entities_wanting_to_load_things.clear();
        for entity in self.world.entities.iter_mut() {
            if let Some(mesh_renderer) = entity.get_component(COMPONENT_TYPE_MESH_RENDERER.clone()) {
                if let Some(mesh) = mesh_renderer.get_parameter("mesh") {
                    // get the string value of the mesh
                    let mesh_name = match mesh.value {
                        ParameterValue::String(ref s) => s.clone(),
                        _ => {
                            error!("render: mesh is not a string");
                            continue;
                        }
                    };
                    // if so, render it
                    let shaders = renderer.shaders.clone();
                    let meshes = renderer.meshes.clone();
                    let mesh = meshes.get(&*mesh_name);
                    if let Some(mesh) = mesh {
                        let mut mesh = *mesh;
                        let shader = mesh_renderer.get_parameter("shader").unwrap();
                        let texture = mesh_renderer.get_parameter("texture").unwrap();
                        let shader_name = match shader.value {
                            ParameterValue::String(ref s) => s.clone(),
                            _ => {
                                error!("render: shader is not a string");
                                continue;
                            }
                        };
                        let texture_name = match texture.value {
                            ParameterValue::String(ref s) => s.clone(),
                            _ => {
                                error!("render: texture is not a string");
                                continue;
                            }
                        };
                        let shaders = renderer.shaders.clone();
                        let textures = renderer.textures.clone();
                        let shader = shaders.get(&*shader_name);
                        let texture = textures.get(&*texture_name);
                        if shader.is_none() || texture.is_none() {
                            error!("shader or texture not found: {:?} {:?}", shader_name, texture_name);
                            continue;
                        }
                        let shader = shader.unwrap();
                        let texture = texture.unwrap();

                        // if this entity has a transform, apply it
                        if let Some(transform) = entity.get_component(COMPONENT_TYPE_TRANSFORM.clone()) {
                            if let Some(position) = transform.get_parameter("position") {
                                let position = match position.value {
                                    ParameterValue::Vec3(v) => v,
                                    _ => {
                                        error!("render: transform position is not a vec3");
                                        continue;
                                    }
                                };
                                mesh.position += position;
                            }
                            if let Some(rotation) = transform.get_parameter("rotation") {
                                let rotation = match rotation.value {
                                    ParameterValue::Quaternion(v) => v,
                                    _ => {
                                        error!("render: transform rotation is not a quaternion");
                                        continue;
                                    }
                                };
                                // add a bit of rotation to the transform to make things more interesting
                                mesh.rotation = rotation;
                            }
                            if let Some(scale) = transform.get_parameter("scale") {
                                let scale = match scale.value {
                                    ParameterValue::Vec3(v) => v,
                                    _ => {
                                        error!("render: transform scale is not a vec3");
                                        continue;
                                    }
                                };
                                mesh.scale += scale;
                            }
                        }

                        // add a bit of rotation to the transform to make things more interesting
                        //entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "rotation", Box::new(Quaternion::from_euler_angles_zyx(&Vec3::new(0.0, self.counter, 0.0))));

                        mesh.render(renderer, Some(texture));
                    }
                }
            }
            /*if let Some(terrain) = entity.get_component(COMPONENT_TYPE_TERRAIN.clone()) {
                if let Some(name) = terrain.get_parameter("name") {
                    // get the string value of the mesh
                    let name = match name.value {
                        ParameterValue::String(ref s) => s.clone(),
                        _ => {
                            error!("render: terrain name is not a string");
                            continue;
                        }
                    };
                    // if so, render it
                    let terrains = renderer.terrains.clone().unwrap();
                    let terrain = terrains.get(&*name);
                    if let Some(terrain) = terrain {
                        let mut terrain = terrain.clone();
                        if let Some(transform) = entity.get_component(COMPONENT_TYPE_TRANSFORM.clone()) {
                            let position = transform.get_parameter("position").unwrap();
                            let position = match position.value {
                                ParameterValue::Vec3(v) => v,
                                _ => {
                                    error!("render: transform position is not a vec3");
                                    continue;
                                }
                            };
                            let rotation = transform.get_parameter("rotation").unwrap();
                            let rotation = match rotation.value {
                                ParameterValue::Quaternion(v) => v,
                                _ => {
                                    error!("render: transform rotation is not a quaternion");
                                    continue;
                                }
                            };
                            let scale = transform.get_parameter("scale").unwrap();
                            let scale = match scale.value {
                                ParameterValue::Vec3(v) => v,
                                _ => {
                                    error!("render: transform scale is not a vec3");
                                    continue;
                                }
                            };
                            terrain.mesh.position += position;
                            terrain.mesh.rotation = rotation;
                            terrain.mesh.scale += scale;
                        }
                        terrain.render(renderer);
                    }
                }
            }
             */
        }
    }
}