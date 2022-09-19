use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::format;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use std::time::Duration;
use crate::server::{ConnectionUUID, FastPacket, FastPacketData, generate_uuid, SteadyPacket, SteadyPacketData};
use crate::server::connections::SteadyMessageQueue;

pub const FAST_QUEUE_LIMIT: usize = 4;
pub const FAKE_LAG: bool = true;
pub const FAKE_LAG_TIME: u64 = 600;

#[derive(Default, Debug)]
pub struct FastUpdateQueue<T> {
    pub queue: VecDeque<T>,
}

impl FastUpdateQueue<FastPacketData> {
    pub fn new(packet: Option<FastPacketData>) -> Self {
        let mut queue = VecDeque::new();
        if let Some(packet) = packet {
            queue.push_back(packet);
        }
        Self { queue }
    }

    pub fn push(&mut self, packet: FastPacketData) {
        if self.queue.len() >= FAST_QUEUE_LIMIT {
            self.queue.pop_front();
        }
        self.queue.push_back(packet);
    }

    pub fn pop(&mut self) -> Option<FastPacketData> {
        self.queue.pop_front()
    }
}

impl FastUpdateQueue<FastPacketLan> {
    pub fn new(packet: Option<FastPacketLan>) -> Self {
        let mut queue = VecDeque::new();
        if let Some(packet) = packet {
            queue.push_back(packet);
        }
        Self { queue }
    }

    pub fn push(&mut self, packet: FastPacketLan) {
        if self.queue.len() >= FAST_QUEUE_LIMIT {
            self.queue.pop_front();
        }
        self.queue.push_back(packet);
    }

    pub fn pop(&mut self) -> Option<FastPacketLan> {
        self.queue.pop_front()
    }
}

#[derive(Clone, Debug)]
pub struct LanConnection {
    pub steady_update: Arc<Mutex<TcpStream>>,
    pub steady_update_queue: Arc<Mutex<SteadyMessageQueue>>,
    pub steady_receive_queue: Arc<Mutex<(SteadyMessageQueue, bool)>>,
    pub remote_addr: SocketAddr,
    pub uuid: ConnectionUUID,
}

unsafe impl Send for LanConnection {}
unsafe impl Sync for LanConnection {}

#[derive(Clone, Debug)]
pub struct ClientLanConnection {
    pub steady_update: Arc<Mutex<TcpStream>>,
    fast_update: Arc<UdpSocket>,
    pub fast_update_queue: Arc<Mutex<FastUpdateQueue<FastPacketData>>>,
    pub steady_sender_queue: Arc<Mutex<SteadyMessageQueue>>,
    pub steady_receiver_queue: Arc<Mutex<SteadyMessageQueue>>,
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
    steady_update: Arc<Mutex<TcpListener>>,
    fast_update_map: Arc<Mutex<HashMap<ConnectionUUID, FastUpdateQueue<FastPacketLan>>>>,
}

unsafe impl Send for LanListener {}
unsafe impl Sync for LanListener {}

#[derive(Clone, Copy, Debug)]
pub enum ConnectionError {
    ConnectionClosed,
    ConnectionError,
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

    pub async fn poll_new_connection(&self) -> Option<TcpStream> {
        let mut steady_update = self.steady_update.lock().await;

        let new_connection = steady_update.accept().await;

        if new_connection.is_err() {
            return None;
        } else {
            return Some(new_connection.unwrap().0);
        }
    }

    pub async fn init_new_connection(&self, steady_update: TcpStream) -> Option<LanConnection> {

        debug!("new connection");
        let mut steady_update = steady_update;

        // check for first handshake packet
        let mut buf = [0; 1024];
        let len = steady_update.read(&mut buf).await.unwrap();
        let mut deserialiser = rmp_serde::Deserializer::new(&buf[..len]);
        let handshake_packet = ConnectionHandshakePacket::deserialize(&mut deserialiser);
        if let Err(_) = handshake_packet {
            warn!("handshake packet error");
            return None;
        }
        let handshake_packet = handshake_packet.unwrap();
        if matches!(handshake_packet, ConnectionHandshakePacket::JoinRequest) {
            debug!("got first handshake packet");
            let uuid_real = generate_uuid();
            let mut serialiser = rmp_serde::Serializer::new(Vec::new());
            let packet = ConnectionHandshakePacket::PleaseConnectUDPNow(uuid_real.clone());
            packet.serialize(&mut serialiser).unwrap();
            let data = serialiser.into_inner();
            let n = steady_update.write(&data).await;
            if let Err(_) = n {
                warn!("handshake packet error");
                return None;
            } else if n.unwrap() != data.len() {
                warn!("handshake packet error");
                return None;
            }
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

            let peer_addr = peer_addr;
            if peer_addr.is_none() {
                warn!("handshake packet error");
                return None;
            }
            let peer_addr = peer_addr.unwrap();

            // send the ready packet
            let mut serialiser = rmp_serde::Serializer::new(Vec::new());
            let packet = ConnectionHandshakePacket::YoureReady(uuid_real.clone());
            packet.serialize(&mut serialiser).unwrap();
            let data = serialiser.into_inner();

            let successful_write = steady_update.write_all(&data).await;
            if successful_write.is_err() {
                warn!("handshake packet error");
                return None;
            }

            debug!("sent fourth handshake packet");

            // return the connection
            return Some(LanConnection {
                steady_update: Arc::new(Mutex::new(steady_update)),
                steady_update_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
                steady_receive_queue: Arc::new(Mutex::new((SteadyMessageQueue::new(), true))),
                remote_addr: peer_addr,
                uuid: uuid_real,
            });
        }

        None
    }

    async fn send_fast_update(&self, connection: LanConnection, data: &[u8]) -> std::io::Result<usize> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        self.fast_update.send_to(data, connection.remote_addr).await
    }

    pub async fn udp_thread(&self) {
        loop {
            let mut buf = [0; 4096];
            let fast_update = &self.fast_update;
            if FAKE_LAG {
                tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
            }
            let (len, addr) =  fast_update.recv_from(&mut buf).await.expect("failed to receive from udp socket");
            let mut deserialiser = rmp_serde::Deserializer::new(&buf[..len]);
            let packet = FastPacketLan::deserialize(&mut deserialiser);
            if let Err(e) = packet {
                debug!("failed to deserialise packet: {:?}", e);
                continue;
            }
            let mut packet = packet.unwrap();
            let mut fast_update_map = self.fast_update_map.lock().await;
            packet.socket_addr = Some(addr);
            if let Some(updates) = fast_update_map.get_mut(&packet.uuid) {
                updates.push(packet);
            } else {
                let new_queue = FastUpdateQueue::<FastPacketLan>::new(Some(packet.clone()));
                fast_update_map.insert(packet.clone().uuid, new_queue);
            }
            drop(fast_update_map);
        }
    }

    pub async fn check_for_fast_update(&self, uuid: &ConnectionUUID) -> Option<FastPacketLan> {
        let mut fast_update_map = self.fast_update_map.lock().await;
        let update = fast_update_map.get_mut(uuid);
        if let Some(updates) = update {
            let update = updates.pop();
            if let Some(update) = update {
                drop(fast_update_map);
                return Some(update);
            }
        }
        drop(fast_update_map);
        None
    }
}

impl LanConnection {
    pub fn new(uuid: ConnectionUUID, fast_update: UdpSocket, steady_update: TcpStream) -> Self {
        let peer_addr = steady_update.peer_addr().unwrap();
        let the_self = Self {
            steady_update: Arc::new(Mutex::new(steady_update)),
            steady_update_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
            steady_receive_queue: Arc::new(Mutex::new((SteadyMessageQueue::new(), true))),
            remote_addr: peer_addr,
            uuid
        };
        let the_clone = the_self.clone();
        tokio::spawn(async move {
            the_clone.tcp_thread().await;
        });
        the_self
    }

    async fn send_steady_update(&self, data: &[u8]) -> std::io::Result<()> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        self.steady_update.lock().await.write_all(data).await
    }

    async fn block_receive_steady_update(&self, data: &mut [u8]) -> std::io::Result<usize> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
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
        self.send_steady_update(&buffer).await
    }

    pub async fn attempt_receive_fast_and_deserialise(&self, listener: LanListener) -> Option<FastPacketData> {
        let last_fast_update_received = listener.check_for_fast_update(&self.uuid).await;
        if let Some(last_fast_update_received) = last_fast_update_received {
            if let FastPacketPotentials::FastPacket(packet) = last_fast_update_received.data {
                return Some(packet);
            }
        }
        None
    }

    pub async fn attempt_receive_steady_and_deserialise(&self) -> Result<Option<SteadyPacketData>, ConnectionError> {
        let packet = self.steady_receive_queue.lock().await.0.pop();
        if let Some(packet) = packet {
            Ok(Some(packet))
        } else if !self.steady_receive_queue.lock().await.1 {
            Err(ConnectionError::ConnectionClosed)
        } else {
            Ok(None)
        }
    }

    async fn block_receive_steady_and_deserialise(&self) -> Result<Option<SteadyPacketData>, ConnectionError> {
        let mut buffer = [0; 2048];
        let attempt = self.block_receive_steady_update(&mut buffer).await;
        if attempt.is_err() {
            return Ok(None);
        }
        let attempt = attempt.unwrap();
        if attempt == 0 {
            // connection closed
            return Err(ConnectionError::ConnectionClosed);
        }
        let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
        let packet = SteadyPacketData::deserialize(&mut deserialiser);
        if let Err(e) = packet {
            warn!("failed to deserialise packet: {:?}", e);
            return Ok(None);
        }
        let packet = packet.unwrap();
        Ok(Some(packet))
    }

    pub async fn tcp_thread(&self) {
        loop {
            let packet = self.attempt_receive_steady_and_deserialise().await;
            if let Err(e) = packet {
                warn!("tcp thread failed: {:?}", e);
                self.steady_receive_queue.lock().await.1 = false;
                break;
            }
            let packet = packet.unwrap();
            if let Some(packet) = packet {
                let queue = &mut self.steady_receive_queue.lock().await.0;
                queue.push(packet);
            }
        }
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
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        stream.write_all(&data).await.ok()?;
        debug!("sent join request");
        let mut buffer = [0; 2048];
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
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
            if FAKE_LAG {
                tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
            }
            socket.send(&data.clone()).await.ok()?;

            // loop until we receive the YoureReady packet
            loop {
                let mut buffer = [0; 2048];
                if FAKE_LAG {
                    tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
                }
                let n = stream.read(&mut buffer).await.ok()?;
                let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..n]);
                let packet = ConnectionHandshakePacket::deserialize(&mut deserialiser).unwrap();
                if let ConnectionHandshakePacket::YoureReady(_) = packet {
                    debug!("received YoureReady packet");
                    break;
                }
            }

            return Some(ClientLanConnection {
                steady_update: Arc::new(Mutex::new(stream)),
                fast_update: Arc::new(socket),
                fast_update_queue: Arc::new(Mutex::new(FastUpdateQueue::<FastPacketData>::new(None))),
                steady_sender_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
                steady_receiver_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
                uuid,
            });
        }

        None
    }

    async fn send_steady_update(&self, data: &[u8]) -> std::io::Result<()> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        self.steady_update.lock().await.write_all(data).await
    }

    async fn attempt_receive_steady_update(&self, data: &mut [u8]) -> Option<std::io::Result<usize>> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        let attempt = self.steady_update.lock().await.try_read(data);
        if attempt.is_err() {
            None
        } else {
            Some(attempt)
        }
    }

    async fn block_receive_steady_update(&self, data: &mut [u8]) -> std::io::Result<usize> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        self.steady_update.lock().await.read(data).await
    }

    async fn send_fast_update(&self, data: &[u8]) -> std::io::Result<usize> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        self.fast_update.send(data).await
    }

    async fn attempt_receive_fast_update(&self, data: &mut [u8]) -> Option<std::io::Result<usize>> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
        let attempt = self.fast_update.try_recv(data);
        if attempt.is_err() {
            None
        } else {
            Some(attempt)
        }
    }

    async fn block_receive_fast_update(&self, data: &mut [u8]) -> std::io::Result<usize> {
        if FAKE_LAG {
            tokio::time::sleep(Duration::from_millis(FAKE_LAG_TIME)).await;
        }
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
            if let FastPacketPotentials::FastPacket(packet) = packet.data {
                self.fast_update_queue.lock().await.push(packet);
            }
        }
    }

    pub async fn tcp_listener_thread(&self) {
        let mut buffer = [0; 2048];
        loop {
            let attempt = self.attempt_receive_steady_update(&mut buffer).await;
            if attempt.is_none() {
                continue;
            }
            let attempt = attempt.unwrap();
            if attempt.is_err() {
                warn!("failed to receive steady update: {:?}", attempt);
                continue;
            }
            let attempt = attempt.unwrap();
            if attempt == 0 {
                warn!("received 0 bytes from steady update");
                break;
            }
            let mut deserialiser = rmp_serde::Deserializer::new(&buffer[..attempt]);
            let packet = SteadyPacketData::deserialize(&mut deserialiser).unwrap();
            self.steady_receiver_queue.lock().await.push(packet);
        }
    }

    pub async fn attempt_receive_fast_and_deserialise(&self) -> Option<FastPacketData> {
        let mut fast_update_queue = self.fast_update_queue.lock().await;
        let attempt = fast_update_queue.pop();
        drop(fast_update_queue);
        if attempt.is_none() {
            None
        } else {
            debug!("received fast update");
            Some(attempt.unwrap())
        }
    }

    pub async fn send_steady_and_serialise(&self, packet: SteadyPacketData) -> std::io::Result<()> {
        let mut serialiser = rmp_serde::Serializer::new(Vec::new());
        packet.serialize(&mut serialiser).unwrap();
        let data = serialiser.into_inner();
        self.send_steady_update(&data).await
    }

    pub async fn attempt_receive_steady_and_deserialise(&self) -> Option<SteadyPacketData> {
        let mut steady_receiver_queue = self.steady_receiver_queue.lock().await;
        let attempt = steady_receiver_queue.pop();
        drop(steady_receiver_queue);
        if attempt.is_none() {
            None
        } else {
            debug!("received steady update");
            Some(attempt.unwrap())
        }
    }
}