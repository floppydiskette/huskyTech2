use halfbrown::HashMap;
use std::collections::{VecDeque};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use async_recursion::async_recursion;
use gfx_maths::*;
use tokio::sync::{mpsc, Mutex, watch};
use tokio::time::{Instant, Duration};
use serde::{Serialize, Deserialize};
use tokio::net::TcpStream;
use crate::physics::PhysicsSystem;
use crate::server::connections::SteadyMessageQueue;
use crate::server::lan::{ClientLanConnection, LanConnection, LanListener};
use crate::server::server_player::{ServerPlayer, ServerPlayerContainer};
use crate::worldmachine::{EntityId, WorldMachine, WorldUpdate};
use crate::worldmachine::ecs::{ComponentType, Entity, ParameterValue};
use crate::worldmachine::player::{MovementInfo, PlayerComponent};
use crate::worldmachine::snowballs::Snowball;

pub mod connections;
pub mod server_player;
pub mod lan;

pub type PacketUUID = String;
pub type ConnectionUUID = String;

#[derive(Clone, Debug)]
pub enum Connection {
    Local(Arc<Mutex<LocalConnection>>),
    Lan(LanListener, LanConnection),
}

#[derive(Clone)]
pub enum ConnectionClientside {
    Local(Arc<Mutex<LocalConnectionClientSide>>),
    Lan(ClientLanConnection),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FastPacket {
    ChangePosition(EntityId, Vec3),
    ChangeRotation(EntityId, Quaternion),
    ChangeScale(EntityId, Vec3),
    PlayerMoved(EntityId, Vec3, Quaternion, Quaternion),
    EntitySetParameter(EntityId, ComponentType, String, ParameterValue),
    PlayerMove(ConnectionUUID, Vec3, Vec3, Quaternion, Quaternion, Option<MovementInfo>),
    // connection uuid, position, displacement_vector, rotation, head rotation, movement info
    PlayerJump(ConnectionUUID),
    PlayerFuckYouMoveHere(Vec3),
    // connection uuid, position,
    PlayerCheckPosition(ConnectionUUID, Vec3),
    // connection uuid, position
    PlayerFuckYouSetRotation(Quaternion), // connection uuid, rotation
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FastPacketData {
    pub packet: Option<FastPacket>,
}

unsafe impl Send for FastPacketData {}

unsafe impl Sync for FastPacketData {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NameRejectionReason {
    IllegalWord,
    Taken,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SteadyPacket {
    Consume(PacketUUID),
    SelfTest,
    KeepAlive,
    InitialiseEntity(EntityId, Entity),
    RemoveEntity(EntityId),
    FinaliseMapLoad,
    InitialisePlayer(ConnectionUUID, EntityId, String, Vec3, Quaternion, Vec3),
    // uuid, (entity id so we know to ignore updates from that entity), name, position, rotation, scale
    Message(String),
    ChatMessage(ConnectionUUID, String),
    SetName(ConnectionUUID, String),
    NameRejected(NameRejectionReason),
    ThrowSnowball(String, Vec3, Vec3) // uuid, position, initial velocity
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SteadyPacketData {
    pub packet: Option<SteadyPacket>,
    pub uuid: Option<PacketUUID>,
}

#[derive(Clone, Debug)]
pub struct LocalConnection {
    pub fast_update_sender: mpsc::Sender<FastPacketData>,
    pub steady_update_sender: mpsc::Sender<SteadyPacketData>,
    pub fast_update_receiver: watch::Receiver<FastPacketData>,
    steady_update_receiver: watch::Receiver<SteadyPacketData>,
    pub consume_receiver_queue: Arc<Mutex<SteadyMessageQueue>>,
    pub uuid: ConnectionUUID,
}

pub struct LocalConnectionClientSide {
    pub fast_update_sender: watch::Sender<FastPacketData>,
    pub steady_update_sender: watch::Sender<SteadyPacketData>,
    pub steady_sender_queue: Arc<Mutex<SteadyMessageQueue>>,
    pub fast_update_receiver: mpsc::Receiver<FastPacketData>,
    pub steady_update_receiver: mpsc::Receiver<SteadyPacketData>,
}

#[derive(Clone, Debug)]
pub enum Connections {
    Local(Arc<Mutex<Vec<Arc<Mutex<LocalConnection>>>>>),
    Lan(LanListener, Arc<Mutex<Vec<LanConnection>>>),
}

#[derive(Clone)]
pub struct Server {
    pub connections: Connections,
    pub connections_incoming: Arc<Mutex<VecDeque<TcpStream>>>,
    pub worldmachine: Arc<Mutex<WorldMachine>>,
}

pub fn generate_uuid() -> PacketUUID {
    uuid::Uuid::new_v4().to_string()
}

impl Server {
    pub fn new(map_name: &str, physics: PhysicsSystem) -> Self {
        let mut worldmachine = WorldMachine::default();
        worldmachine.initialise(physics, true);
        worldmachine.load_map(map_name).expect("failed to load map");

        worldmachine.players = Some(Arc::new(Mutex::new(HashMap::new())));

        info!("server started");

        Self {
            connections: Connections::Local(Arc::new(Mutex::new(Vec::new()))),
            connections_incoming: Arc::new(Mutex::new(VecDeque::new())),
            worldmachine: Arc::new(Mutex::new(worldmachine)),
        }
    }

    pub async fn new_host_lan_server(map_name: &str, physics: PhysicsSystem, tcp_port: u16, udp_port: u16, hostname: &str) -> Self {
        let mut worldmachine = WorldMachine::default();
        worldmachine.initialise(physics, true);
        worldmachine.load_map(map_name).expect("failed to load map");

        worldmachine.players = Some(Arc::new(Mutex::new(HashMap::new())));

        let listener = LanListener::new(hostname, tcp_port, udp_port).await;

        let the_self = Self {
            connections: Connections::Lan(listener.clone(), Arc::new(Mutex::new(Vec::new()))),
            connections_incoming: Arc::new(Mutex::new(VecDeque::new())),
            worldmachine: Arc::new(Mutex::new(worldmachine)),
        };
        let the_clone = the_self.clone();
        let listener_clone = listener;
        tokio::spawn(async move {
            loop {
                the_clone.connection_listening_thread(listener_clone.clone()).await;
            }
        });

        info!("server started");
        the_self
    }

    async fn connection_listening_thread(&self, listener: LanListener) {
        loop {
            let new_connection = listener.poll_new_connection().await;
            if let Some(new_connection) = new_connection {
                self.connections_incoming.lock().await.push_back(new_connection);
            }
        }
    }

    pub async fn listen_for_lan_connections(&mut self) {
        if let Connections::Lan(listener, _connections_raw) = self.connections.clone() {
            let mut connections_incoming = self.connections_incoming.lock().await;
            while let Some(connection) = connections_incoming.pop_front() {
                let the_clone = self.clone();
                let listener_clone = listener.clone();
                tokio::spawn(async move {
                    let connection = listener_clone.clone().init_new_connection(connection).await;
                    if connection.is_none() {
                        return;
                    }
                    let connection = connection.unwrap();
                    let the_listener_clone = listener_clone.clone();
                    the_clone.new_connection(Connection::Lan(the_listener_clone.clone(), connection.clone())).await;
                });
                debug!("spawned new connection thread");
            }
        }
    }

    async fn get_connection_uuid(&self, connection: &Connection) -> ConnectionUUID {
        match connection {
            Connection::Local(local_connection) => local_connection.lock().await.uuid.clone(),
            Connection::Lan(_, lan_connection) => lan_connection.uuid.clone(),
        }
    }

    async unsafe fn send_steady_packet_unsafe(&self, connection_og: &Connection, packet: SteadyPacketData) -> bool {
        let mut tries = 0;
        const MAX_TRIES: i32 = 10;

        match connection_og.clone() {
            Connection::Local(connection) => {
                let connection_lock = connection.lock().await;
                connection_lock.steady_update_sender.send(packet.clone()).await.unwrap();
                debug!("sent packet to local connection");
                // wait for the packet to be consumed
                drop(connection_lock);
                loop {
                    { // hack so that we can get consume packets
                        self.handle_steady_packets(connection_og.clone()).await; // TODO: this may cause a stack overflow in the future, watch out!
                    }
                    {
                        let connection_lock = connection.lock().await.clone();
                        let mut queue = connection_lock.consume_receiver_queue.lock().await;
                        if let Some(packet_data) = queue.pop().await {
                            if let SteadyPacket::Consume(uuid) = packet_data.packet.clone().unwrap() {
                                if uuid == packet.clone().uuid.unwrap() {
                                    debug!("packet consumed");
                                    drop(queue);
                                    break;
                                } else {
                                    // put the packet back in the queue
                                    queue.push(packet_data);
                                }
                            } else {
                                // put the packet back in the queue
                                queue.push(packet_data);
                            }
                        }
                    }
                }
            }
            Connection::Lan(_, connection) => {
                let mut start_time = Instant::now();
                let retry_time = Duration::from_millis(1000);
                let timeout_time = Duration::from_millis(50000);
                loop {
                    loop {
                        let res = connection.serialise_and_send_steady(packet.clone()).await;
                        if res.is_ok() {
                            break;
                        } else if tries < MAX_TRIES {
                            tries += 1;
                            warn!("failed to send packet to lan connection, retrying");
                            continue;
                        } else {
                            return false;
                        }
                    }
                    debug!("sent packet to lan connection");
                    debug!("uuid: {:?}", packet.clone().uuid);
                    debug!("type: {:?}", packet.clone().packet);
                    tokio::time::sleep(Duration::from_millis(100)).await; // wait a bit cause the packet might still be sending
                    // wait for the packet to be consumed
                    tries = 0;
                    loop {
                        let received_packet = connection.attempt_receive_steady_and_deserialise().await;
                        if let Err(e) = received_packet {
                            warn!("failed to receive packet from lan connection: {:?}", e);
                            return false;
                        }
                        let received_packet = received_packet.unwrap();
                        if let Some(received_packet) = received_packet {
                            if let SteadyPacket::Consume(uuid) = received_packet.packet.clone().unwrap() {
                                if uuid == packet.clone().uuid.unwrap() {
                                    debug!("packet consumed");
                                    return true;
                                } else {
                                    // requeue the packet
                                    connection.steady_receive_queue.lock().await.0.push(received_packet);
                                }
                            } else {
                                // requeue the packet
                                connection.steady_receive_queue.lock().await.0.push(received_packet);
                            }
                        } else if tries < MAX_TRIES {
                            tries += 1;
                            warn!("failed to receive packet from lan connection, retrying");
                            tokio::time::sleep(Duration::from_millis(100)).await; // wait a bit cause the packet might still be sending
                            continue;
                        } else {
                            if start_time.elapsed() > timeout_time {
                                warn!("timed out waiting for packet to be consumed");
                                return false;
                            }
                            if start_time.elapsed() > retry_time {
                                warn!("retrying packet");
                                start_time = Instant::now();
                                break;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    async fn send_steady_packet(&self, connection: &Connection, packet: SteadyPacket) -> bool {
        match connection.clone() {
            Connection::Local(connection) => {
                let uuid = generate_uuid();
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                unsafe {
                    self.send_steady_packet_unsafe(&Connection::Local(connection), packet_data).await
                }
            }
            Connection::Lan(listener, connection) => {
                let uuid = generate_uuid();
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                unsafe {
                    self.send_steady_packet_unsafe(&Connection::Lan(listener, connection), packet_data).await
                }
            }
        }
    }

    pub async fn send_fast_packet(&self, connection: &Connection, packet: FastPacket) {
        match connection.clone() {
            Connection::Local(connection) => {
                let connection = connection.lock().await;
                let packet_data = FastPacketData {
                    packet: Some(packet),
                };
                connection.fast_update_sender.send(packet_data).await.unwrap();
            }
            Connection::Lan(listener, connection) => {
                let packet_data = FastPacketData {
                    packet: Some(packet),
                };
                connection.serialise_and_send_fast(connection.uuid.clone(), listener.clone(), packet_data).await.unwrap();
            }
        }
    }

    pub async fn try_receive_fast_packet(&mut self, connection: &Connection) -> Option<FastPacket> {
        match connection.clone() {
            Connection::Local(connection) => {
                let mut connection = connection.lock().await;
                if !connection.fast_update_receiver.has_changed().ok().unwrap_or(false) {
                    return None;
                }
                let message = connection.fast_update_receiver.borrow_and_update().clone();
                if let Some(packet) = message.packet {
                    return Some(packet);
                }
            }
            Connection::Lan(listener, connection) => {
                let packet = connection.attempt_receive_fast_and_deserialise(listener).await;
                if let Some(packet) = packet {
                    return Some(packet.packet.unwrap());
                }
            }
        }
        None
    }

    pub async fn begin_connection(&self, connection: Connection) -> Option<EntityId> {
        let worldmachine = self.worldmachine.lock().await;
        // for each entity in the worldmachine, send an initialise packet
        let world_clone = worldmachine.world.clone();
        let physics = worldmachine.physics.lock().unwrap().clone().unwrap();
        // drop worldmachine so we don't hold the lock while we send packets
        drop(worldmachine);
        for entity in world_clone.entities.iter() {
            let res = self.send_steady_packet(&connection, SteadyPacket::InitialiseEntity(entity.uid, entity.clone())).await;
            if !res {
                return None;
            }
        }
        let uuid = self.get_connection_uuid(&connection).await;

        let name = "morbius";

        let position = Vec3::new(0.0, 2.0, 0.0);
        let rotation = Quaternion::identity();
        let scale = Vec3::new(1.0, 1.0, 1.0);

        let mut player = ServerPlayer::new(uuid.as_str(), name, position, rotation, scale);

        player.init(physics.clone());

        let mut player_entity = Entity::new(player.name.as_str());
        let entity_uuid = player_entity.uid;
        let player_component = PlayerComponent::new(name, uuid.clone(), position, rotation, scale);
        player_entity.add_component(player_component);

        // relock worldmachine
        let mut worldmachine = self.worldmachine.lock().await;
        worldmachine.world.entities.push(player_entity.clone());

        let res = self.send_steady_packet(&connection, SteadyPacket::InitialisePlayer(
            player.uuid.clone(),
            entity_uuid,
            player.name.clone(),
            position,
            rotation,
            scale)).await;
        worldmachine.queue_update(WorldUpdate::InitEntity(entity_uuid, player_entity.clone())).await;

        if !res {
            return None;
        }

        worldmachine.players.as_mut().unwrap().lock().await.insert(uuid.clone(), ServerPlayerContainer {
            player: player.clone(),
            entity_id: Some(entity_uuid),
        });

        drop(worldmachine);

        let res = self.send_steady_packet(&connection, SteadyPacket::FinaliseMapLoad).await;

        if res {
            Some(entity_uuid)
        } else {
            None
        }
    }

    #[async_recursion]
    async fn handle_steady_packets(&self, connection: Connection) -> bool {
        match connection.clone() {
            Connection::Local(local_connection) => {
                let mut local_connection = local_connection.lock().await;
                if local_connection.steady_update_receiver.has_changed().unwrap_or(false) {
                    let steady_packet_data = local_connection.steady_update_receiver.borrow_and_update().clone();
                    if let Some(steady_packet) = steady_packet_data.clone().packet {
                        match steady_packet {
                            SteadyPacket::KeepAlive => {
                                // do nothing
                            }
                            SteadyPacket::InitialiseEntity(_uid, _entity) => {
                                // client shouldn't be sending this
                                debug!("client sent initialise packet");
                            }
                            SteadyPacket::Consume(_) => {
                                local_connection.consume_receiver_queue.lock().await.push(steady_packet_data);
                            }

                            // client shouldn't be sending these
                            SteadyPacket::SelfTest => {}
                            SteadyPacket::InitialisePlayer(_, _, _, _, _, _) => {}
                            SteadyPacket::Message(_) => {}
                            SteadyPacket::FinaliseMapLoad => {}
                            SteadyPacket::RemoveEntity(_) => {}
                            SteadyPacket::ChatMessage(_who_sent, message) => {
                                // mirror to all other clients
                                let who_sent = local_connection.uuid.clone();
                                let packet = SteadyPacket::ChatMessage(who_sent, message);
                                drop(local_connection);
                                match &self.connections {
                                    Connections::Local(local_connections) => {
                                        let cons = local_connections.lock().await.clone();
                                        for connection in cons.iter() {
                                            self.send_steady_packet(&Connection::Local(connection.clone()), packet.clone()).await;
                                        }
                                    }
                                    Connections::Lan(_, _) => {
                                        unimplemented!()
                                    }
                                }
                            }
                            SteadyPacket::SetName(_who_sent, new_name) => {
                                // mirror to all other clients
                                let who_sent = local_connection.uuid.clone();
                                let packet = SteadyPacket::SetName(who_sent, new_name);
                                drop(local_connection);
                                match &self.connections {
                                    Connections::Local(local_connections) => {
                                        let cons = local_connections.lock().await.clone();
                                        for connection in cons.iter() {
                                            self.send_steady_packet(&Connection::Local(connection.clone()), packet.clone()).await;
                                        }
                                    }
                                    Connections::Lan(_, _) => {
                                        unimplemented!()
                                    }
                                }
                            }
                            SteadyPacket::ThrowSnowball(_uuid, _positon, _initial_velocity) => {
                                // as server is authoritative, calculate the snowball's position and velocity ourselves
                                // position will be the player's position
                                // velocity will be the player's velocity + the player's forward vector * 10
                                let mut worldmachine = self.worldmachine.lock().await;
                                let mut players = worldmachine.players.as_mut().unwrap().lock().await;
                                let player = players.get_mut(&local_connection.uuid).unwrap();
                                let snowball_cooldown = player.player.snowball_cooldown;
                                if snowball_cooldown <= 0.0 {
                                    player.player.snowball_cooldown = 0.5;
                                    let position = player.player.get_position(None, None).await;
                                    let mut rotation = player.player.get_head_rotation(None, None).await;
                                    rotation.w = -rotation.w;
                                    let forward = rotation.forward();
                                    let forward = Vec3::new(forward.x, forward.y, forward.z);
                                    let position = forward * 1.5 + Vec3::new(0.0, 0.1, 0.0) + position;
                                    let velocity = forward * 20.0 + Vec3::new(0.0, 5.0, 0.0);
                                    drop(players);

                                    let snowball = Snowball::new(position, velocity, worldmachine.physics.lock().unwrap().as_ref().unwrap());
                                    // send to all clients (including the one that sent it)
                                    let packet = SteadyPacket::ThrowSnowball(snowball.uuid.clone(), position, velocity);

                                    worldmachine.snowballs.push(snowball);
                                    drop(local_connection);
                                    match &self.connections {
                                        Connections::Local(local_connections) => {
                                            let cons = local_connections.lock().await.clone();
                                            for connection in cons.iter() {
                                                self.send_steady_packet(&Connection::Local(connection.clone()), packet.clone()).await;
                                            }
                                        }
                                        Connections::Lan(_, _) => {
                                            unimplemented!()
                                        }
                                    }
                                }
                            }
                            SteadyPacket::NameRejected(_) => {}
                        }
                    }
                }
            }
            Connection::Lan(_, connection) => {
                let packet = connection.attempt_receive_steady_and_deserialise().await;
                if let Err(e) = packet {
                    debug!("error receiving steady packet: {:?}", e);
                    return false;
                }
                let packet_og = packet.unwrap();
                if let Some(packet) = packet_og.clone() {
                    match packet.packet.unwrap() {
                        SteadyPacket::KeepAlive => {
                            // do nothing
                        }
                        SteadyPacket::Consume(_) => {
                            // oops, requeue the packet
                            connection.steady_receive_queue.lock().await.0.push(packet_og.unwrap());
                        }
                        SteadyPacket::ChatMessage(_who_sent, message) => {
                            // mirror to all other clients
                            let who_sent = connection.uuid.clone();
                            let packet = SteadyPacket::ChatMessage(who_sent, message);
                            match &self.connections {
                                Connections::Lan(listener, connections) => {
                                    let cons = connections.lock().await.clone();
                                    for a_connection in cons.iter() {
                                        self.send_steady_packet(&Connection::Lan(listener.clone(), a_connection.clone()), packet.clone()).await;
                                    }
                                }
                                Connections::Local(_) => {
                                    unimplemented!()
                                }
                            }
                        }
                        SteadyPacket::SetName(_who_sent, new_name) => {
                            match &self.connections {
                                Connections::Lan(listener, connections) => {
                                    // check if name is taken
                                    let mut name_taken = false;
                                    let mut wm = self.worldmachine.lock().await;
                                    let players = wm.players.as_mut().unwrap().lock().await;
                                    for player in players.values() {
                                        if player.player.name == new_name {
                                            name_taken = true;
                                            break;
                                        }
                                    }
                                    drop(players);
                                    drop(wm);
                                    if name_taken {
                                        self.send_steady_packet(&Connection::Lan(listener.clone(), connection.clone()), SteadyPacket::NameRejected(NameRejectionReason::Taken)).await;
                                    } else {
                                        // set name
                                        let mut wm = self.worldmachine.lock().await;
                                        let mut players = wm.players.as_mut().unwrap().lock().await;
                                        let player = players.get_mut(&connection.uuid).unwrap();
                                        player.player.name = new_name.clone();
                                        drop(players);
                                        drop(wm);

                                        // send to all other clients
                                        let packet = SteadyPacket::SetName(connection.uuid.clone(), new_name);
                                        let cons = connections.lock().await.clone();
                                        for a_connection in cons {
                                            self.send_steady_packet(&Connection::Lan(listener.clone(), a_connection.clone()), packet.clone()).await;
                                        }
                                    }
                                }
                                Connections::Local(_) => {
                                    unimplemented!()
                                }
                            }
                        }
                        SteadyPacket::ThrowSnowball(_uuid, _positon, _initial_velocity) => {
                            // as server is authoritative, calculate the snowball's position and velocity ourselves
                            // position will be the player's position
                            // velocity will be the player's velocity + the player's forward vector * 10
                            let mut worldmachine = self.worldmachine.lock().await;
                            let mut players = worldmachine.players.as_mut().unwrap().lock().await;
                            let player = players.get_mut(&connection.uuid).unwrap();
                            let mut position = player.player.get_position(None, None).await;
                            let rotation = player.player.get_rotation(None, None).await;
                            position = position + (-rotation.right() * 1.2);
                            let velocity = -rotation.right() * 10.0;
                            drop(players);

                            let snowball = Snowball::new(position, velocity, worldmachine.physics.lock().unwrap().as_ref().unwrap());
                            // send to all clients (including the one that sent it)
                            let packet = SteadyPacket::ThrowSnowball(snowball.uuid.clone(), position, velocity);

                            worldmachine.snowballs.push(snowball);
                            match &self.connections {
                                Connections::Lan(listener, connections) => {
                                    let cons = connections.lock().await.clone();
                                    for a_connection in cons.iter() {
                                        self.send_steady_packet(&Connection::Lan(listener.clone(), a_connection.clone()), packet.clone()).await;
                                    }
                                }
                                Connections::Local(_) => {
                                    unimplemented!()
                                }
                            }
                        }

                        // client shouldn't be sending these

                        SteadyPacket::InitialiseEntity(_, _) => {}
                        SteadyPacket::SelfTest => {}
                        SteadyPacket::InitialisePlayer(_, _, _, _, _, _) => {}
                        SteadyPacket::Message(_) => {}
                        SteadyPacket::FinaliseMapLoad => {}
                        SteadyPacket::RemoveEntity(_) => {}
                        SteadyPacket::NameRejected(_) => {}
                    }
                }
            }
        }
        true
    }

    async fn player_move(&self, connection: Connection, packet: FastPacket) {
        if let FastPacket::PlayerMove(uuid, position, displacement_vector, rotation, head_rotation, movement_info) = packet {
            let worldmachine = self.worldmachine.clone();
            let mut worldmachine = worldmachine.lock().await;
            let mut players = worldmachine.players.clone();
            let mut players = players.as_mut().unwrap().lock().await;
            let player = players.get_mut(&uuid).unwrap();
            let success = player.player.attempt_position_change(position, displacement_vector, rotation, head_rotation, movement_info.unwrap_or_default(), player.entity_id, &mut worldmachine).await;
            if success {} else {
                let connection = connection.clone();
                self.send_fast_packet(&connection, FastPacket::PlayerFuckYouMoveHere(player.player.get_position(player.entity_id, Some(&mut worldmachine)).await)).await
            }
            if player.player.get_position(None, None).await.y < -20.0 {
                let connection = connection.clone();
                player.player.set_position(Vec3::new(0.0, 0.0, 0.0), player.entity_id, &mut worldmachine).await;
                self.send_fast_packet(&connection, FastPacket::PlayerFuckYouMoveHere(player.player.get_position(player.entity_id, Some(&mut worldmachine)).await)).await
            }
        }
    }

    async fn player_check_position(&self, connection: Connection, packet: FastPacket) {
        if let FastPacket::PlayerCheckPosition(uuid, position) = packet {
            let worldmachine = self.worldmachine.clone();
            let mut worldmachine = worldmachine.lock().await;
            let mut players = worldmachine.players.clone();
            let mut players = players.as_mut().unwrap().lock().await;
            let player = players.get_mut(&uuid).unwrap();
            let server_position = player.player.get_position(player.entity_id, Some(&mut worldmachine)).await;
            let success = server_position == position;
            if success {} else {
                self.send_fast_packet(&connection, FastPacket::PlayerFuckYouMoveHere(player.player.get_position(player.entity_id, Some(&mut worldmachine)).await)).await
            }
        }
    }

    async fn handle_fast_packets(&self, connection: Connection) {
        match connection.clone() {
            Connection::Local(local_connection) => {
                let mut local_connection = local_connection.lock().await;
                if local_connection.fast_update_receiver.has_changed().unwrap_or(false) {
                    let fast_packet_data = local_connection.fast_update_receiver.borrow_and_update().clone();
                    drop(local_connection);
                    if let Some(fast_packet) = fast_packet_data.packet {
                        match fast_packet.clone() {
                            /// sent when the player wants to move
                            FastPacket::PlayerMove(_, _, _, _, _, _) => {
                                self.player_move(connection.clone(), fast_packet).await;
                            }
                            /// sent when player is attempting to check if their position is correct against the server's stored position
                            FastPacket::PlayerCheckPosition(_, _) => {
                                self.player_check_position(connection.clone(), fast_packet).await;
                            }

                            /// sent when the player jumps (deprecated)
                            FastPacket::PlayerJump(uuid) => {
                                //let mut worldmachine = self.worldmachine.clone();
                                //let mut worldmachine = worldmachine.lock().await;
                                //let mut players = worldmachine.players.clone();
                                //let mut players = players.as_mut().unwrap().lock().await;
                                //let player = players.get_mut(&uuid).unwrap();
                                //if !player.player.attempt_jump() {
                                //    drop(local_connection);
                                //    let mut connection = connection.clone();
                                //    self.send_fast_packet(&connection, FastPacket::PlayerFuckYouMoveHere(player.player.get_position())).await
                                //}
                            }

                            // client shouldn't be sending these
                            FastPacket::ChangePosition(_, _) => {}
                            FastPacket::ChangeRotation(_, _) => {}
                            FastPacket::ChangeScale(_, _) => {}
                            FastPacket::PlayerFuckYouMoveHere(_) => {}
                            FastPacket::PlayerFuckYouSetRotation(_) => {}
                            FastPacket::EntitySetParameter(_, _, _, _) => {}
                            FastPacket::PlayerMoved(_, _, _, _) => {}
                        }
                    }
                }
            }
            Connection::Lan(listener, lan_connection) => {
                let listener = listener.clone();
                let packet = lan_connection.attempt_receive_fast_and_deserialise(listener).await;
                if let Some(packet) = packet {
                    match packet.clone().packet.unwrap() {
                        /// sent when the player wants to move
                        FastPacket::PlayerMove(_, _, _, _, _, _) => {
                            self.player_move(connection.clone(), packet.clone().packet.unwrap()).await;
                        }
                        /// sent when player is attempting to check if their position is correct against the server's stored position
                        FastPacket::PlayerCheckPosition(_, _) => {
                            self.player_check_position(connection.clone(), packet.clone().packet.unwrap()).await;
                        }

                        /// deprecated
                        FastPacket::PlayerJump(uuid) => {}

                        // client shouldn't be sending these
                        FastPacket::ChangePosition(_, _) => {}
                        FastPacket::ChangeRotation(_, _) => {}
                        FastPacket::ChangeScale(_, _) => {}
                        FastPacket::PlayerFuckYouMoveHere(_) => {}
                        FastPacket::PlayerFuckYouSetRotation(_) => {}
                        FastPacket::EntitySetParameter(_, _, _, _) => {}
                        FastPacket::PlayerMoved(_, _, _, _) => {}
                    }
                }
            }
        }
    }

    pub async fn handle_connection(&self, connection: Connection) -> bool {
        loop {
            match connection.clone() {
                Connection::Local(local_connection) => {
                    self.handle_fast_packets(Connection::Local(local_connection.clone())).await;
                    self.handle_steady_packets(Connection::Local(local_connection.clone())).await;
                }
                Connection::Lan(listener, lan_connection) => {
                    self.handle_fast_packets(Connection::Lan(listener.clone(), lan_connection.clone())).await;
                    let connection = self.handle_steady_packets(Connection::Lan(listener.clone(), lan_connection.clone())).await;
                    if !connection {
                        return false;
                    }
                }
            }
        }
    }

    async fn assert_connection_type_allowed(&self, connection: Connection) -> bool {
        match connection {
            Connection::Local(_) => {
                matches!(self.connections.clone(), Connections::Local(_))
            }
            Connection::Lan(_, _) => {
                matches!(self.connections.clone(), Connections::Lan(_, _))
            }
        }
    }

    async fn new_connection(&self, connection: Connection) {
        if self.assert_connection_type_allowed(connection.clone()).await {
            match connection.clone() {
                Connection::Local(local_connection) => {
                    let _connection_index = match self.connections.clone() {
                        Connections::Local(connections) => {
                            let mut connections = connections.lock().await;
                            connections.push(local_connection.clone());
                            connections.len() - 1
                        }
                        _ => {
                            panic!("assert_connection_type_allowed failed");
                        }
                    };
                    self.begin_connection(connection.clone()).await;
                    self.handle_connection(connection).await;
                }
                Connection::Lan(_, lan_connection) => {
                    let _connection_index = match self.connections.clone() {
                        Connections::Lan(_, connections) => {
                            let mut connections = connections.lock().await;
                            connections.push(lan_connection.clone());
                            connections.len() - 1
                        }
                        _ => {
                            panic!("assert_connection_type_allowed failed");
                        }
                    };
                    let player_entity_id = self.begin_connection(connection.clone()).await;
                    if player_entity_id.is_none() {
                        let connections = match self.connections.clone() {
                            Connections::Lan(_, connections) => {
                                connections.clone()
                            }
                            _ => {
                                panic!("assert_connection_type_allowed failed");
                            }
                        };
                        let mut connections = connections.lock().await;
                        connections.retain(|x| x.uuid != lan_connection.uuid);
                        debug!("connections: {:?}", connections.len());
                        return;
                    }
                    let player_entity_id = player_entity_id.expect("player_entity_id is None");
                    let connected = self.handle_connection(connection).await;
                    if !connected {
                        let connections = match self.connections.clone() {
                            Connections::Lan(_, connections) => {
                                connections.clone()
                            }
                            _ => {
                                panic!("assert_connection_type_allowed failed");
                            }
                        };
                        let mut connections = connections.lock().await;
                        connections.retain(|x| x.uuid != lan_connection.uuid);
                        debug!("connections: {:?}", connections.len());
                        drop(connections);
                        // remove the player from the world
                        let worldmachine = self.worldmachine.clone();
                        let mut worldmachine = worldmachine.lock().await;
                        let entity_index = worldmachine.get_entity_index(player_entity_id).unwrap();
                        worldmachine.world.entities.remove(entity_index);
                        worldmachine.queue_update(WorldUpdate::EntityNoLongerExists(player_entity_id)).await;
                    }
                }
            }
        }
    }

    pub async fn join_local_server(&mut self) -> Arc<Mutex<LocalConnectionClientSide>> {
        info!("joining local server");
        let (fast_update_sender_client, fast_update_receiver_server) = watch::channel(FastPacketData {
            packet: None,
        });
        let (steady_update_sender_client, steady_update_receiver_server) = watch::channel(SteadyPacketData {
            packet: None,
            uuid: None,
        });
        let (fast_update_sender_server, fast_update_receiver_client) = mpsc::channel(100);
        let (steady_update_sender_server, steady_update_receiver_client) = mpsc::channel(100);
        let uuid = generate_uuid();
        let local_connection = LocalConnection {
            fast_update_sender: fast_update_sender_server,
            steady_update_sender: steady_update_sender_server,
            fast_update_receiver: fast_update_receiver_server,
            steady_update_receiver: steady_update_receiver_server,
            consume_receiver_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
            uuid,
        };
        let local_connection_client_side = LocalConnectionClientSide {
            fast_update_sender: fast_update_sender_client,
            steady_update_sender: steady_update_sender_client,
            steady_sender_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
            fast_update_receiver: fast_update_receiver_client,
            steady_update_receiver: steady_update_receiver_client,
        };
        struct ThreadData {
            server: Server,
            connection: Arc<Mutex<LocalConnectionClientSide>>,
        }
        let connection = Arc::new(Mutex::new(local_connection_client_side));
        let thread_data = ThreadData {
            server: self.clone(),
            connection: connection.clone(),
        };
        tokio::spawn(async move {
            let thread_data = thread_data;
            let connection = Arc::new(Mutex::new(local_connection));
            thread_data.server.new_connection(Connection::Local(connection)).await;
        });
        connection
    }

    async fn get_connections_affected_from_position(&mut self, position: Vec3) -> Vec<Connection> {
        let mut connections_affected = Vec::new();
        match self.connections.clone() {
            Connections::Local(connections) => {
                let connections = connections.lock().await;
                for connection in connections.iter() {
                    // todo! check if connection is affected by position
                    connections_affected.push(Connection::Local(connection.clone()));
                }
            }
            Connections::Lan(listener, connections) => {
                let connections = connections.lock().await;
                for connection in connections.iter() {
                    // todo! check if connection is affected by position
                    connections_affected.push(Connection::Lan(listener.clone(), connection.clone()));
                }
            }
        }
        connections_affected
    }

    async fn get_all_connections(&mut self) -> Vec<Connection> {
        let mut connections_final = Vec::new();
        match self.connections.clone() {
            Connections::Local(connections) => {
                let connections = connections.lock().await;
                for connection in connections.iter() {
                    connections_final.push(Connection::Local(connection.clone()));
                }
            }
            Connections::Lan(listener, connections) => {
                let connections = connections.lock().await;
                for connection in connections.iter() {
                    connections_final.push(Connection::Lan(listener.clone(), connection.clone()));
                }
            }
        }
        connections_final
    }

    pub async fn handle_world_updates(&mut self, updates: Vec<WorldUpdate>) {
        let mut player_entity_movement_stack: HashMap<EntityId, Vec<(Vec3, Quaternion, Quaternion)>> = HashMap::new();
        for update in updates {
            match update {
                WorldUpdate::SetPosition(entity_id, vec3) => {
                    let connections = self.get_connections_affected_from_position(vec3).await;
                    for connection in connections {
                        self.send_fast_packet(&connection, FastPacket::ChangePosition(entity_id, vec3)).await;
                    }
                }
                WorldUpdate::SetRotation(entity_id, quat) => {
                    let connections = self.get_connections_affected_from_position(Vec3::new(0.0, 0.0, 0.0)).await;
                    for connection in connections {
                        self.send_fast_packet(&connection, FastPacket::ChangeRotation(entity_id, quat)).await;
                    }
                }
                WorldUpdate::SetScale(entity_id, vec3) => {
                    let connections = self.get_connections_affected_from_position(Vec3::new(0.0, 0.0, 0.0)).await;
                    for connection in connections {
                        self.send_fast_packet(&connection, FastPacket::ChangeScale(entity_id, vec3)).await;
                    }
                }
                WorldUpdate::InitEntity(entity_id, entity_data) => {
                    let connections = self.get_all_connections().await;
                    for connection in connections {
                        self.send_steady_packet(&connection, SteadyPacket::InitialiseEntity(entity_id, entity_data.clone())).await;
                    }
                }
                WorldUpdate::EntityNoLongerExists(entity_id) => {
                    let connections = self.get_all_connections().await;
                    for connection in connections {
                        self.send_steady_packet(&connection, SteadyPacket::RemoveEntity(entity_id)).await;
                    }
                }
                WorldUpdate::MovePlayerEntity(entity_id, position, rotation, head_rotation) => {
                    player_entity_movement_stack.entry(entity_id).or_insert(Vec::new()).push((position, rotation, head_rotation));
                }
            }
        }
        // the last player entity movement for each entity is the one that should be sent to the client
        for (entity_id, movement_stack) in player_entity_movement_stack {
            let movement = movement_stack.last().unwrap();
            let connections = self.get_connections_affected_from_position(movement.0).await;
            for connection in connections {
                self.send_fast_packet(&connection, FastPacket::PlayerMoved(entity_id, movement.0, movement.1, movement.2)).await;
            }
        }
    }

    // loop
    // check if there are any new connections to initialise
    // if there are, initialise them
    // if not, run the worldmachine
    pub async fn run(&mut self) {
        let mut compensation_delta = 0.0;
        loop {
            // do physics tick
            let mut worldmachine = self.worldmachine.lock().await;
            let last_physics_tick = worldmachine.last_physics_update;
            let current_time = std::time::Instant::now();
            let delta = (current_time - last_physics_tick).as_secs_f32();
            if delta > 0.0 {
                let res = worldmachine.physics.lock().unwrap().as_mut().unwrap().tick(delta + compensation_delta);
                if let Some(delta) = res {
                    compensation_delta += delta;
                } else {
                    compensation_delta = 0.0;
                    worldmachine.last_physics_update = current_time;
                }
                // do a player physics tick for each player
                {
                    let players = worldmachine.players.clone().unwrap();
                    let mut players = players.lock().await;
                    for (_uuid, player) in players.iter_mut() {
                        player.player.gravity_tick(player.entity_id, &mut worldmachine, delta).await;
                        player.player.snowball_cooldown -= delta;
                    }
                }
            }
            drop(worldmachine);

            // lock worldmachine
            {
                let mut worldmachine = self.worldmachine.lock().await;
                let updates = {
                    worldmachine.server_tick().await
                };
                drop(worldmachine);
                if let Some(updates) = updates {
                    self.handle_world_updates(updates).await;
                }
            }

            self.listen_for_lan_connections().await;
        }
    }
}
