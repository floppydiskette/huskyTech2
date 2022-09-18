use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::format;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use crate::server::{ConnectionUUID, FastPacket, FastPacketData, generate_uuid, SteadyPacket, SteadyPacketData};
use crate::server::connections::SteadyMessageQueue;

#[derive(Clone, Debug)]
pub struct LanConnection {
    pub steady_update: Arc<Mutex<TcpStream>>,
    pub steady_update_queue: Arc<Mutex<SteadyMessageQueue>>,
    last_fast_update_received: Arc<UnsafeCell<Option<FastPacketData>>>,
    pub remote_addr: SocketAddr,
    pub uuid: ConnectionUUID,
}

unsafe impl Send for LanConnection {}
unsafe impl Sync for LanConnection {}

#[derive(Clone, Debug)]
pub struct ClientLanConnection {
    pub steady_update: Arc<Mutex<TcpStream>>,
    fast_update: Arc<UdpSocket>,
    pub fast_update_next: Arc<UnsafeCell<Option<FastPacketLan>>>,
    pub steady_sender_queue: Arc<Mutex<SteadyMessageQueue>>,
    last_fast_update_received: Arc<UnsafeCell<Option<FastPacketData>>>,
    pub uuid: ConnectionUUID,
}

unsafe impl Send for ClientLanConnection {}
unsafe impl Sync for ClientLanConnection {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FastPacketLan {
    pub uuid: ConnectionUUID,
    pub socket_addr: Option<SocketAddr>,
    pub data: FastPacketPotentials,
}

unsafe impl Send for FastPacketLan {}
unsafe impl Sync for FastPacketLan {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FastPacketPotentials {
    FastPacket(FastPacketData),
    ConnectionHandshake(ConnectionHandshakePacket),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConnectionHandshakePacket {
    JoinRequest, // sent from client to server
    PleaseConnectUDPNow(ConnectionUUID), // sent from server to client
    IconnectedUDP(ConnectionUUID), // sent from client to server (over udp)
    YoureReady(ConnectionUUID), // sent from server to client (over udp)
}

#[derive(Clone, Debug)]
pub struct LanListener {
    pub fast_update: Arc<UdpSocket>,
    pub steady_update: Arc<Mutex<TcpListener>>,
    fast_update_map: Arc<Mutex<HashMap<ConnectionUUID, FastPacketLan>>>,
}

impl LanListener {
    pub async fn new(hostname: &str, tcp_port: u16, udp_port: u16) -> Self {
        let tcp_listener = TcpListener::bind(format!("{}:{}", hostname, tcp_port)).await.unwrap();
        let udp_socket = UdpSocket::bind(format!("{}:{}", hostname, udp_port)).await.unwrap();
        let fast_update_map = Arc::new(Mutex::new(HashMap::new()));

        let the_self = Self {
                fast_update: Arc::new(udp_socket),
                steady_update: Arc::new(Mutex::new(tcp_listener)),
                fast_update_map,
            };

        let the_clone = the_self.clone();
        tokio::spawn(async move {
            the_clone.udp_thread().await;
        });

        the_self
    }

    pub async fn check_for_new_connections(&self) -> Option<LanConnection> {
        let steady_update = self.steady_update.lock().await;

        let new_connection = steady_update.accept().await;
        if new_connection.is_err() {
            return None;
        }

        debug!("new connection");
        let (mut steady_update, _) = new_connection.unwrap();

        // check for first handshake packet
        let mut buf = [0; 1024];
        let len = steady_update.read(&mut buf).await.unwrap();
        let mut deserialiser = rmp_serde::Deserializer::new(&buf[..len]);
        let handshake_packet: ConnectionHandshakePacket = Deserialize::deserialize(&mut deserialiser).unwrap();
        if {
            match handshake_packet {
                ConnectionHandshakePacket::JoinRequest => true,
                _ => false,
            }
        } {
            debug!("got first handshake packet");
            let uuid_real = generate_uuid();
            let mut serialiser = rmp_serde::Serializer::new(Vec::new());
            let packet = ConnectionHandshakePacket::PleaseConnectUDPNow(uuid_real.clone());
            packet.serialize(&mut serialiser).unwrap();
            let data = serialiser.into_inner();
            let _ = steady_update.write(&data).await.unwrap();
            steady_update.flush().await.unwrap();
            debug!("sent second handshake packet");

            let mut peer_addr = None;

            // wait for udp connection
            loop {
                let packet = self.check_for_fast_update(&uuid_real).await;
                if let Some(packet) = packet {
                    debug!("got a packet, checking if it's the right one");
                    if let FastPacketPotentials::ConnectionHandshake(handshake_packet) = packet.data {
                        if let ConnectionHandshakePacket::IconnectedUDP(uuid) = handshake_packet {
                            if uuid == uuid_real {
                                peer_addr = packet.socket_addr;
                                break;
                            }
                        }
                    }
                }
            }

            debug!("got third handshake packet");

            let peer_addr = peer_addr.unwrap();

            // send the ready packet
            let mut serialiser = rmp_serde::Serializer::new(Vec::new());
            let packet = FastPacketLan {
                uuid: uuid_real.clone(),
                socket_addr: None,
                data: FastPacketPotentials::ConnectionHandshake(ConnectionHandshakePacket::YoureReady(uuid_real.clone())),
            };
            packet.serialize(&mut serialiser).unwrap();
            let data = serialiser.into_inner();

            let fast_update = &self.fast_update;

            fast_update.send_to(&data, peer_addr).await.unwrap();

            debug!("sent fourth handshake packet");

            // return the connection
            return Some(LanConnection {
                steady_update: Arc::new(Mutex::new(steady_update)),
                steady_update_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
                last_fast_update_received: Arc::new(UnsafeCell::new(None)),
                remote_addr: peer_addr,
                uuid: uuid_real,
            });
        }

        None
    }

    async fn send_fast_update(&self, connection: LanConnection, data: &[u8]) -> std::io::Result<usize> {
        debug!("sending fast update");
        self.fast_update.send_to(data, connection.remote_addr).await
    }

    async fn attempt_receive_fast_update(&self, data: &mut [u8]) -> Option<std::io::Result<usize>> {
        let fast_update = &self.fast_update;
        let attempt = fast_update.try_recv(data);
        if attempt.is_err() {
            None
        } else {
            Some(attempt)
        }
    }

    pub async fn block_receive_fast_update(&self, data: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        let fast_update = &self.fast_update;
        fast_update.recv_from(data).await
    }

    pub async fn udp_thread(&self) {
        let mut buf = [0; 1024];
        loop {
            let (len, addr) = self.block_receive_fast_update(&mut buf).await.unwrap();
            let mut deserialiser = rmp_serde::Deserializer::new(&buf[..len]);
            let mut packet: FastPacketLan = Deserialize::deserialize(&mut deserialiser).unwrap();
            let mut fast_update_map = self.fast_update_map.lock().await;
            packet.socket_addr = Some(addr);
            fast_update_map.insert(packet.uuid.clone(), packet);
        }
    }

    pub async fn check_for_fast_update(&self, uuid: &ConnectionUUID) -> Option<FastPacketLan> {
        let fast_update_map = self.fast_update_map.lock().await;
        if let Some(packet) = fast_update_map.get(uuid) {
            Some(packet.clone())
        } else {
            None
        }
    }
}

impl LanConnection {
    pub fn new(uuid: ConnectionUUID, fast_update: UdpSocket, steady_update: TcpStream) -> Self {
        let peer_addr = steady_update.peer_addr().unwrap();
        Self {
            steady_update: Arc::new(Mutex::new(steady_update)),
            steady_update_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
            last_fast_update_received: Arc::new(UnsafeCell::new(None)),
            remote_addr: peer_addr,
            uuid
        }
    }

    async fn send_steady_update(&self, data: &[u8]) -> std::io::Result<()> {
        self.steady_update.lock().await.write_all(data).await
    }

    async fn attempt_receive_steady_update(&self, data: &mut [u8]) -> Option<std::io::Result<usize>> {
        let steady_update = self.steady_update.lock().await;
        let attempt = steady_update.try_read(data);
        return if attempt.is_err() {
            None
        } else {
            Some(attempt)
        }
    }

    async fn block_receive_steady_update(&self, data: &mut [u8]) -> std::io::Result<usize> {
        self.steady_update.lock().await.read(data).await
    }

    pub async fn serialise_and_send_fast(&self, to_uuid: ConnectionUUID, listener: LanListener, packet: FastPacketData) -> std::io::Result<usize> {
        let mut buffer = Vec::new();
        let packet = FastPacketLan {
            uuid: to_uuid,
            socket_addr: None,
            data: FastPacketPotentials::FastPacket(packet),
        };
        let mut serialiser = rmp_serde::Serializer::new(&mut buffer);
        packet.serialize(&mut serialiser).unwrap();
        listener.send_fast_update(self.clone(), &buffer).await
    }

    pub async fn serialise_and_send_steady(&self, packet: SteadyPacketData) -> std::io::Result<()> {
        let mut buffer = Vec::new();
        let mut serialiser = rmp_serde::Serializer::new(&mut buffer);
        packet.serialize(&mut serialiser).unwrap();
        debug!("sending: {:?}", packet);
        self.send_steady_update(&buffer).await
    }

    pub async fn udp_listener_thread(&self, listener: LanListener) {
        loop {
            let mut buffer = [0; 2048];
            let attempt = listener.block_receive_fast_update(&mut buffer).await;
            if attempt.is_err() {
                warn!("failed to receive fast update: {:?}", attempt);
                continue;
            }
            let (attempt, addr) = attempt.unwrap();
            let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
            let packet = FastPacketLan::deserialize(&mut deserialiser).unwrap();
            if packet.uuid == self.uuid {
                debug!("got a packet for the right uuid");
                debug!("packet: {:?}", packet);
                if let FastPacketPotentials::FastPacket(packet) = packet.data {
                    let last_fast_update_received = unsafe { &mut *self.last_fast_update_received.get() };
                    *last_fast_update_received = Some(packet);
                }
            }
        }
    }

    pub async fn attempt_receive_fast_and_deserialise(&self) -> Option<FastPacketData> {
        let last_fast_update_received = unsafe { self.last_fast_update_received.get() };
        let unwrapped = unsafe { last_fast_update_received.as_ref().unwrap() };
        if let Some(packet) = unwrapped.clone() {
            unsafe { *last_fast_update_received = None };
            Some(packet)
        } else {
            None
        }
    }

    pub async fn attempt_receive_steady_and_deserialise(&self) -> Option<SteadyPacketData> {
        let mut buffer = [0; 2048];
        let attempt = self.attempt_receive_steady_update(&mut buffer).await;
        if attempt.is_none() {
            return None;
        }
        let attempt = attempt.unwrap();
        if attempt.is_err() {
            warn!("failed to receive steady update: {:?}", attempt);
            return None;
        }
        let attempt = attempt.unwrap();
        let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
        let packet = SteadyPacketData::deserialize(&mut deserialiser).unwrap();
        debug!("received steady packet: {:?}", packet);
        Some(packet)
    }

    pub async fn block_receive_steady_and_deserialise(&self) -> Option<SteadyPacketData> {
        let mut buffer = [0; 2048];
        let attempt = self.block_receive_steady_update(&mut buffer).await;
        if attempt.is_err() {
            warn!("failed to receive steady update: {:?}", attempt);
            return None;
        }
        let attempt = attempt.unwrap();
        let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
        let packet = SteadyPacketData::deserialize(&mut deserialiser).unwrap();
        debug!("received steady packet: {:?}", packet);
        Some(packet)
    }
}

impl ClientLanConnection {
    pub async fn connect(hostname: &str, tcp_port: u16, udp_port: u16) -> Option<Self> {
        let mut stream = TcpStream::connect(format!("{}:{}", hostname, tcp_port)).await.ok()?;
        debug!("connected to server");
        let mut serialiser = rmp_serde::Serializer::new(Vec::new());
        let packet = ConnectionHandshakePacket::JoinRequest;
        packet.serialize(&mut serialiser).unwrap();
        let data = serialiser.into_inner();
        stream.write_all(&data).await.ok()?;
        debug!("sent join request");
        let mut buffer = [0; 2048];
        let n = stream.read(&mut buffer).await.ok()?;
        let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..n]);
        let packet = ConnectionHandshakePacket::deserialize(&mut deserialiser).unwrap();
        if let ConnectionHandshakePacket::PleaseConnectUDPNow(uuid) = packet {
            debug!("received join response");
            debug!("our uuid is {}", uuid);
            let remote_addr: SocketAddr = format!("{}:{}", hostname, udp_port).parse().unwrap();
            let local_addr: SocketAddr = if remote_addr.is_ipv4() {
                "0.0.0.0:0"
            } else {
                "[::]:0"
            }.parse().unwrap();
            let socket = UdpSocket::bind(local_addr).await.ok()?;
            socket.connect(remote_addr).await.ok()?;
            debug!("connected to udp");

            let packet = FastPacketLan {
                uuid: uuid.clone(),
                socket_addr: Some(socket.local_addr().unwrap()),
                data: FastPacketPotentials::ConnectionHandshake(ConnectionHandshakePacket::IconnectedUDP(uuid.clone())),
            };
            let mut serialiser = rmp_serde::Serializer::new(Vec::new());
            packet.serialize(&mut serialiser).unwrap();
            let data = serialiser.into_inner();
            debug!("told the server we're ready to receive udp");
            socket.send(&data.clone()).await.ok()?;

            return Some(ClientLanConnection {
                steady_update: Arc::new(Mutex::new(stream)),
                fast_update: Arc::new(socket),
                fast_update_next: Arc::new(UnsafeCell::new(None)),
                steady_sender_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
                last_fast_update_received: Arc::new(UnsafeCell::new(None)),
                uuid,
            });
        }

        None
    }

    async fn send_steady_update(&self, data: &[u8]) -> std::io::Result<()> {
        self.steady_update.lock().await.write_all(data).await
    }

    async fn attempt_receive_steady_update(&self, data: &mut [u8]) -> Option<std::io::Result<usize>> {
        let attempt = self.steady_update.lock().await.try_read(data);
        if attempt.is_err() {
            None
        } else {
            Some(attempt)
        }
    }

    async fn block_receive_steady_update(&self, data: &mut [u8]) -> std::io::Result<usize> {
        self.steady_update.lock().await.read(data).await
    }

    async fn send_fast_update(&self, data: &[u8]) -> std::io::Result<usize> {
        self.fast_update.send(data).await
    }

    async fn attempt_receive_fast_update(&self, data: &mut [u8]) -> Option<std::io::Result<usize>> {
        let attempt = self.fast_update.try_recv(data);
        debug!("failed to receive fast update: {:?}", attempt);
        if attempt.is_err() {
            None
        } else {
            Some(attempt)
        }
    }

    async fn block_receive_fast_update(&self, data: &mut [u8]) -> std::io::Result<usize> {
        self.fast_update.recv(data).await
    }

    pub async fn send_fast_and_serialise(&self, packet: FastPacketData) -> std::io::Result<usize> {
        let mut serialiser = rmp_serde::Serializer::new(Vec::new());
        let packet = FastPacketLan {
            uuid: self.uuid.clone(),
            socket_addr: None,
            data: FastPacketPotentials::FastPacket(packet),
        };
        packet.serialize(&mut serialiser).unwrap();
        let data = serialiser.into_inner();
        self.send_fast_update(&data).await
    }

    pub async fn udp_listener_thread(&self) {
        let mut buffer = [0; 2048];
        loop {
            let attempt = self.block_receive_fast_update(&mut buffer).await;
            if attempt.is_err() {
                warn!("failed to receive fast update: {:?}", attempt);
                continue;
            }
            let attempt = attempt.unwrap();
            let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
            let packet = FastPacketLan::deserialize(&mut deserialiser).unwrap();
            debug!("received fast packet: {:?}", packet);
            if let FastPacketPotentials::FastPacket(packet) = packet.data {
                unsafe {
                    *self.last_fast_update_received.get() = Some(packet);
                }
            }
        }
    }


    pub async fn attempt_receive_fast_and_deserialise(&self) -> Option<FastPacketData> {
        let attempt = unsafe {
            self.last_fast_update_received.get()
        };
        let inner = unsafe { attempt.as_ref().take()? };
        let inner = inner.clone();
        if inner.is_none() {
            None
        } else {
            unsafe {
                *self.last_fast_update_received.get() = None;
            }
            Some(inner.unwrap())
        }
    }

    pub async fn send_steady_and_serialise(&self, packet: SteadyPacketData) -> std::io::Result<()> {
        let mut serialiser = rmp_serde::Serializer::new(Vec::new());
        packet.serialize(&mut serialiser).unwrap();
        let data = serialiser.into_inner();
        debug!("sending steady packet: {:?}", packet);
        self.send_steady_update(&data).await
    }

    pub async fn attempt_receive_steady_and_deserialise(&self) -> Option<SteadyPacketData> {
        let mut buffer = [0; 2048];
        let attempt = self.attempt_receive_steady_update(&mut buffer).await;
        if attempt.is_none() {
            return None;
        }
        let attempt = attempt.unwrap();
        if attempt.is_err() {
            warn!("failed to receive steady update: {:?}", attempt);
            return None;
        }
        let attempt = attempt.unwrap();
        let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
        let packet = SteadyPacketData::deserialize(&mut deserialiser).unwrap();

        Some(packet)
    }

    pub async fn block_receive_steady_and_deserialise(&self) -> SteadyPacketData {
        let mut buffer = [0; 2048];
        let attempt = self.block_receive_steady_update(&mut buffer).await;
        if attempt.is_err() {
            panic!("failed to receive steady update: {:?}", attempt);
        }
        let attempt = attempt.unwrap();
        let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
        let packet = SteadyPacketData::deserialize(&mut deserialiser).unwrap();
        packet
    }
}