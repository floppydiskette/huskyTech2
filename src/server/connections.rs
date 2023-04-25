use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use crate::server::SteadyPacketData;

#[derive(Clone, Debug)]
pub struct SteadyMessageQueue {
    receiver: Arc<Mutex<mpsc::Receiver<SteadyPacketData>>>,
    sender: mpsc::Sender<SteadyPacketData>,
}

impl SteadyMessageQueue {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(100);
        Self {
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
        }
    }

    pub async fn pop(&self) -> Option<SteadyPacketData> {
        let mut receiver = self.receiver.lock().await;
        let peek = receiver.try_recv();
        if let Ok(packet) = peek {
            Some(packet)
        } else {
            None
        }
    }

    pub fn push(&self, packet: SteadyPacketData) {
        let _ = self.sender.try_send(packet);
    }
}