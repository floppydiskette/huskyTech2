use crate::HTKey;
use crate::keyboard::KeyState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardMap {
    inner: [Option<KeyState>; 256],
}

impl Default for KeyboardMap {
    fn default() -> Self {
        Self {
            inner: [None; 256],
        }
    }
}

impl KeyboardMap {
    pub fn insert(&mut self, key: HTKey, state: KeyState) {
        self.inner[key as usize] = Some(state);
    }

    pub fn get(&self, key: HTKey) -> Option<&KeyState> {
        self.inner[key as usize].as_ref()
    }

    pub fn set_all(&mut self, state: KeyState) {
        for i in 0..256 {
            self.inner[i] = Some(state);
        }
    }
}