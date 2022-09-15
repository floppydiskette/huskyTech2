use std::future::Future;
use std::sync::Arc;
use std::thread;
use gfx_maths::*;
use tokio::sync::{broadcast, mpsc, Mutex, watch};
use async_recursion::async_recursion;
use crate::physics::PhysicsSystem;
use crate::worldmachine::{EntityId, WorldMachine};
use crate::worldmachine::ecs::Entity;

pub type PacketUUID = String;

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
}

#[derive(Clone, Debug)]
pub struct FastPacketData {
    pub packet: Option<FastPacket>,
}

#[derive(Clone, Debug)]
pub enum SteadyPacket {
    Consume(PacketUUID),
    KeepAlive,
    InitialiseEntity(EntityId, Entity),
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
    pub steady_update_receiver: watch::Receiver<SteadyPacketData>,
}

pub struct LocalConnectionClientSide {
    pub fast_update_sender: watch::Sender<FastPacketData>,
    pub steady_update_sender: watch::Sender<SteadyPacketData>,
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
    pub connections_to_initialise: Arc<Mutex<Vec<usize>>>,
    pub worldmachine: Arc<Mutex<WorldMachine>>,
}

fn generate_uuid() -> PacketUUID {
    uuid::Uuid::new_v4().to_string()
}

impl Server {
    pub fn new() -> Self {
        let mut physics = PhysicsSystem::init();
        let mut worldmachine = WorldMachine::default();
        worldmachine.initialise(physics);

        info!("server started");

        Self {
            connections: Connections::Local(Arc::new(Mutex::new(Vec::new()))),
            connections_to_initialise: Arc::new(Mutex::new(Vec::new())),
            worldmachine: Arc::new(Mutex::new(worldmachine)),
        }
    }

    #[async_recursion]
    pub async fn send_steady_packet(&mut self, connection_index: usize, packet: SteadyPacket) {
        let uuid = generate_uuid();
        match self.connections.clone() {
            Connections::Local(connections) => {
                let connections = connections.lock().await;
                let connection = connections.get(connection_index).unwrap().clone();
                let packet_data = SteadyPacketData {
                    packet: Some(packet.clone()),
                    uuid: Some(uuid.clone()),
                };
                connection.steady_update_sender.send(packet_data).await.unwrap();
                // wait for the packet to be consumed
                loop {
                    let packet_data = connection.steady_update_receiver.borrow().clone();
                    if let Some(SteadyPacket::Consume(consumed_uuid)) = packet_data.packet {
                        if consumed_uuid == uuid {
                            break;
                        }
                    } else {
                        // send the packet back to the receiver
                        self.send_steady_packet(connection_index, packet.clone()).await;
                    }
                }
            }
        }
    }

    pub async fn begin_connection(&mut self, connection_index: usize) {
        // for each entity in the worldmachine, send an initialise packet
        let world_clone = self.worldmachine.lock().await.world.clone();
        for entity in world_clone.entities.iter() {
            let packet = SteadyPacket::InitialiseEntity(entity.uid, entity.clone());
            self.send_steady_packet(connection_index, packet);
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
                    self.begin_connection(connection_index);
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
        let local_connection = LocalConnection {
            fast_update_sender: fast_update_sender_server,
            steady_update_sender: steady_update_sender_server,
            fast_update_receiver: fast_update_receiver_server,
            steady_update_receiver: steady_update_receiver_server,
        };
        let local_connection_client_side = LocalConnectionClientSide {
            fast_update_sender: fast_update_sender_client,
            steady_update_sender: steady_update_sender_client,
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
                                thread_data.server.begin_connection(thread_data.connection_index);
                                thread_data.server.handle_connection(Connection::Local(thread_data.connection)).await;
                            });
                        }
                    }
                }
            }
            // unlock
        }
    }
}