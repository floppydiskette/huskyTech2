use std::collections::VecDeque;
use crate::server::SteadyPacketData;

#[derive(Clone, Debug)]
pub struct SteadyMessageQueue {
    pub queue: VecDeque<SteadyPacketData>,
}

impl SteadyMessageQueue {
    pub fn new() -> Self {
        SteadyMessageQueue {
            queue: VecDeque::new(),
        }
    }

    pub fn peek(&self) -> Option<&SteadyPacketData> {
        self.queue.front()
    }

    pub fn pop(&mut self) -> Option<SteadyPacketData> {
        self.queue.pop_front()
    }

    pub fn push(&mut self, packet: SteadyPacketData) {
        self.queue.push_back(packet);
    }
}