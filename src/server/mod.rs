use std::borrow::BorrowMut;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use gfx_maths::*;
use tokio::sync::{broadcast, mpsc, Mutex, watch};
use async_recursion::async_recursion;
use libsex::bindings::XConnectionWatchProc;
use serde::{Serialize, Deserialize};
use tokio::net::TcpStream;
use crate::physics::PhysicsSystem;
use crate::server::connections::SteadyMessageQueue;
use crate::server::lan::{ClientLanConnection, LanConnection, LanListener};
use crate::server::server_player::{ServerPlayer, ServerPlayerContainer};
use crate::worldmachine::{EntityId, WorldMachine, WorldUpdate};
use crate::worldmachine::ecs::Entity;
use crate::worldmachine::player::PlayerContainer;

pub mod connections;
pub mod server_player;
pub mod lan;

pub type PacketUUID = String;
pub type ConnectionUUID = String;

#[derive(Clone, Debug)]
pub enum Connection {
    Local(Arc<Mutex<LocalConnection>>),
    Lan(LanListener, Arc<Mutex<LanConnection>>),
}

#[derive(Clone)]
pub enum ConnectionClientside {
    Local(Arc<Mutex<LocalConnectionClientSide>>),
    Lan(Arc<Mutex<ClientLanConnection>>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FastPacket {
    ChangePosition(EntityId, Vec3),
    ChangeRotation(EntityId, Quaternion),
    ChangeScale(EntityId, Vec3),
    PlayerMove(ConnectionUUID, Vec3, Vec3, Quaternion, Quaternion, bool),
    // connection uuid, position, displacement_vector, rotation, head rotation, jumped
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
pub enum SteadyPacket {
    Consume(PacketUUID),
    SelfTest,
    KeepAlive,
    InitialiseEntity(EntityId, Entity),
    FinaliseMapLoad,
    InitialisePlayer(ConnectionUUID, String, Vec3, Quaternion, Vec3),
    // uuid, name, position, rotation, scale
    Message(String),
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
    pub steady_receiver_queue: Arc<Mutex<SteadyMessageQueue>>,
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
    Lan(LanListener, Arc<Mutex<Vec<Arc<Mutex<LanConnection>>>>>),
}

#[derive(Clone)]
pub struct Server {
    pub connections: Connections,
    pub connections_to_initialise: Arc<Mutex<Vec<usize>>>,
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
            connections_to_initialise: Arc::new(Mutex::new(Vec::new())),
            connections_incoming: Arc::new(Mutex::new(VecDeque::new())),
            worldmachine: Arc::new(Mutex::new(worldmachine)),
        }
    }

    pub async fn new_host_lan_server(map_name: &str, physics: PhysicsSystem, tcp_port: u16, udp_port: u16, hostname: &str) -> Self {
        let mut worldmachine = WorldMachine::default();
        worldmachine.initialise(physics, true);
        worldmachine.load_map(map_name).expect("failed to load map");

        worldmachine.players = Some(Arc::new(Mutex::new(HashMap::new())));

        let mut listener = LanListener::new(hostname, tcp_port, udp_port).await;

        let mut the_self = Self {
                connections: Connections::Lan(listener.clone(), Arc::new(Mutex::new(Vec::new()))),
                connections_to_initialise: Arc::new(Mutex::new(Vec::new())),
                connections_incoming: Arc::new(Mutex::new(VecDeque::new())),
                worldmachine: Arc::new(Mutex::new(worldmachine)),
            };
        let the_clone = the_self.clone();
        let listener_clone = listener.clone();
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
        if let Connections::Lan(listener_raw, connections_raw) = self.connections.clone() {
            let listener = listener_raw.clone();
            let mut connections_incoming = self.connections_incoming.clone();
            let mut connections_incoming = connections_incoming.lock().await;
            while let Some(connection) = connections_incoming.pop_front() {
                let connection = listener.clone().init_new_connection(connection).await.unwrap();
                let connection = Arc::new(Mutex::new(connection));
                let listener = listener_raw.clone();
                let connection_clone = connection.clone();
                let the_clone = self.clone();
                let the_listener_clone = listener_raw.clone();
                tokio::spawn(async move {
                    the_clone.new_connection(Connection::Lan(the_listener_clone.clone(), connection.clone())).await;
                });
            }
        }
    }

    async fn get_connection_uuid(&self, connection: &Connection) -> ConnectionUUID {
        match connection {
            Connection::Local(local_connection) => local_connection.lock().await.uuid.clone(),
            Connection::Lan(_, lan_connection) => lan_connection.lock().await.uuid.clone(),
        }
    }

    async unsafe fn send_steady_packet_unsafe(&self, connection: &Connection, packet: SteadyPacketData) -> bool {
        match connection.clone() {
            Connection::Local(connection) => {
                let mut connection_lock = connection.lock().await;
                connection_lock.steady_update_sender.send(packet.clone()).await.unwrap();
                debug!("sent packet to local connection");
                // wait for the packet to be consumed
                drop(connection_lock);
                loop {
                    let mut connection_lock = connection.lock().await;
                    let mut connection = Arc::new(Mutex::new(connection_lock.clone()));
                    self.get_received_steady_packets(&Connection::Local(connection.clone())).await;
                    let connection = connection.lock().await;
                    *connection_lock = connection.clone();
                    {
                        let mut queue = connection_lock.steady_receiver_queue.lock().await;
                        if let Some(packet_data) = queue.pop() {
                            if let SteadyPacket::Consume(uuid) = packet_data.packet.clone().unwrap() {
                                if uuid == packet.clone().uuid.unwrap() {
                                    debug!("packet consumed");
                                    break;
                                } else {
                                    // put the packet back in the queue
                                    queue.push(packet_data);
                                }
                            }
                        }
                    }
                }
            }
            Connection::Lan(_, connection) => {
                let mut connection_lock = connection.lock().await;
                loop {
                    let res = connection_lock.serialise_and_send_steady(packet.clone()).await;
                    if res.is_ok() {
                        break;
                    } else {
                        warn!("failed to send packet to lan connection, retrying");
                    }
                }
                debug!("sent packet to lan connection");
                debug!("uuid: {:?}", packet.clone().uuid);
                // wait for the packet to be consumed
                drop(connection_lock);
                loop {
                    let mut connection_lock = connection.lock().await;
                    let received_packet = connection_lock.block_receive_steady_and_deserialise().await;
                    if let Err(e) = received_packet {
                        warn!("failed to receive packet from lan connection: {:?}", e);
                        return false;
                    }
                    let received_packet = received_packet.unwrap();
                    if let Some(received_packet) = received_packet {
                        if let SteadyPacket::Consume(uuid) = received_packet.packet.clone().unwrap() {
                            if uuid == packet.clone().uuid.unwrap() {
                                debug!("packet consumed");
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
                    return self.send_steady_packet_unsafe(&Connection::Local(connection), packet_data).await;
                }
            }
            Connection::Lan(listener, connection) => {
                let uuid = generate_uuid();
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                unsafe {
                    return self.send_steady_packet_unsafe(&Connection::Lan(listener, connection), packet_data).await;
                }
            }
        }
    }

    async unsafe fn queue_received_steady_packet(&self, connection: &Connection, packet: SteadyPacket) {
        let uuid = generate_uuid();
        match connection.clone() {
            Connection::Local(connection) => {
                let connection = connection.lock().await;
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                let mut queue = connection.steady_receiver_queue.lock().await;
                queue.push(packet_data);
            }
            Connection::Lan(_, connection) => {
                let connection = connection.lock().await;
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                let mut queue = connection.steady_update_queue.lock().await;
                queue.push(packet_data);
            }
        }
    }

    pub async fn get_received_steady_packets(&self, connection: &Connection) -> bool {
        match connection.clone() {
            Connection::Local(connection_raw) => {
                let mut connection = connection_raw.lock().await;
                if !connection.steady_update_receiver.has_changed().ok().unwrap_or(false) {
                    return true;
                }
                let message = connection.steady_update_receiver.borrow_and_update().clone();
                if let Some(packet) = message.packet {
                    unsafe {
                        drop(connection);
                        self.queue_received_steady_packet(&Connection::Local(connection_raw.clone()), packet).await;
                    }
                }
            }
            Connection::Lan(listener, connection_raw) => {
                let connection = connection_raw.lock().await;
                let packet = connection.attempt_receive_steady_and_deserialise().await;
                if let Err(e) = packet {
                    warn!("failed to receive packet from lan connection: {:?}", e);
                    return false;
                }
                let packet = packet.unwrap();
                if let Some(packet) = packet {
                    unsafe {
                        drop(connection);
                        self.queue_received_steady_packet(&Connection::Lan(listener, connection_raw.clone()), packet.packet.unwrap()).await;
                    }
                }
            }
        }
        true
    }

    pub async fn send_fast_packet(&self, connection: &Connection, packet: FastPacket) {
        match connection.clone() {
            Connection::Local(connection) => {
                let mut connection = connection.lock().await;
                let packet_data = FastPacketData {
                    packet: Some(packet),
                };
                connection.fast_update_sender.send(packet_data).await.unwrap();
            }
            Connection::Lan(listener, connection) => {
                let mut connection = connection.lock().await;
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
                let connection = connection.lock().await;
                let packet = connection.attempt_receive_fast_and_deserialise(listener).await;
                if let Some(packet) = packet {
                    return Some(packet.packet.unwrap());
                }
            }
        }
        None
    }

    pub async fn begin_connection(&self, connection: Connection) -> bool {
        // for each entity in the worldmachine, send an initialise packet
        let world_clone = self.worldmachine.lock().await.world.clone();
        let physics = self.worldmachine.lock().await.physics.clone().unwrap();
        for entity in world_clone.entities.iter() {
            self.send_steady_packet(&connection, SteadyPacket::InitialiseEntity(entity.uid, entity.clone())).await;
        }
        let uuid = self.get_connection_uuid(&connection).await;

        let mut player = ServerPlayer::new(uuid.as_str(), "morbius", Vec3::new(0.0, 2.0, 0.0), Quaternion::identity(), Vec3::new(1.0, 1.0, 1.0));

        player.init(physics.clone());

        self.worldmachine.lock().await.players.as_mut().unwrap().lock().await.insert(uuid.clone(), ServerPlayerContainer {
            player: player.clone(),
            entity_id: None,
        });

        self.send_steady_packet(&connection, SteadyPacket::FinaliseMapLoad).await;
        return self.send_steady_packet(&connection, SteadyPacket::InitialisePlayer(player.uuid.clone(), player.name.clone(), player.get_position(), player.get_rotation(), player.scale)).await;
    }

    async fn handle_steady_packets(&self, connection: Connection) -> bool {
        match connection.clone() {
            Connection::Local(local_connection) => {
                let mut local_connection = local_connection.lock().await;
                if local_connection.steady_update_receiver.has_changed().unwrap_or(false) {
                    let steady_packet_data = local_connection.steady_update_receiver.borrow_and_update().clone();
                    if let Some(steady_packet) = steady_packet_data.packet {
                        match steady_packet {
                            SteadyPacket::KeepAlive => {
                                // do nothing
                            }
                            SteadyPacket::InitialiseEntity(uid, entity) => {
                                // client shouldn't be sending this
                                debug!("client sent initialise packet");
                            }

                            // client shouldn't be sending these
                            SteadyPacket::Consume(_) => {}
                            SteadyPacket::SelfTest => {}
                            SteadyPacket::InitialisePlayer(_, _, _, _, _) => {}
                            SteadyPacket::Message(_) => {}
                            SteadyPacket::FinaliseMapLoad => {}
                        }
                    }
                }
            }
            Connection::Lan(_, connection) => {
                let connection = connection.lock().await;
                let packet = connection.attempt_receive_steady_and_deserialise().await;
                if let Err(e) = packet {
                    debug!("error receiving steady packet: {:?}", e);
                    return false;
                }
                let packet = packet.unwrap();
                if let Some(packet) = packet {
                    match packet.packet.unwrap() {
                        SteadyPacket::KeepAlive => {
                            // do nothing
                        }
                        SteadyPacket::InitialiseEntity(uid, entity) => {
                            // client shouldn't be sending this
                            debug!("client sent initialise packet");
                        }

                        // client shouldn't be sending these
                        SteadyPacket::Consume(_) => {}
                        SteadyPacket::SelfTest => {}
                        SteadyPacket::InitialisePlayer(_, _, _, _, _) => {}
                        SteadyPacket::Message(_) => {}
                        SteadyPacket::FinaliseMapLoad => {}
                    }
                }
            }
        }
        // get steady packets
        self.get_received_steady_packets(&connection).await;
        true
    }

    async fn player_move(&self, connection: Connection, packet: FastPacket) {
        if let FastPacket::PlayerMove(uuid, position, displacement_vector, rotation, head_rotation, jumped) = packet {
            let mut worldmachine = self.worldmachine.clone();
            let mut worldmachine = worldmachine.lock().await;
            let mut players = worldmachine.players.clone();
            let mut players = players.as_mut().unwrap().lock().await;
            let player = players.get_mut(&uuid).unwrap();
            let success = player.player.attempt_position_change(position, displacement_vector, rotation, head_rotation, jumped, &mut worldmachine).await;
            if success {} else {
                let connection = connection.clone();
                self.send_fast_packet(&connection, FastPacket::PlayerFuckYouMoveHere(player.player.get_position())).await
            }
        }
    }

    async fn player_check_position(&self, connection: Connection, packet: FastPacket) {
        if let FastPacket::PlayerCheckPosition(uuid, position) = packet {
            let mut worldmachine = self.worldmachine.clone();
            let mut worldmachine = worldmachine.lock().await;
            let mut players = worldmachine.players.clone();
            let mut players = players.as_mut().unwrap().lock().await;
            let player = players.get_mut(&uuid).unwrap();
            let server_position = player.player.get_position();
            let success = server_position == position;
            if success {} else {
                self.send_fast_packet(&connection, FastPacket::PlayerFuckYouMoveHere(player.player.get_position())).await
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
                        }
                    }
                }
            }
            Connection::Lan(listener, lan_connection) => {
                let mut lan_connection = lan_connection.lock().await;
                let listener = listener.clone();
                let packet = lan_connection.attempt_receive_fast_and_deserialise(listener).await;
                drop(lan_connection);
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
                        FastPacket::PlayerJump(uuid) => {
                        }

                        // client shouldn't be sending these
                        FastPacket::ChangePosition(_, _) => {}
                        FastPacket::ChangeRotation(_, _) => {}
                        FastPacket::ChangeScale(_, _) => {}
                        FastPacket::PlayerFuckYouMoveHere(_) => {}
                        FastPacket::PlayerFuckYouSetRotation(_) => {}
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
                    let connection_index = match self.connections.clone() {
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
                    let connection_index = match self.connections.clone() {
                        Connections::Lan(_, connections) => {
                            let mut connections = connections.lock().await;
                            connections.push(lan_connection.clone());
                            connections.len() - 1
                        }
                        _ => {
                            panic!("assert_connection_type_allowed failed");
                        }
                    };
                    self.begin_connection(connection.clone()).await;
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
                        connections.retain(|x| !Arc::ptr_eq(x, &lan_connection));
                        debug!("connections: {:?}", connections.len());
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
            steady_receiver_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
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
            let mut thread_data = thread_data;
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
            }
        }
    }

    // loop
    // check if there are any new connections to initialise
    // if there are, initialise them
    // if not, run the worldmachine
    pub async fn run(&mut self) {
        loop {

            self.listen_for_lan_connections().await;

            // lock
            {
                let mut connections_to_initialise = self.connections_to_initialise.lock().await;
                if connections_to_initialise.len() > 0 {
                    let connection_index = connections_to_initialise.remove(0);
                    let connection = self.connections.clone();
                    match connection {
                        Connections::Local(connections) => {
                            let connections = connections.lock().await;
                            let connection = connections.get(connection_index).unwrap().clone();
                            struct ThreadData {
                                server: Server,
                                connection: Arc<Mutex<LocalConnection>>,
                            }
                            let thread_data = ThreadData {
                                server: self.clone(),
                                connection,
                            };
                            tokio::spawn(async move {
                                let mut thread_data = thread_data;
                                thread_data.server.begin_connection(Connection::Local(thread_data.connection.clone())).await;
                                thread_data.server.handle_connection(Connection::Local(thread_data.connection)).await;
                            });
                        }
                        Connections::Lan(listener, connections) => {
                            let connections = connections.lock().await;
                            let connection = connections.get(connection_index).unwrap().clone();
                            struct ThreadData {
                                server: Server,
                                connection: Arc<Mutex<LanConnection>>,
                            }
                            let thread_data = ThreadData {
                                server: self.clone(),
                                connection,
                            };
                            tokio::spawn(async move {
                                let mut thread_data = thread_data;
                                thread_data.server.begin_connection(Connection::Lan(listener.clone(), thread_data.connection.clone())).await;
                                thread_data.server.handle_connection(Connection::Lan(listener.clone(), thread_data.connection)).await;
                            });
                        }
                    }
                }
            }
            // unlock
            let updates = self.worldmachine.lock().await.server_tick().await;
            if let Some(updates) = updates {
                self.handle_world_updates(updates).await;
            }

            // do physics tick
            let mut worldmachine = self.worldmachine.lock().await;
            let last_physics_tick = worldmachine.last_physics_update;
            let delta = last_physics_tick.elapsed().as_secs_f32();
            if delta > 0.1 {
                // do a player physics tick for each player
                {
                    let players = worldmachine.players.clone().unwrap();
                    let mut players = players.lock().await;
                    for (_uuid, player) in players.iter_mut() {
                        player.player.gravity_tick();
                    }
                }
                worldmachine.physics.as_mut().unwrap().tick(delta);
                worldmachine.last_physics_update = Instant::now();
            }
        }
    }
}
