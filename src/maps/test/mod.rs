use std::sync::{Arc, Mutex};
use halfbrown::HashMap;
use crate::maps::triggers::{Trigger, TriggerType};

lazy_static!{
    static ref TRIGGERS: crate::maps::TriggerMap = {
        let mut map = HashMap::new();
        map.insert("test".to_string(), Arc::new(Mutex::new(Trigger::trigger(
            TriggerType::OnEnter,
            |_, _, _| {
                println!("test");
            }
        ))));
        Arc::new(map)
    };
}

pub fn get_trigger(key: impl ToString) -> Option<Arc<Mutex<Trigger>>> {
    let key = key.to_string();
    TRIGGERS.get(&key).cloned()
}