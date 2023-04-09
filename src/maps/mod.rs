use std::sync::{Arc, Mutex};
use halfbrown::HashMap;
use crate::maps::triggers::Trigger;

pub mod triggers;
mod test;

pub type TriggerMap = Arc<Mutex<HashMap<String, Trigger>>>;

pub fn get_trigger<'a>(map: impl ToString, key: impl ToString) -> Option<&'a mut Trigger> {
    let map = map.to_string();
    let key = key.to_string();
    match map.as_str() {
        "test" => {
            test::get_trigger(&key)
        },
        _ => {
            None
        }
    }
}