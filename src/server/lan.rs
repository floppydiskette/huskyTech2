use std::cell::UnsafeCell;
use halfbrown::HashMap;
use std::collections::{VecDeque};
use std::fmt::format;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream, UdpSocket};
use tokio::sync::{mpsc};
use mutex_timeouts::tokio::MutexWithTimeoutAuto as Mutex;
use serde::{Serialize, Deserialize};
use std::net::{SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use bytes::{Bytes, BytesMut};
use fyrox_sound::futures::{SinkExt, TryStreamExt};
use tokio::sync::mpsc::error::SendError;
use tokio::time::Instant;
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Decoder, Encoder, Framed, LengthDelimitedCodec};
use crate::server::{ConnectionUUID, FastPacket, FastPacketData, generate_uuid, PacketUUID, SteadyPacket, SteadyPacketData};
use crate::server::connections::SteadyMessageQueue;

pub const FAST_QUEUE_LIMIT: usize = 4;
pub const FAKE_LAG: bool = false;
pub const FAKE_LAG_TIME: u64 = 10;

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

#[derive(Clone)]
pub struct LanConnection {
    pub steady_update: mpsc::Sender<SteadyPacketData>,
    pub steady_receiver_passthrough: Arc<Mutex<Option<mpsc::Receiver<SteadyPacketData>>>>,
    pub is_connected: Arc<AtomicBool>,
    pub remote_addr: SocketAddr,
    pub uuid: ConnectionUUID,
    pub last_successful_ping: Arc<AtomicU64>,
}

unsafe impl Send for LanConnection {}

unsafe impl Sync for LanConnection {}

#[derive(Clone)]
pub struct ClientLanConnection {
    fast_update: Arc<UdpSocket>,
    pub fast_update_queue: Arc<Mutex<FastUpdateQueue<FastPacketData>>>,
    pub steady_sender_queue: mpsc::Sender<SteadyPacketData>,
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
    JoinRequest,
    // sent from client to server
    PleaseConnectUDPNow(ConnectionUUID),
    // sent from server to client
    IconnectedUDP(ConnectionUUID),
    // sent from client to server (over udp)
    YoureReady(ConnectionUUID), // sent from server to client (over udp)
}

#[derive(Clone)]
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
        let mut reader = Framed::new(steady_update, LengthDelimitedCodec::new());

        // check for first handshake packet
        let buffer = StreamExt::next(&mut reader).await.expect("failed to read result from server").expect("error reading result from server");
        let mut deserialiser = rmp_serde::Deserializer::new(buffer.as_ref());
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
            let n = reader.send(Bytes::from(data)).await;
            if let Err(_) = n {
                warn!("handshake packet error");
                return None;
            }
            debug!("sent second handshake packet");

            let mut peer_addr = None;

            // wait for udp connection
            let starting_time = Instant::now();
            const TIMEOUT_SECS: u64 = 20;
            const RETRY_SECS: u64 = 5;
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

                // every RETRY_SECS seconds, resend the packet
                if starting_time.elapsed().as_secs() % RETRY_SECS == 0 {
                    let mut serialiser = rmp_serde::Serializer::new(Vec::new());
                    let packet = ConnectionHandshakePacket::PleaseConnectUDPNow(uuid_real.clone());
                    packet.serialize(&mut serialiser).unwrap();
                    let data = serialiser.into_inner();
                    let n = reader.send(Bytes::from(data)).await;
                    if let Err(_) = n {
                        warn!("handshake packet error");
                        return None;
                    }
                    debug!("resent second handshake packet");
                }

                if starting_time.elapsed().as_secs() > TIMEOUT_SECS {
                    warn!("handshake packet error: timed out");
                    return None;
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

            let successful_write = reader.send(Bytes::from(data)).await;
            if successful_write.is_err() {
                warn!("handshake packet error");
                return None;
            }

            debug!("sent fourth handshake packet");

            // return the connection
            //return Some(LanConnection {
            //    steady_update: Arc::new(Mutex::new(steady_update)),
            //    steady_update_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
            //    steady_receive_queue: Arc::new(Mutex::new((SteadyMessageQueue::new(), true))),
            //    remote_addr: peer_addr,
            //    uuid: uuid_real,
            //});
            return Some(LanConnection::new(uuid_real, reader, peer_addr));
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
            let (len, addr) = fast_update.recv_from(&mut buf).await.expect("failed to receive from udp socket");
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
    pub fn new(uuid: ConnectionUUID, steady_update: Framed<TcpStream, LengthDelimitedCodec>, peer_addr: SocketAddr) -> Self {
        let (steady_sender_to_client, steady_receiver_at_tcpthread) = mpsc::channel(100);
        let (steady_sender_at_tcpthread, steady_receiver_from_tcpthread) = mpsc::channel(100);
        let the_self = Self {
            steady_update: steady_sender_to_client,
            steady_receiver_passthrough: Arc::new(Mutex::new(Some(steady_receiver_from_tcpthread))),
            is_connected: Arc::new(AtomicBool::new(true)),
            remote_addr: peer_addr,
            uuid,
            last_successful_ping: Arc::new(AtomicU64::new(0)),
        };
        let the_clone = the_self.clone();
        tokio::spawn(async move {
            the_clone.tcp_thread(steady_sender_at_tcpthread, steady_receiver_at_tcpthread, steady_update).await;
        });
        the_self
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

    pub async fn serialise_and_send_steady(&self, packet: SteadyPacketData) -> Result<(), SendError<SteadyPacketData>> {
        self.steady_update.send(packet).await
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

    pub async fn attempt_receive_steady_and_deserialise(&self, steady_receiver: &mut mpsc::Receiver<SteadyPacketData>) -> Result<Option<SteadyPacketData>, ConnectionError> {
        let packet = steady_receiver.recv().await;
        if let Some(packet) = packet {
            Ok(Some(packet))
        } else if !self.is_connected.load(Ordering::Relaxed) {
            Err(ConnectionError::ConnectionClosed)
        } else {
            Ok(None)
        }
    }

    // sender receives from client and sends to other threads, receiver receives from other threads and sends to client
    pub async fn tcp_thread(&self, sender: mpsc::Sender<SteadyPacketData>, mut receiver: mpsc::Receiver<SteadyPacketData>, mut reader: Framed<TcpStream, LengthDelimitedCodec>) {
        loop {
            tokio::select! {
                attempt = StreamExt::next(&mut reader) => {
                    if let Some(packet) = attempt {
                        if let Ok(packet) = packet {
                            let mut deserialiser = rmp_serde::Deserializer::new(&packet[..]);
                            let packet = SteadyPacketData::deserialize(&mut deserialiser);
                            if let Ok(packet) = packet {
                                debug!("received steady packet: {:?}", packet);
                                let uuid = packet.uuid.clone();
                                sender.send(packet).await.unwrap();
                            }
                        }
                    } else {
                        // connection closed
                        self.is_connected.store(false, Ordering::Relaxed);
                        break;
                    }
                }
                attempt = receiver.recv() => {
                    if let Some(packet) = attempt {
                        debug!("sending steady packet: {:?}", packet);
                        let mut buffer = Vec::new();
                        let mut serialiser = rmp_serde::Serializer::new(&mut buffer);
                        packet.serialize(&mut serialiser).unwrap();
                        reader.send(Bytes::from(buffer)).await.unwrap();
                    }
                }
            }
        }
    }
}

impl ClientLanConnection {
    pub async fn connect(hostname: &str, tcp_port: u16, udp_port: u16) -> Option<(Self, Framed<TcpStream, LengthDelimitedCodec>, mpsc::Receiver<SteadyPacketData>)> {
        let stream = TcpStream::connect(format!("{}:{}", hostname, tcp_port)).await.ok()?;
        let mut reader = Framed::new(stream, LengthDelimitedCodec::new());
        debug!("connected to server");
        let mut serialiser = rmp_serde::Serializer::new(Vec::new());
        let packet = ConnectionHandshakePacket::JoinRequest;
        packet.serialize(&mut serialiser).unwrap();
        let data = serialiser.into_inner();
        reader.send(Bytes::from(data)).await.ok()?;
        debug!("sent join request");
        let buffer = StreamExt::next(&mut reader).await.expect("failed to read result from server").expect("error reading result from server");
        let mut deserialiser = rmp_serde::Deserializer::new(buffer.as_ref());
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

            // loop until we receive the YoureReady packet
            loop {
                let buffer = StreamExt::next(&mut reader).await.expect("failed to read result from server").expect("error reading result from server");
                let mut deserialiser = rmp_serde::Deserializer::new(buffer.as_ref());
                let packet = ConnectionHandshakePacket::deserialize(&mut deserialiser).unwrap();
                if let ConnectionHandshakePacket::YoureReady(_) = packet {
                    debug!("received YoureReady packet");
                    break;
                }

                if let ConnectionHandshakePacket::PleaseConnectUDPNow(_) = packet {
                    // server didn't get our IConnectedUDP packet, send it again
                    let packet = FastPacketLan {
                        uuid: uuid.clone(),
                        socket_addr: Some(socket.local_addr().unwrap()),
                        data: FastPacketPotentials::ConnectionHandshake(ConnectionHandshakePacket::IconnectedUDP(uuid.clone())),
                    };
                    let mut serialiser = rmp_serde::Serializer::new(Vec::new());
                    packet.serialize(&mut serialiser).unwrap();
                    let data = serialiser.into_inner();
                    debug!("told the server we're ready to receive udp (again)");
                    socket.send(&data.clone()).await.ok()?;
                }
            }

            let (sender, receiver) = mpsc::channel(100);

            return Some((ClientLanConnection {
                fast_update: Arc::new(socket),
                fast_update_queue: Arc::new(Mutex::new(FastUpdateQueue::<FastPacketData>::new(None))),
                steady_sender_queue: sender,
                steady_receiver_queue: Arc::new(Mutex::new(SteadyMessageQueue::new())),
                uuid,
            }, reader, receiver));
        }

        None
    }

    async fn send_fast_update(&self, data: &[u8]) -> std::io::Result<usize> {
        self.fast_update.send(data).await
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
            let packet = FastPacketLan::deserialize(&mut deserialiser);
            if packet.is_err() {
                warn!("failed to deserialise fast update: {:?}", packet);
                continue;
            }
            let packet = packet.unwrap();
            if let FastPacketPotentials::FastPacket(packet) = packet.data {
                self.fast_update_queue.lock().await.push(packet);
            }
        }
    }

    pub async fn tcp_listener_thread(&self, mut reader: Framed<TcpStream, LengthDelimitedCodec>, mut receiver: mpsc::Receiver<SteadyPacketData>) {
        loop {
            tokio::select! {
                attempt = StreamExt::next(&mut reader) => {
                    if let Some(attempt) = attempt {
                        if let Ok(attempt) = attempt {
                            let mut deserialiser = rmp_serde::Deserializer::new(attempt.as_ref());
                            let packet = SteadyPacketData::deserialize(&mut deserialiser);
                            if packet.is_err() {
                                warn!("failed to deserialise steady update: {:?}", packet);
                            } else {
                                let packet = packet.unwrap();
                                let uuid = packet.uuid.clone();
                                self.steady_receiver_queue.lock().await.push(packet);
                            }
                        }
                    } else {
                        error!("connection closed");
                        break;
                    }
                }
                attempt = receiver.recv() => {
                    if let Some(attempt) = attempt {
                        debug!("sending steady update: {:?}", attempt);

                        let mut buffer = Vec::new();
                        let mut serialiser = rmp_serde::Serializer::new(&mut buffer);
                        attempt.serialize(&mut serialiser).unwrap();
                        let attempt = reader.send(Bytes::from(buffer)).await;
                        if attempt.is_err() {
                            warn!("failed to send steady update: {:?}", attempt);
                        }
                    }
                }
            }
        }
    }

    pub async fn attempt_receive_fast_and_deserialise(&self) -> Option<FastPacketData> {
        let mut fast_update_queue = self.fast_update_queue.lock().await;
        let attempt = fast_update_queue.pop();
        drop(fast_update_queue);
        if attempt.is_none() {
            None
        } else {
            Some(attempt.unwrap())
        }
    }

    pub async fn send_steady_and_serialise(&self, packet: SteadyPacketData) -> std::io::Result<()> {
        self.steady_sender_queue.send(packet).await.map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "failed to send steady packet"))
    }

    pub async fn attempt_receive_steady_and_deserialise(&self) -> Option<SteadyPacketData> {
        let mut steady_receiver_queue = self.steady_receiver_queue.lock().await;
        let attempt = steady_receiver_queue.pop().await;
        drop(steady_receiver_queue);
        if attempt.is_none() {
            None
        } else {
            Some(attempt.unwrap())
        }
    }
}