use gfx_maths::{Quaternion, Vec3};
use crate::renderer::ht_renderer;
use crate::worldmachine::EntityId;

#[derive(Clone)]
pub struct Trigger {
    pub position: Vec3,
    pub rotation: Quaternion,
    pub scale: Vec3,
    pub report: TriggerReport,
    pub trigger_type: TriggerType,
    pub trigger_fn: Option<fn(&mut ht_renderer, &mut Trigger, WhoTriggered)>,
    state: bool,
}

#[derive(Debug, Clone)]
pub enum TriggerType {
    OnEnter,
    OnUse,
}

#[derive(Debug, Clone)]
pub enum TriggerReport {
    OnlyPlayers,
    OnlyClient,
    OnlyNPCS,
    All,
}

#[derive(Debug, Clone)]
pub enum WhoTriggered {
    Client,
    Player(EntityId),
    Other(EntityId),
}

impl Trigger {
    pub fn trigger(trigger_type: TriggerType, trigger_fn: fn(&mut ht_renderer, &mut Trigger, WhoTriggered)) -> Trigger {
        Trigger {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
            report: TriggerReport::OnlyClient,
            trigger_type,
            trigger_fn: Some(trigger_fn),
            state: false,
        }
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }
    pub fn set_size(&mut self, scale: Vec3) {
        self.scale = scale;
    }
}