use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::thread;
use gfx_maths::*;
use tokio::sync::{broadcast, mpsc, Mutex, watch};
use async_recursion::async_recursion;
use libsex::bindings::XConnectionWatchProc;
use crate::physics::PhysicsSystem;
use crate::server::connections::SteadyMessageQueue;
use crate::worldmachine::{EntityId, WorldMachine, WorldUpdate};
use crate::worldmachine::ecs::Entity;
use crate::worldmachine::player::PlayerContainer;

pub mod connections;

pub type PacketUUID = String;
pub type ConnectionUUID = String;

#[derive(Clone, Debug)]
pub enum Connection {
    Local(LocalConnection),
}

pub enum ConnectionClientside {
    Local(LocalConnectionClientSide),
}

#[derive(Clone, Debug)]
pub enum FastPacket {
    ChangePosition(EntityId, Vec3),
    ChangeRotation(EntityId, Quaternion),
    ChangeScale(EntityId, Vec3),
}

#[derive(Clone, Debug)]
pub struct FastPacketData {
    pub packet: Option<FastPacket>,
}

#[derive(Clone, Debug)]
pub enum SteadyPacket {
    Consume(PacketUUID),
    SelfTest,
    KeepAlive,
    InitialiseEntity(EntityId, Entity),
    Message(String),
}

#[derive(Clone, Debug)]
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
    Local(Arc<Mutex<Vec<LocalConnection>>>),
}

#[derive(Clone)]
pub struct Server {
    pub connections: Connections,
    players: Arc<Mutex<HashMap<Connection, PlayerContainer>>>,
    pub connections_to_initialise: Arc<Mutex<Vec<usize>>>,
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

        info!("server started");

        Self {
            connections: Connections::Local(Arc::new(Mutex::new(Vec::new()))),
            players: Arc::new(Mutex::new(HashMap::new())),
            connections_to_initialise: Arc::new(Mutex::new(Vec::new())),
            worldmachine: Arc::new(Mutex::new(worldmachine)),
        }
    }

    #[async_recursion]
    async unsafe fn send_steady_packet_unsafe(&mut self, connection: &Connection, packet: SteadyPacketData) {
        match connection.clone() {
            Connection::Local(connection) => {
                connection.steady_update_sender.send(packet.clone()).await.unwrap();
                debug!("sent packet to local connection");
                // wait for the packet to be consumed
                loop {
                    self.get_received_steady_packets(&Connection::Local(connection.clone())).await;
                    let mut queue = connection.steady_receiver_queue.lock().await;
                    if let Some(packet_data) = queue.pop() {
                        if let SteadyPacket::Consume(uuid) = packet_data.packet.clone().unwrap() {
                            if uuid == packet.clone().uuid.unwrap() {
                                debug!("packet consumed");
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn send_steady_packet(&mut self, connection: &Connection, packet: SteadyPacket) {
        match connection.clone() {
            Connection::Local(connection) => {
                let uuid = generate_uuid();
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                unsafe {
                    self.send_steady_packet_unsafe(&Connection::Local(connection), packet_data).await;
                }
            }
        }
    }

    async unsafe fn queue_received_steady_packet(&mut self, connection: &Connection, packet: SteadyPacket) {
        let uuid = generate_uuid();
        match connection.clone() {
            Connection::Local(connection) => {
                let packet_data = SteadyPacketData {
                    packet: Some(packet),
                    uuid: Some(uuid.clone()),
                };
                let mut queue = connection.steady_receiver_queue.lock().await;
                queue.push(packet_data);
            }
        }
    }

    pub async fn get_received_steady_packets(&mut self, connection: &Connection) {
        match connection.clone() {
            Connection::Local(connection) => {
                if !connection.steady_update_receiver.has_changed().ok().unwrap_or(false) {
                    return;
                }
                let message = connection.steady_update_receiver.borrow().clone();
                if let Some(packet) = message.packet {
                    unsafe {
                        self.queue_received_steady_packet(&Connection::Local(connection.clone()), packet).await;
                    }
                }
            }
        }
    }

    pub async fn send_fast_packet(&mut self, connection: &Connection, packet: FastPacket) {
        match connection.clone() {
            Connection::Local(connection) => {
                let packet_data = FastPacketData {
                    packet: Some(packet),
                };
                connection.fast_update_sender.send(packet_data).await.unwrap();
            }
        }
    }

    pub async fn try_receive_fast_packet(&mut self, connection: &Connection) -> Option<FastPacket> {
        match connection.clone() {
            Connection::Local(connection) => {
                if !connection.fast_update_receiver.has_changed().ok().unwrap_or(false) {
                    return None;
                }
                let message = connection.fast_update_receiver.borrow().clone();
                if let Some(packet) = message.packet {
                    return Some(packet);
                }
            }
        }
        None
    }

    pub async fn begin_connection(&mut self, connection: Connection) {
        // for each entity in the worldmachine, send an initialise packet
        let world_clone = self.worldmachine.lock().await.world.clone();
        for entity in world_clone.entities.iter() {
            self.send_steady_packet(&connection, SteadyPacket::InitialiseEntity(entity.uid, entity.clone())).await;
        }
    }

    pub async fn handle_connection(&mut self, connection: Connection) {
        loop {
            match connection.clone() {
                Connection::Local(local_connection) => {
                    let steady_packet_data = local_connection.steady_update_receiver.borrow().clone();
                    if let Some(steady_packet) = steady_packet_data.packet {
                        match steady_packet {
                            SteadyPacket::KeepAlive => {
                                // do nothing
                            }
                            SteadyPacket::InitialiseEntity(uid, entity) => {
                                // client shouldn't be sending this
                                debug!("client sent initialise packet");
                            }
                            _ => {}
                        }
                    }

                    // get steady packets
                    self.get_received_steady_packets(&Connection::Local(local_connection.clone())).await;
                }
            }
        }
    }

    async fn assert_connection_type_allowed(&self, connection: Connection) -> bool {
        match connection {
            Connection::Local(_) => {
                matches!(self.connections.clone(), Connections::Local(_))
            }
        }
    }

    async fn new_connection(&mut self, connection: Connection) {
        if self.assert_connection_type_allowed(connection.clone()).await {
            match connection.clone() {
                Connection::Local(local_connection) => {
                    let connection_index = match self.connections.clone() {
                        Connections::Local(connections) => {
                            let mut connections = connections.lock().await;
                            connections.push(local_connection);
                            connections.len() - 1
                        }
                    };
                    self.begin_connection(connection.clone()).await;
                    self.handle_connection(connection).await;
                }
            }
        }
    }

    pub async fn join_local_server(&mut self) -> LocalConnectionClientSide {
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
            connection: LocalConnection,
        }
        let thread_data = ThreadData {
            server: self.clone(),
            connection: local_connection.clone(),
        };
        tokio::spawn(async move {
            let mut thread_data = thread_data;
            thread_data.server.new_connection(Connection::Local(local_connection)).await;
        });
        local_connection_client_side
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
                                connection_index: usize,
                                connection: LocalConnection,
                            }
                            let thread_data = ThreadData {
                                server: self.clone(),
                                connection_index,
                                connection,
                            };
                            tokio::spawn(async move {
                                let mut thread_data = thread_data;
                                thread_data.server.begin_connection(Connection::Local(thread_data.connection.clone())).await;
                                thread_data.server.handle_connection(Connection::Local(thread_data.connection)).await;
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
        }
    }
}
