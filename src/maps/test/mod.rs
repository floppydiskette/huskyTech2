use std::sync::{Arc, Mutex};
use halfbrown::HashMap;
use crate::maps::triggers::{Trigger, TriggerType};

pub struct TestMap {}

lazy_static!{
    static ref TRIGGERS: crate::maps::TriggerMap = {
        let mut map = HashMap::new();
        map.insert("test".to_string(), Trigger::trigger(
            TriggerType::OnEnter,
            |_, _, _| {
                println!("test");
            }
        ))
        Arc::new(Mutex::new(map))
    };
}

pub fn get_trigger<'a>(key: impl ToString) -> Option<&'a mut Trigger> {
    let key = key.to_string();
    TRIGGERS.lock().unwrap().get_mut(&key)
}