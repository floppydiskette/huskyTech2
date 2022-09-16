use std::any::Any;
use std::borrow::{Borrow, BorrowMut};
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use gfx_maths::{Quaternion, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::error::TryRecvError;
use crate::camera::Camera;
use crate::{ht_renderer, renderer, server};
use crate::physics::PhysicsSystem;
use crate::server::{ConnectionClientside, FastPacket, SteadyPacket, SteadyPacketData};
use crate::worldmachine::components::{COMPONENT_TYPE_LIGHT, COMPONENT_TYPE_MESH_RENDERER, COMPONENT_TYPE_TERRAIN, COMPONENT_TYPE_TRANSFORM, Light, MeshRenderer, Terrain, Transform};
use crate::worldmachine::ecs::*;
use crate::worldmachine::entities::new_ht2_entity;
use crate::worldmachine::MapLoadError::FolderNotFound;
use crate::worldmachine::player::PlayerContainer;

pub mod ecs;
pub mod components;
pub mod entities;
pub mod helpers;
pub mod player;

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

#[derive(Clone, Debug)]
pub enum WorldUpdate {
    InitEntity(EntityId, Entity),
    SetPosition(EntityId, Vec3),
    SetRotation(EntityId, Quaternion),
    SetScale(EntityId, Vec3),
}

#[derive(Clone, Debug)]
pub enum MapLoadError {
    FolderNotFound(String),
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
    pub entities_wanting_to_load_things: Vec<usize>,
    // index
    lights_changed: bool,
    is_server: bool,
    server_connection: Option<crate::server::ConnectionClientside>,
    world_update_queue: Arc<Mutex<VecDeque<WorldUpdate>>>,
    player: Option<PlayerContainer>,
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
            is_server: false,
            server_connection: None,
            world_update_queue: Arc::new(Mutex::new(VecDeque::new())),
            player: None
        }
    }
}

impl WorldMachine {
    pub fn initialise(&mut self, physics: PhysicsSystem, is_server: bool) {
        let _ = *components::COMPONENTS_INITIALISED;
        self.game_data_path = String::from("base");
        self.physics = Some(physics);
        self.is_server = is_server;

        self.blank_slate(is_server);
    }

    // resets the world to a blank slate
    pub fn blank_slate(&mut self, is_server: bool) {
        {
            let mut eid_manager = ENTITY_ID_MANAGER.lock().unwrap();
            eid_manager.borrow_mut().id = 0;
        }
        self.world.entities.clear();
        self.world.systems.clear();
        self.counter = 0.0;
        self.lights_changed = true;

        if !is_server {
            self.player = Some(PlayerContainer{
                player: Default::default(),
                entity_id: None
            });
        }
    }

    pub fn load_map(&mut self, map_name: &str) -> Result<(), MapLoadError> {
        self.blank_slate(self.is_server);
        let map_dir = format!("{}/maps/{}", self.game_data_path, map_name);
        if !std::path::Path::new(&map_dir).exists() {
            return Err(FolderNotFound(map_dir));
        }
        let mut deserializer = rmp_serde::Deserializer::new(std::fs::File::open(format!("{}/worlddef", map_dir)).unwrap());
        let world_def: WorldDef = Deserialize::deserialize(&mut deserializer).unwrap();

        // load entities
        for entity in world_def.world.entities {
            let mut entity_new = unsafe {
                Entity::new_with_id(&*entity.name, entity.uid)
            };
            for component in entity.components {
                let component_type = ComponentType::get(component.get_type().name).expect("component type not found");
                let mut component = component;
                component.component_type = component_type;
                entity_new.add_component(component);
            }
            self.world.entities.push(entity_new);
        }

        // if we're a server, queue entity init packets
        if self.is_server {
            let mut entity_init_packets = Vec::new();
            for entity in &self.world.entities {
                entity_init_packets.push(WorldUpdate::InitEntity(entity.uid, entity.clone()));
            }
            self.queue_updates(entity_init_packets);
        }

        // load systems
        for system in world_def.world.systems {
            self.world.systems.push(system);
        }

        Ok(())
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

    /*
    pub fn set_entity_position(&mut self, entity_id: EntityId, position: Vec3) {
        let entity_index = self.get_entity_index(entity_id).unwrap();
        let entity = self.world.entities[entity_index].borrow_mut();
        let res = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "position", ParameterValue::Vec3(position));
        if res.is_none() {
            warn!("attempted to set entity position on an entity that has no transform component");
        }
    }
     */

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
                lights.push(crate::light::Light {
                    position,
                    color,
                    intensity: intensity as f32,
                });
            }
        }
        Some(lights)
    }

    pub fn connect_to_server(&mut self, connection: ConnectionClientside) {
        self.server_connection = Some(connection);
    }

    async unsafe fn send_queued_steady_message(&mut self, message: SteadyPacketData) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let attempt = connection.steady_update_sender.send(message);
                    if attempt.is_err() {
                        error!("send_queued_steady_message: failed to send message");
                    }
                    loop {
                        let try_recv = connection.steady_update_receiver.try_recv();
                        if let Ok(message) = try_recv {
                            if let SteadyPacket::Consume(uuid) = message.packet.unwrap().clone() {
                                if uuid == message.uuid.unwrap() {
                                    debug!("consume message received");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn send_queued_steady_messages(&mut self) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut queue = connection.steady_sender_queue.lock().await;
                    while let Some(message) = queue.pop() {}
                }
            }
        }
    }

    fn consume_steady_message(&mut self, message: SteadyPacketData) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let attempt = connection.steady_update_sender.send(SteadyPacketData {
                        packet: Some(SteadyPacket::Consume(message.uuid.unwrap())),
                        uuid: Some(server::generate_uuid()),
                    });
                    if attempt.is_err() {
                        error!("send_queued_steady_message: failed to send message");
                    }
                }
            }
        }
    }

    fn process_steady_messages(&mut self) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    // check if we have any messages to process
                    let try_recv = connection.steady_update_receiver.try_recv();
                    if let Ok(message) = try_recv {
                        match message.clone().packet.unwrap().clone() {
                            SteadyPacket::Consume(_) => {}
                            SteadyPacket::KeepAlive => {}
                            SteadyPacket::InitialiseEntity(entity_id, entity_data) => {
                                // check if we already have this entity
                                if self.get_entity(entity_id).is_none() {
                                    let mut entity = unsafe {
                                        Entity::new_with_id(entity_data.name.as_str(), entity_id)
                                    };
                                    entity.copy_data_from_other_entity(&entity_data);
                                    self.world.entities.push(entity);
                                    self.entities_wanting_to_load_things.push(self.world.entities.len() - 1);
                                } else {
                                    // we already have this entity, so we need to update it
                                    let entity_index = self.get_entity_index(entity_id).unwrap();
                                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                                    entity.copy_data_from_other_entity(&entity_data);
                                    self.entities_wanting_to_load_things.push(entity_index);
                                }
                                debug!("initialise entity message received");
                                debug!("world entities: {:?}", self.world.entities);
                            }
                            SteadyPacket::Message(str_message) => {
                                info!("Received message from server: {}", str_message);
                            }
                            SteadyPacket::SelfTest => {
                                self.counter += 1.0;
                                info!("received {} self test messages", self.counter);
                            }
                        }
                        self.consume_steady_message(message);
                    } else if let Err(e) = try_recv {
                        if e != TryRecvError::Empty {
                            warn!("process_steady_messages: error receiving message: {:?}", e);
                        }
                    }
                }
            }
        }
    }

    fn process_fast_messages(&mut self) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    // check if we have any messages to process
                    let try_recv = connection.fast_update_receiver.try_recv();
                    if let Ok(message) = try_recv {
                        match message.clone().packet.unwrap().clone() {
                            FastPacket::ChangePosition(entity_id, vec3) => {
                                if let Some(entity_index) = self.get_entity_index(entity_id) {
                                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                                    let transform = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "position", ParameterValue::Vec3(vec3));
                                    if transform.is_none() {
                                        warn!("process_fast_messages: failed to set transform position");
                                    }
                                }
                            }
                            FastPacket::ChangeRotation(entity_id, quat) => {
                                if let Some(entity_index) = self.get_entity_index(entity_id) {
                                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                                    let transform = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "rotation", ParameterValue::Quaternion(quat));
                                    if transform.is_none() {
                                        warn!("process_fast_messages: failed to set transform rotation");
                                    }
                                }
                            }
                            FastPacket::ChangeScale(entity_id, vec3) => {
                                if let Some(entity_index) = self.get_entity_index(entity_id) {
                                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                                    let transform = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "scale", ParameterValue::Vec3(vec3));
                                    if transform.is_none() {
                                        warn!("process_fast_messages: failed to set transform scale");
                                    }
                                }
                            }
                        }
                    } else if let Err(e) = try_recv {
                        if e != TryRecvError::Empty {
                            warn!("process_steady_messages: error receiving message: {:?}", e);
                        }
                    }
                }
            }
        }
    }

    pub async fn tick_connection(&mut self) {
        self.process_steady_messages();
        self.send_queued_steady_messages().await;
        self.process_fast_messages();
    }

    pub async fn server_tick(&mut self) -> Option<Vec<WorldUpdate>> {
        let mut updates = Vec::new();

        let mut world_updates = self.world_update_queue.lock().await;
        world_updates.drain(..).for_each(|update| {
            updates.push(update);
        });

        if !updates.is_empty() {
            Some(updates)
        } else {
            None
        }
    }

    pub async fn queue_update(&mut self, update: WorldUpdate) {
        if !self.is_server {
            warn!("queue_update: called on client");
        } else {
            let mut world_updates = self.world_update_queue.lock().await;
            world_updates.push_back(update);
        }
    }

    pub fn queue_updates(&mut self, updates: Vec<WorldUpdate>) {
        if !self.is_server {
            warn!("queue_update: called on client");
        } else {
            let world_updates = self.world_update_queue.clone();
            tokio::spawn(async move {
                let mut world_updates = world_updates.lock().await;
                updates.iter().for_each(|update| {
                    world_updates.push_back(update.clone());
                });
            });
        }
    }

    pub fn client_tick(&mut self, renderer: &mut ht_renderer, delta_time: f32) {
        if let Some(player_container) = self.player.as_mut() {
            let player = &mut player_container.player;
            player.handle_input(renderer, delta_time);
        }
    }

    pub fn render(&mut self, renderer: &mut ht_renderer) {
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
        for (i, entity) in self.world.entities.iter_mut().enumerate() {
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
                    } else {
                        // if not, add it to the list of things to load
                        self.entities_wanting_to_load_things.push(i);
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