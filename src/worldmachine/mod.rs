use std::any::Any;
use std::borrow::{Borrow, BorrowMut};
use halfbrown::HashMap;
use std::collections::{VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Instant;
use fyrox_sound::context::SoundContext;
use gfx_maths::{Quaternion, Vec2, Vec3};
use gl_matrix::common::Quat;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::error::TryRecvError;
use crate::camera::Camera;
use crate::{ht_renderer, renderer, server};
use crate::animgraph::{AnimGraph, AnimGraphNode};
use crate::audio::AudioBackend;
use crate::common_anim::move_anim::{Features, MoveAnim};
use crate::helpers::{add_quaternion, from_q64, multiply_quaternion, rotate_vector_by_quaternion, to_q64};
use crate::physics::{Materials, PhysicsSystem};
use crate::server::{ConnectionClientside, ConnectionUUID, FastPacket, FastPacketData, NameRejectionReason, SteadyPacket, SteadyPacketData};
use crate::server::server_player::{ServerPlayer, ServerPlayerContainer};
use crate::ui_defs::chat;
use crate::worldmachine::components::{COMPONENT_TYPE_BOX_COLLIDER, COMPONENT_TYPE_JUKEBOX, COMPONENT_TYPE_LIGHT, COMPONENT_TYPE_MESH_RENDERER, COMPONENT_TYPE_PLAYER, COMPONENT_TYPE_TERRAIN, COMPONENT_TYPE_TRANSFORM, COMPONENT_TYPE_TRIGGER, Light, MeshRenderer, Terrain, Transform};
use crate::worldmachine::ecs::*;
use crate::worldmachine::MapLoadError::FolderNotFound;
use crate::worldmachine::player::{MovementInfo, Player, PlayerContainer};

pub mod ecs;
pub mod components;
pub mod entities;
pub mod helpers;
pub mod player;
pub mod playermodel;

pub type EntityId = u64;

#[derive(Deserialize, Serialize)]
pub struct World {
    pub entities: Vec<Entity>,
    pub systems: Vec<System>,
    eid_manager: EntityId,
    current_map: String,
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
    MovePlayerEntity(EntityId, Vec3, Quaternion, Quaternion),
    EntityNoLongerExists(EntityId),
}

#[derive(Clone, Debug)]
pub enum ClientUpdate {
    // internal
    IDisplaced((Vec3, Option<MovementInfo>)),
    ILooked(Quaternion),
    // external
    IMoved(Vec3, Option<Vec3>, Quaternion, Quaternion, Option<MovementInfo>), // position, displacement vector, rotation, head rotation, extra movement info
    IJumped,
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
            current_map: self.current_map.clone(),
        }
    }
}

pub struct WorldMachine {
    pub world: World,
    pub physics: Option<PhysicsSystem>,
    pub last_physics_update: std::time::Instant,
    pub game_data_path: String,
    pub counter: f32,
    pub entities_wanting_to_load_things: Vec<usize>,
    // index
    lights_changed: bool,
    is_server: bool,
    server_connection: Option<crate::server::ConnectionClientside>,
    world_update_queue: Arc<Mutex<VecDeque<WorldUpdate>>>,
    client_update_queue: Arc<Mutex<VecDeque<ClientUpdate>>>,
    player: Option<PlayerContainer>,
    ignore_this_entity: Option<EntityId>, // should be the player entity that other players will see, we don't want it's updates to be received because we already know them
    pub players: Option<Arc<Mutex<HashMap<ConnectionUUID, ServerPlayerContainer>>>>, // not used clientside
}

impl Default for WorldMachine {
    fn default() -> Self {
        let world = World {
            entities: Vec::new(),
            systems: Vec::new(),
            eid_manager: 0,
            current_map: "".to_string(),
        };
        Self {
            world,
            physics: None,
            last_physics_update: std::time::Instant::now(),
            game_data_path: String::from(""),
            counter: 0.0,
            entities_wanting_to_load_things: Vec::new(),
            lights_changed: true,
            is_server: false,
            server_connection: None,
            world_update_queue: Arc::new(Mutex::new(VecDeque::new())),
            client_update_queue: Arc::new(Mutex::new(VecDeque::new())),
            player: None,
            ignore_this_entity: None,
            players: None,
        }
    }
}

impl WorldMachine {
    pub fn initialise(&mut self, physics: PhysicsSystem, is_server: bool) {
        let _ = *components::COMPONENTS_INITIALISED;
        self.game_data_path = String::from("base");
        self.physics = Some(physics);
        self.is_server = is_server;

        if self.is_server {
            let physics = self.physics.as_mut().unwrap().copy_with_new_scene();
            self.physics = Some(physics);
        }

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
                Entity::new(entity.name.as_str())
            };
            for component in entity.components {
                let component_type = ComponentType::get(component.get_type().name);
                if component_type.is_none() {
                    panic!("component type not found: {}", component.get_type().name);
                }
                let component_type = component_type.unwrap();
                let mut component = component;
                component.component_type = component_type.clone();

                entity_new.add_component(component);
            }
            self.world.entities.push(entity_new);
        }

        self.world.current_map = map_name.to_string();

        // initialise entities
        self.initialise_entities();

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

    /// this should only be called once per map load
    pub fn initialise_entities(&mut self) {
        for entity in &mut self.world.entities {
            if let Some(box_collider) = entity.get_component(COMPONENT_TYPE_BOX_COLLIDER.clone()) {
                let box_collider = box_collider.borrow();
                let position = box_collider.get_parameter("position").unwrap().borrow().clone();
                let mut position = match position.value {
                    ParameterValue::Vec3(position) => position,
                    _ => panic!("position is not a vec3"),
                };
                let scale = box_collider.get_parameter("scale").unwrap().borrow().clone();
                let mut scale = match scale.value {
                    ParameterValue::Vec3(scale) => scale,
                    _ => panic!("scale is not a vec3"),
                };
                if let Some(transform) = entity.get_component(COMPONENT_TYPE_TRANSFORM.clone()) {
                    let transform = transform.borrow();
                    let trans_position = transform.get_parameter("position").unwrap().borrow().clone();
                    let trans_position = match trans_position.value {
                        ParameterValue::Vec3(position) => position,
                        _ => panic!("position is not a vec3"),
                    };
                    let trans_scale = transform.get_parameter("scale").unwrap().borrow().clone();
                    let trans_scale = match trans_scale.value {
                        ParameterValue::Vec3(scale) => scale,
                        _ => panic!("scale is not a vec3"),
                    };
                    position += trans_position;
                    scale *= trans_scale;
                }
                let box_collider_physics = self.physics.as_ref().unwrap().create_box_collider_static(position, scale, Materials::Player).unwrap();
                box_collider_physics.add_self_to_scene(self.physics.clone().unwrap());
            }
            if let Some(trigger) = entity.get_component(COMPONENT_TYPE_TRIGGER.clone()) {
                let trigger = trigger.borrow();
                let position = trigger.get_parameter("position").unwrap().borrow().clone();
                let mut position = match position.value {
                    ParameterValue::Vec3(position) => position,
                    _ => panic!("position is not a vec3"),
                };
                let scale = trigger.get_parameter("size").unwrap().borrow().clone();
                let mut scale = match scale.value {
                    ParameterValue::Vec3(scale) => scale,
                    _ => panic!("scale is not a vec3"),
                };
                if let Some(transform) = entity.get_component(COMPONENT_TYPE_TRANSFORM.clone()) {
                    let transform = transform.borrow();
                    let trans_position = transform.get_parameter("position").unwrap().borrow().clone();
                    let trans_position = match trans_position.value {
                        ParameterValue::Vec3(position) => position,
                        _ => panic!("position is not a vec3"),
                    };
                    let trans_scale = transform.get_parameter("scale").unwrap().borrow().clone();
                    let trans_scale = match trans_scale.value {
                        ParameterValue::Vec3(scale) => scale,
                        _ => panic!("scale is not a vec3"),
                    };
                    position += trans_position;
                    scale *= trans_scale;
                }
                let trigger_physics = self.physics.as_ref().unwrap().create_trigger_shape(position, scale, Materials::Player).unwrap();
                trigger_physics.add_self_to_scene(self.physics.clone().unwrap());
                debug!("added trigger to physics scene with position: {:?} and scale: {:?}", position, scale);
            }
        }
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

    pub fn send_lights_to_renderer(&mut self) -> Option<Vec<crate::light::Light>> {
        //if !self.lights_changed {
        //    return Option::None;
        //}
        let mut lights = Vec::new();
        for entity in &self.world.entities {
            let components = entity.get_components();
            let mut light_component = None;
            let mut transform_component = None; // if we have a transform component, this will be added to the light's position
            for component in components {
                if component.get_type() == COMPONENT_TYPE_LIGHT.clone() {
                    light_component = Some(component);
                }
                if component.get_type() == COMPONENT_TYPE_TRANSFORM.clone() {
                    transform_component = Some(component);
                }
            }
            if let Some(light) = light_component {
                let light = light.clone();
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
                    position += trans_position;
                }
                lights.push(crate::light::Light {
                    position,
                    color,
                    intensity: intensity as f32,
                });
            }
        }
        self.lights_changed = false;
        Some(lights)
    }

    pub fn connect_to_server(&mut self, connection: ConnectionClientside) {
        self.server_connection = Some(connection);
    }

    async unsafe fn send_queued_steady_message(&mut self, message: SteadyPacketData) {
        let mut tries = 0;
        const MAX_TRIES: i32 = 10;

        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut connection = connection.lock().await;
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
                ConnectionClientside::Lan(connection) => {
                    let attempt = connection.send_steady_and_serialise(message).await;
                    if attempt.is_err() {
                        error!("send_queued_steady_message: failed to send message");
                    }
                    loop {
                        let try_recv = connection.attempt_receive_steady_and_deserialise().await;
                        if let Some(message) = try_recv {
                            if let SteadyPacket::Consume(uuid) = message.clone().packet.unwrap() {
                                if uuid == message.clone().uuid.unwrap() {
                                    debug!("consume message received");
                                    break;
                                } else {
                                    // requeue the message
                                    connection.steady_receiver_queue.lock().await.push(message);
                                }
                            } else {
                                // requeue the message
                                connection.steady_receiver_queue.lock().await.push(message);
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
                    let mut connection = connection.lock().await;
                    let mut queue = connection.steady_sender_queue.lock().await;
                    while let Some(message) = queue.pop().await {}
                }
                ConnectionClientside::Lan(connection) => {
                    let mut queue = connection.steady_sender_queue.lock().await;
                    while let Some(message) = queue.pop().await {
                        debug!("sending queued steady message");
                        debug!("message: {:?}", message);
                        let attempt = connection.send_steady_and_serialise(message).await;
                        if attempt.is_err() {
                            error!("send_queued_steady_messages: failed to send message");
                        }
                    }
                }
            }
        }
    }

    async fn send_fast_message(&mut self, message: FastPacketData) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut connection = connection.lock().await;
                    let attempt = connection.fast_update_sender.send(message);
                    if attempt.is_err() {
                        error!("send_fast_message: failed to send message");
                    }
                }
                ConnectionClientside::Lan(connection) => {
                    let attempt = connection.send_fast_and_serialise(message).await;
                }
            }
        }
    }

    async fn send_steady_message(&mut self, message: SteadyPacketData) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut connection = connection.lock().await;
                    let attempt = connection.steady_update_sender.send(message);
                    if attempt.is_err() {
                        error!("send_steady_message: failed to send message");
                    }
                }
                ConnectionClientside::Lan(connection) => {
                    let attempt = connection.send_steady_and_serialise(message).await;
                    if attempt.is_err() {
                        error!("send_steady_message: failed to send message");
                    }
                }
            }
        }
    }

    async fn consume_steady_message(&mut self, message: SteadyPacketData) {
        if let Some(connection) = &mut self.server_connection {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut connection = connection.lock().await;
                    let attempt = connection.steady_update_sender.send(SteadyPacketData {
                        packet: Some(SteadyPacket::Consume(message.uuid.unwrap())),
                        uuid: Some(server::generate_uuid()),
                    });
                    if attempt.is_err() {
                        error!("send_queued_steady_message: failed to send message");
                    }
                    debug!("consume message sent");
                }
                ConnectionClientside::Lan(connection) => {
                    let attempt = connection
                        .send_steady_and_serialise(SteadyPacketData {
                            packet: Some(SteadyPacket::Consume(message.uuid.unwrap())),
                            uuid: Some(server::generate_uuid()),
                        })
                        .await;
                    if attempt.is_err() {
                        error!("send_queued_steady_message: failed to send message");
                    }
                    debug!("consume message sent");
                }
            }
        } else {
            error!("consume_steady_message: no connection");
        }
    }

    async fn initialise_entity(&mut self, packet: SteadyPacket) {
        if let SteadyPacket::InitialiseEntity(entity_id, entity_data) = packet {
        }
    }

    async fn initialise_player(&mut self, packet: SteadyPacket) {
        if let SteadyPacket::InitialisePlayer(uuid, id,  name, position, rotation, scale) = packet {
        }
    }

    async fn remove_entity(&mut self, packet: SteadyPacket) {
        if let SteadyPacket::RemoveEntity(entity_id) = packet {
        }
    }

    pub async fn set_name(&mut self, name: String) {
        self.send_steady_message(SteadyPacketData {
            packet: Some(SteadyPacket::SetName(String::new(), name)),
            uuid: Some(server::generate_uuid()),
        }).await;
    }

    pub async fn send_chat_message(&mut self, message: String) {
        self.send_steady_message(SteadyPacketData {
            packet: Some(SteadyPacket::ChatMessage(String::new(), message)),
            uuid: Some(server::generate_uuid()),
        }).await;
    }

    async fn handle_steady_message(&mut self, packet: SteadyPacket) {
        match packet {
            SteadyPacket::Consume(_) => {}
            SteadyPacket::KeepAlive => {}
            SteadyPacket::InitialiseEntity(entity_id, entity_data) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }

                if let Some(_player) = entity_data.get_component(COMPONENT_TYPE_PLAYER.clone()) {
                    chat::write_chat("engine".to_string(), "a new player has joined!".to_string());
                }

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
            }
            SteadyPacket::Message(str_message) => {
                info!("Received message from server: {}", str_message);
            }
            SteadyPacket::SelfTest => {
                self.counter += 1.0;
                info!("received {} self test messages", self.counter);
            }
            SteadyPacket::InitialisePlayer(uuid, id, name, position, rotation, scale) => {
                let mut player = Player::default();
                player.init(self.physics.clone().unwrap(), uuid, name.clone(), position, rotation, scale);
                chat::CHAT_BUFFER.lock().unwrap().my_name = name;
                self.ignore_this_entity = Some(id);
                self.player = Some(PlayerContainer {
                    player,
                    entity_id: None
                });
            }
            SteadyPacket::FinaliseMapLoad => {
                self.initialise_entities();
            }
            SteadyPacket::RemoveEntity(entity_id) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }
                let entity_index = self.get_entity_index(entity_id);
                if let Some(entity_index) = entity_index {
                    self.world.entities.remove(entity_index);
                    debug!("remove entity message received");
                    debug!("world entities: {:?}", self.world.entities);
                }
            }
            SteadyPacket::ChatMessage(who_sent, message) => {
                let mut dont_show = false;
                if let Some(player) = &self.player {
                    if player.player.uuid == who_sent {
                        dont_show = true;
                    }
                }
                if !dont_show {
                    let players = self.world.entities.iter().filter(|e| e.has_component(COMPONENT_TYPE_PLAYER.clone())).collect::<Vec<&Entity>>();
                    let name = {
                        let mut namebuf = None;
                        for player in players {
                            if let Some(player_component) = player.get_component(COMPONENT_TYPE_PLAYER.clone()) {
                                let uuid = player_component.get_parameter("uuid").unwrap();
                                let uuid = match &uuid.value {
                                    ParameterValue::String(uuid) => uuid,
                                    _ => panic!("uuid is not a string")
                                };
                                let name = player_component.get_parameter("name").unwrap();
                                let name = match &name.value {
                                    ParameterValue::String(name) => name,
                                    _ => panic!("name is not a string")
                                };

                                if uuid == &who_sent {
                                    namebuf = Some(name.clone());
                                }
                            }
                        }
                        namebuf
                    };
                    if let Some(name) = name {
                        chat::write_chat(name, message);
                    } else {
                        chat::write_chat(who_sent, message);
                    }
                }
            }
            SteadyPacket::SetName(who_sent, new_name) => {
                let players = self.world.entities.iter().filter(|e| e.has_component(COMPONENT_TYPE_PLAYER.clone())).collect::<Vec<&Entity>>();
                let name = {
                    let mut namebuf = None;
                    for player in players {
                        if let Some(player_component) = player.get_component(COMPONENT_TYPE_PLAYER.clone()) {
                            let uuid = player_component.get_parameter("uuid").unwrap();
                            let uuid = match &uuid.value {
                                ParameterValue::String(uuid) => uuid,
                                _ => panic!("uuid is not a string")
                            };
                            let name = player_component.get_parameter("name").unwrap();
                            let name = match &name.value {
                                ParameterValue::String(name) => name,
                                _ => panic!("name is not a string")
                            };

                            if uuid == &who_sent {
                                namebuf = Some(name.clone());
                            }
                        }
                    }
                    namebuf
                };
                if let Some(name) = name {
                    chat::write_chat("server".to_string(), format!("{} is now known as {}", name, new_name));
                } else {
                    chat::write_chat("server".to_string(), format!("{} is now known as {}", who_sent, new_name));
                }
                if let Some(players) = &self.players {
                    let mut players = players.lock().await;
                    if let Some(player) = players.get_mut(&who_sent) {
                        player.player.name = new_name;
                    }
                }
            }
            SteadyPacket::NameRejected(reason) => {
                match reason {
                    NameRejectionReason::IllegalWord => {
                        chat::write_chat("server".to_string(), "whoa there! we don't use that kind of language here!".to_string());
                    }
                    NameRejectionReason::Taken => {
                        chat::write_chat("server".to_string(), "your name was rejected because it is already taken".to_string());
                    }
                }
            }
        }
    }

    async fn process_steady_messages(&mut self) {
        if let Some(connection) = self.server_connection.clone() {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut connection = connection.lock().await;
                    // check if we have any messages to process
                    let try_recv = connection.steady_update_receiver.try_recv();
                    if let Ok(message) = try_recv {
                        drop(connection);
                        self.handle_steady_message(message.clone().packet.unwrap()).await;
                        self.consume_steady_message(message).await;
                    } else if let Err(e) = try_recv {
                        if e != TryRecvError::Empty {
                            warn!("process_steady_messages: error receiving message: {:?}", e);
                        }
                    }
                }
                ConnectionClientside::Lan(connection) => {
                    // check if we have any messages to process
                    let try_recv = connection.attempt_receive_steady_and_deserialise().await;
                    if let Some(message) = try_recv {
                        self.handle_steady_message(message.clone().packet.unwrap()).await;
                        // don't consume consume packets
                        if let SteadyPacket::Consume(_) = message.clone().packet.unwrap() {
                        } else {
                            self.consume_steady_message(message).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_message_fast(&mut self, packet: FastPacket) {
        match packet.clone() {
            FastPacket::ChangePosition(entity_id, vec3) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }
                if let Some(entity_index) = self.get_entity_index(entity_id) {
                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                    let transform = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "position", ParameterValue::Vec3(vec3));
                    if transform.is_none() {
                        warn!("process_fast_messages: failed to set transform rotation");
                    }
                }
            }
            FastPacket::ChangeRotation(entity_id, quat) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }
                if let Some(entity_index) = self.get_entity_index(entity_id) {
                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                    let transform = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "rotation", ParameterValue::Quaternion(quat));
                    if transform.is_none() {
                        warn!("process_fast_messages: failed to set transform rotation");
                    }
                }
            }
            FastPacket::ChangeScale(entity_id, vec3) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }
                if let Some(entity_index) = self.get_entity_index(entity_id) {
                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                    let transform = entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "scale", ParameterValue::Vec3(vec3));
                    if transform.is_none() {
                        warn!("process_fast_messages: failed to set transform scale");
                    }
                }
            }
            FastPacket::PlayerMoved(entity_id, new_position, new_rotation, new_head_rotation) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }
                if let Some(entity_index) = self.get_entity_index(entity_id) {
                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                    let prev_transform = entity.get_component(COMPONENT_TYPE_PLAYER.clone());
                    if let Some(prev_transform) = prev_transform {
                        let prev_position = prev_transform.get_parameter("position").unwrap();

                        // calculate the difference between the previous and new position
                        let prev_position = match prev_position.value {
                            ParameterValue::Vec3(vec3) => vec3,
                            _ => {
                                warn!("process_fast_messages: failed to get previous position");
                                return;
                            }
                        };

                        let position_diff = new_position - prev_position;
                        let forward_mag = position_diff.dot(new_rotation.forward());
                        let strafe_mag = position_diff.dot(new_rotation.right());
                        const threshold: f32 = 0.01;
                        let forward_mag = if forward_mag.abs() < threshold { 0.0 } else { 1.0 * forward_mag.signum() };
                        let strafe_mag = if strafe_mag.abs() < threshold { 0.0 } else { 1.0 * strafe_mag.signum() };

                        // set speed and strafe for animation
                        let player_component = entity.set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "speed", ParameterValue::Float(forward_mag as f64));
                        if player_component.is_none() {
                            warn!("process_fast_messages: failed to set transform position");
                        }
                        let player_component = entity.set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "strafe", ParameterValue::Float(strafe_mag as f64));
                        if player_component.is_none() {
                            warn!("process_fast_messages: failed to set transform position");
                        }
                    }

                    let player_component = entity.set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "position", ParameterValue::Vec3(new_position));
                    if player_component.is_none() {
                        warn!("process_fast_messages: failed to set transform position");
                    }
                    let player_component = entity.set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "rotation", ParameterValue::Quaternion(new_rotation));
                    if player_component.is_none() {
                        warn!("process_fast_messages: failed to set transform rotation");
                    }
                    let player_component = entity.set_component_parameter(COMPONENT_TYPE_PLAYER.clone(), "head_rotation", ParameterValue::Quaternion(new_head_rotation));
                    if player_component.is_none() {
                        warn!("process_fast_messages: failed to set transform rotation");
                    }

                }
            }
            FastPacket::EntitySetParameter(entity_id, component_type, parameter_name, parameter_value) => {
                if let Some(ignore) = self.ignore_this_entity {
                    if entity_id == ignore {
                        return;
                    }
                }
                if let Some(entity_index) = self.get_entity_index(entity_id) {
                    let entity = self.world.entities.get_mut(entity_index).unwrap();
                    let component = entity.set_component_parameter(component_type, parameter_name.as_str(), parameter_value);
                    if component.is_none() {
                        warn!("process_fast_messages: failed to set component parameter");
                    }
                }
            }
            FastPacket::PlayerFuckYouMoveHere(new_position) => {
                if let Some(player) = self.player.as_mut() {
                    warn!("we moved too fast, so the server is telling us to move to a new position");
                    player.player.set_position(new_position);
                }
            }
            FastPacket::PlayerFuckYouSetRotation(new_rotation) => {
                if let Some(player) = self.player.as_mut() {
                    warn!("we did something wrong, so the server is telling us to set our rotation");
                    player.player.set_rotation(new_rotation);
                    player.player.set_head_rotation(new_rotation);
                }
            }
            FastPacket::PlayerCheckPosition(_, _) => {}
            FastPacket::PlayerMove(_, _, _, _, _, _) => {}
            FastPacket::PlayerJump(_) => {}
        }
    }

    async fn process_fast_messages(&mut self) {
        if let Some(connection) = self.server_connection.clone() {
            match connection {
                ConnectionClientside::Local(connection) => {
                    let mut connection = connection.lock().await;
                    // check if we have any messages to process
                    let try_recv = connection.fast_update_receiver.try_recv();
                    if let Ok(message) = try_recv {
                        self.handle_message_fast(message.clone().packet.unwrap()).await;
                    } else if let Err(e) = try_recv {
                        if e != TryRecvError::Empty {
                            warn!("process_steady_messages: error receiving message: {:?}", e);
                        }
                    }
                }
                ConnectionClientside::Lan(connection) => {
                    // check if we have any messages to process
                    let try_recv = connection.attempt_receive_fast_and_deserialise().await;
                    if let Some(message) = try_recv {
                        self.handle_message_fast(message.clone().packet.unwrap()).await;
                    }
                }
            }
        }
    }

    async fn process_client_updates(&mut self, client_updates: &mut Vec<ClientUpdate>) {
        let mut updates = Vec::new();
        let mut movement_updates = Vec::new();
        let mut jumped_real = false;
        let mut movement_info = None;
        for client_update in client_updates {
            match client_update {
                ClientUpdate::IDisplaced(displacement_vector) => {
                    let position = self.player.as_mut().unwrap().player.get_position();
                    let rotation = self.player.as_mut().unwrap().player.get_rotation();
                    let head_rotation = self.player.as_mut().unwrap().player.get_head_rotation();
                    if movement_info.is_none() {
                        if let Some(movement_info_some) = displacement_vector.1 {
                            movement_info = Some(movement_info_some);
                        }
                    }
                    movement_updates.push(ClientUpdate::IMoved(position, Some(displacement_vector.0), rotation, head_rotation, movement_info));
                }
                ClientUpdate::ILooked(look_quat) => {
                    let position = self.player.as_mut().unwrap().player.get_position();
                    let rotation = self.player.as_mut().unwrap().player.get_rotation();
                    let head_rotation = self.player.as_mut().unwrap().player.get_head_rotation();
                    movement_updates.push(ClientUpdate::IMoved(position, None, rotation, head_rotation, movement_info));}
                ClientUpdate::IJumped => {
                    jumped_real = true;
                }
                _ => {
                    updates.push(client_update.clone());
                }
            }
        }
        // get the latest movement update and append it to the end of the updates
        let mut last_displacement_vector = None;
        if movement_updates.len() > 0 {
            for update in movement_updates.clone() {
                if let ClientUpdate::IMoved(_, displacement_vector, _, _, _) = update {
                    last_displacement_vector = displacement_vector;
                }
            }
            let mut latest_movement_update = movement_updates.last().unwrap().clone();
            // if we have a displacement vector, we need to add it to the last movement update
            if let Some(displacement_vector) = last_displacement_vector {
                if let ClientUpdate::IMoved(position, _, rotation, head_rotation, jumped) = latest_movement_update {
                    let new = ClientUpdate::IMoved(position, Some(displacement_vector), rotation, head_rotation, movement_info);
                    latest_movement_update = new;
                }
            }
            updates.push(latest_movement_update.clone());
        }
        // send the updates to the server
        for update in updates {
            match update {
                ClientUpdate::IDisplaced(_) => {}
                ClientUpdate::ILooked(_) => {}
                ClientUpdate::IMoved(position, displacement_vector, rotation, head_rotation, jumped) => {
                    let uuid = self.player.as_ref().unwrap().player.uuid.clone();
                    let displacement_vector = displacement_vector.unwrap_or(Vec3::new(0.0, 0.0, 0.0));
                    let packet = FastPacket::PlayerMove(uuid, position, displacement_vector, rotation, head_rotation, movement_info);
                    self.send_fast_message(FastPacketData {
                        packet: Some(packet),
                    }).await;
                }
                ClientUpdate::IJumped => {
                    let uuid = self.player.as_ref().unwrap().player.uuid.clone();
                    let packet = FastPacket::PlayerJump(uuid);
                    self.send_fast_message(FastPacketData {
                        packet: Some(packet),
                    }).await;
                }
            }
        }
    }

    pub async fn tick_connection(&mut self, client_updates: &mut Vec<ClientUpdate>) {
        self.process_steady_messages().await;
        self.send_queued_steady_messages().await;
        self.process_fast_messages().await;
        self.process_client_updates(client_updates).await;
    }

    pub async fn server_tick(&mut self) -> Option<Vec<WorldUpdate>> {
        let mut updates = Vec::new();

        let mut world_updates = self.world_update_queue.lock().await;
        world_updates.drain(..).for_each(|update| {
            updates.push(update);
        });
        drop(world_updates);

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

    pub async fn client_tick(&mut self, renderer: &mut ht_renderer, physics_engine: PhysicsSystem, delta_time: f32) -> Vec<ClientUpdate> {
        if self.is_server {
            warn!("client_tick: called on server");
            return vec![];
        }

        let mut updates = Vec::new();

        if let Some(player_container) = self.player.as_mut() {
            let player = &mut player_container.player;
            let player_updates = player.handle_input(renderer, delta_time);
            if let Some(mut player_updates) = player_updates {
                updates.append(&mut player_updates);
            }
        }

        updates
    }

    pub fn next_frame(&mut self, renderer: &mut ht_renderer) {
        for mesh in &mut renderer.meshes.values_mut() {
            mesh.updated_animations_this_frame = false;
            if let Some(shadow_mesh) = &mesh.shadow_mesh {
                shadow_mesh.lock().unwrap().updated_animations_this_frame = false;
            }
        }
    }

    pub fn render(&mut self, renderer: &mut ht_renderer, shadow_pass: Option<(u8, usize)>) {
        // todo! actual good player rendering
        if let Some(player) = &mut self.player {
            let position = player.player.get_position();
            let rotation = player.player.get_rotation();
            if let Some(mut mesh) = renderer.meshes.get("player").cloned() {
                renderer.meshes.get_mut("player").unwrap().updated_animations_this_frame = false;
                if let Some(shadow_mesh) = &renderer.meshes.get_mut("player").unwrap().shadow_mesh {
                    shadow_mesh.lock().unwrap().updated_animations_this_frame = false;
                }
                let texture = renderer.textures.get("default").cloned().unwrap();
                mesh.position = position + (rotation.forward() * -0.2) + Vec3::new(0.0, -0.2, 0.0);
                mesh.rotation = rotation;
                mesh.scale = Vec3::new(0.6, 0.6, 0.6);

                let move_anim = MoveAnim::from_values(player.player.speed, player.player.strafe);

                mesh.render(renderer, Some(&texture), Some(move_anim.weights()), shadow_pass);
            }
        }

        let lights = self.send_lights_to_renderer();
        if let Some(..) = lights {
            renderer.set_lights(lights.unwrap());
        }
        let mut indices_to_remove = Vec::new();
        for index in self.entities_wanting_to_load_things.clone() {
            let entity = &self.world.entities[index];
            let components = entity.get_components();
            let mut finished_loading = components.len();
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
                            warn!("render: failed to load mesh '{}': {:?}", mesh, res);
                        }
                        let mesh_loaded = res.unwrap();
                        let res = renderer.load_texture_if_not_already_loaded(texture);
                        if res.is_err() {
                            warn!("render: failed to load texture '{}': {:?}", texture, res);
                        }
                        let texture_loaded = res.unwrap();
                        if mesh_loaded && texture_loaded {
                            finished_loading -= 1;
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
                        let terrain_loaded = true;
                        if terrain_loaded {
                            finished_loading -= 1;
                        }
                    }
                    x if x == COMPONENT_TYPE_LIGHT.clone() => {
                        self.lights_changed = true;
                        finished_loading -= 1;
                    }
                    _ => {
                        finished_loading -= 1;
                    }
                }
            }
            if finished_loading == 0 {
                indices_to_remove.push(index);
            }
        }
        self.entities_wanting_to_load_things.retain(|x| !indices_to_remove.contains(x));
        for (i, entity) in self.world.entities.iter_mut().enumerate() {
            if self.entities_wanting_to_load_things.contains(&i) {
                continue;
            }
            if let Some(mesh_renderer) = entity.get_component(COMPONENT_TYPE_MESH_RENDERER.clone()) {
                if let Some(mesh) = mesh_renderer.get_parameter("mesh") {
                    // get the string value of the mesh
                    let mut mesh_name = match mesh.value {
                        ParameterValue::String(ref s) => s.clone(),
                        _ => {
                            error!("render: mesh is not a string");
                            continue;
                        }
                    };
                    // if so, render it
                    let mesh = renderer.meshes.get(&*mesh_name).cloned();
                    if let Some(mut mesh) = mesh {
                        let texture = mesh_renderer.get_parameter("texture").unwrap();
                        let texture_name = match texture.value {
                            ParameterValue::String(ref s) => s.clone(),
                            _ => {
                                error!("render: texture is not a string");
                                continue;
                            }
                        };
                        let texture = renderer.textures.get(&*texture_name).cloned();
                        if texture.is_none() {
                            error!("texture not found: {:?}", texture_name);
                            continue;
                        }
                        let texture = texture.unwrap();

                        let old_position = mesh.position;
                        let old_rotation = mesh.rotation;
                        let old_scale = mesh.scale;

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
                                mesh.scale *= scale;
                            }
                        }

                        // add a bit of rotation to the transform to make things more interesting
                        //entity.set_component_parameter(COMPONENT_TYPE_TRANSFORM.clone(), "rotation", Box::new(Quaternion::from_euler_angles_zyx(&Vec3::new(0.0, self.counter, 0.0))));

                        let mut anim_weights = None;
                        if mesh_name == "player" {
                            let move_anim = MoveAnim::from_values(0.0, 0.0);
                            anim_weights = Some(move_anim.weights());
                        }

                        mesh.render(renderer, Some(&texture), anim_weights, shadow_pass);
                        mesh.position = old_position;
                        mesh.rotation = old_rotation;
                        mesh.scale = old_scale;
                        *renderer.meshes.get_mut(&*mesh_name).unwrap() = mesh;
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
            if let Some(player_component) = entity.get_component(COMPONENT_TYPE_PLAYER.clone()) {
                if let Some(ignore) = self.ignore_this_entity {
                    if ignore == entity.uid {
                        continue;
                    }
                }
                let position = player_component.get_parameter("position").unwrap();
                let position = match position.value {
                    ParameterValue::Vec3(v) => v,
                    _ => {
                        error!("render: player position is not a vec3");
                        continue;
                    }
                };
                let rotation = player_component.get_parameter("rotation").unwrap();
                let rotation = match rotation.value {
                    ParameterValue::Quaternion(v) => v,
                    _ => {
                        error!("render: player rotation is not a quaternion");
                        continue;
                    }
                };
                let speed = player_component.get_parameter("speed").unwrap();
                let speed = match speed.value {
                    ParameterValue::Float(v) => v,
                    _ => {
                        error!("render: player speed is not a float");
                        continue;
                    }
                };
                let strafe = player_component.get_parameter("strafe").unwrap();
                let strafe = match strafe.value {
                    ParameterValue::Float(v) => v,
                    _ => {
                        error!("render: player strafe is not a float");
                        continue;
                    }
                };
                if let Some(mesh) = renderer.meshes.get("player").cloned() {
                    renderer.meshes.get_mut("player").unwrap().updated_animations_this_frame = false;
                    if let Some(shadow_mesh) = &renderer.meshes.get_mut("player").unwrap().shadow_mesh {
                        shadow_mesh.lock().unwrap().updated_animations_this_frame = false;
                    }
                    let texture = renderer.textures.get("default").cloned().unwrap();
                    let mut mesh = mesh.clone();
                    let old_position = mesh.position;
                    let old_rotation = mesh.rotation;
                    mesh.position = position + Vec3::new(0.0, -0.2, 0.0);
                    mesh.rotation = rotation;
                    mesh.scale = Vec3::new(0.6, 0.6, 0.6);

                    let move_anim = MoveAnim::from_values(speed, strafe);

                    mesh.render(renderer, Some(&texture), Some(move_anim.weights()), shadow_pass);

                    mesh.position = old_position;
                    mesh.rotation = old_rotation;
                    *renderer.meshes.get_mut("player").unwrap() = mesh;
                }
            }
        }
    }

    pub fn handle_audio(&mut self, renderer: &ht_renderer, audio: &AudioBackend, scontext: &SoundContext) {
        audio.update(renderer.camera.get_position(), -renderer.camera.get_front(), renderer.camera.get_up(), scontext);

        for index in self.entities_wanting_to_load_things.clone() {
            let entity = &self.world.entities[index];
            let components = entity.get_components();
            for component in components {
                match component.get_type() {
                    x if x == COMPONENT_TYPE_JUKEBOX.clone() => {
                        let track = component.get_parameter("track").unwrap();
                        let track = match track.value {
                            ParameterValue::String(ref s) => s.clone(),
                            _ => {
                                error!("audio: jukebox track is not a string");
                                continue;
                            }
                        };
                        // check if the track is already loaded
                        if !audio.is_sound_loaded(&track) {
                            audio.load_sound(&track);
                        }
                    }
                    _ => {}
                }
            }
        }
        // don't clear here because that's done later in rendering


        for (i, entity) in self.world.entities.iter_mut().enumerate() {
            if let Some(jukebox) = entity.get_component(COMPONENT_TYPE_JUKEBOX.clone()) {
                let track = jukebox.get_parameter("track").unwrap();
                let track = match track.value {
                    ParameterValue::String(ref s) => s.clone(),
                    _ => {
                        error!("audio: jukebox track is not a string");
                        continue;
                    }
                };
                let volume = jukebox.get_parameter("volume").unwrap();
                let volume = match volume.value {
                    ParameterValue::Float(v) => v,
                    _ => {
                        error!("audio: jukebox volume is not a float");
                        continue;
                    }
                };
                let playing = jukebox.get_parameter("playing").unwrap();
                let playing = match playing.value {
                    ParameterValue::Bool(ref s) => s.clone(),
                    _ => {
                        error!("audio: jukebox playing is not a string");
                        continue;
                    }
                };
                let uuid = jukebox.get_parameter("uuid").unwrap();
                let uuid = match uuid.value {
                    ParameterValue::String(ref s) => s.clone(),
                    _ => {
                        error!("audio: jukebox uuid is not a string");
                        continue;
                    }
                };

                let position = if let Some(transform) = entity.get_component(COMPONENT_TYPE_TRANSFORM.clone()) {
                    let position = transform.get_parameter("position").unwrap();
                    let position = match position.value {
                        ParameterValue::Vec3(v) => v,
                        _ => {
                            error!("audio: transform position is not a vec3");
                            continue;
                        }
                    };
                    position
                } else {
                    Vec3::new(0.0, 0.0, 0.0)
                };

                if audio.is_sound_loaded(&track) {
                    if playing && !audio.is_sound_playing(&uuid) {
                        audio.play_sound_with_uuid(&uuid, &track, scontext);
                    } else if !playing && audio.is_sound_playing(&uuid) {
                        audio.stop_sound_with_uuid(&uuid, scontext);
                    }
                    if playing {
                        audio.set_sound_position(&uuid, position, scontext);
                    }
                } else {
                    // if not, add it to the list of things to load
                    self.entities_wanting_to_load_things.push(i);
                }
            }
        }
    }
}