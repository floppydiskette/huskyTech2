use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::{Arc, Mutex};
use libsex::bindings::*;
use crate::ht_renderer;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    W,
    A,
    S,
    D,
    Space,
    LeftShift,
    LeftControl,
    LeftAlt,
    RightAlt,
    Left,
    Right,
    Up,
    Down,
    Escape,
    Tab,
    Enter,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    N0,
    N1,
    N2,
    N3,
    N4,
    N5,
    N6,
    N7,
    N8,
    N9,
    Apostrophe,
    Backslash,
    Comma,
    Equal,
    GraveAccent,
    LeftBracket,
    Minus,
    Period,
    RightBracket,
    Semicolon,
    Slash,
    World1,
    World2,
    Unknown,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyState {
    Pressed,
    Released,
    Repeated,
    TakenCareOf,
    Unknown,
}

pub struct Keyboard {
    pub key_state: HashMap<Key, KeyState>,
}

// todo! maybe there's a better way to do this
impl Default for Keyboard {
    fn default() -> Self {
        let mut key_state = HashMap::new();
        key_state.insert(Key::W, KeyState::TakenCareOf);
        key_state.insert(Key::A, KeyState::TakenCareOf);
        key_state.insert(Key::S, KeyState::TakenCareOf);
        key_state.insert(Key::D, KeyState::TakenCareOf);
        key_state.insert(Key::Space, KeyState::TakenCareOf);
        key_state.insert(Key::LeftShift, KeyState::TakenCareOf);
        key_state.insert(Key::LeftControl, KeyState::TakenCareOf);
        key_state.insert(Key::LeftAlt, KeyState::TakenCareOf);
        key_state.insert(Key::RightAlt, KeyState::TakenCareOf);
        key_state.insert(Key::Left, KeyState::TakenCareOf);
        key_state.insert(Key::Right, KeyState::TakenCareOf);
        key_state.insert(Key::Up, KeyState::TakenCareOf);
        key_state.insert(Key::Down, KeyState::TakenCareOf);
        key_state.insert(Key::Escape, KeyState::TakenCareOf);
        key_state.insert(Key::Tab, KeyState::TakenCareOf);
        key_state.insert(Key::Enter, KeyState::TakenCareOf);
        key_state.insert(Key::Backspace, KeyState::TakenCareOf);
        key_state.insert(Key::Delete, KeyState::TakenCareOf);
        key_state.insert(Key::Home, KeyState::TakenCareOf);
        key_state.insert(Key::End, KeyState::TakenCareOf);
        key_state.insert(Key::PageUp, KeyState::TakenCareOf);
        key_state.insert(Key::PageDown, KeyState::TakenCareOf);
        key_state.insert(Key::Insert, KeyState::TakenCareOf);
        key_state.insert(Key::F1, KeyState::TakenCareOf);
        key_state.insert(Key::F2, KeyState::TakenCareOf);
        key_state.insert(Key::F3, KeyState::TakenCareOf);
        key_state.insert(Key::F4, KeyState::TakenCareOf);
        key_state.insert(Key::F5, KeyState::TakenCareOf);
        key_state.insert(Key::F6, KeyState::TakenCareOf);
        key_state.insert(Key::F7, KeyState::TakenCareOf);
        key_state.insert(Key::F8, KeyState::TakenCareOf);
        key_state.insert(Key::F9, KeyState::TakenCareOf);
        key_state.insert(Key::F10, KeyState::TakenCareOf);
        key_state.insert(Key::F11, KeyState::TakenCareOf);
        key_state.insert(Key::F12, KeyState::TakenCareOf);
        key_state.insert(Key::F13, KeyState::TakenCareOf);
        key_state.insert(Key::F14, KeyState::TakenCareOf);
        key_state.insert(Key::F15, KeyState::TakenCareOf);
        key_state.insert(Key::F16, KeyState::TakenCareOf);
        key_state.insert(Key::F17, KeyState::TakenCareOf);
        key_state.insert(Key::F18, KeyState::TakenCareOf);
        key_state.insert(Key::F19, KeyState::TakenCareOf);
        key_state.insert(Key::F20, KeyState::TakenCareOf);
        key_state.insert(Key::F21, KeyState::TakenCareOf);
        key_state.insert(Key::F22, KeyState::TakenCareOf);
        key_state.insert(Key::F23, KeyState::TakenCareOf);
        key_state.insert(Key::F24, KeyState::TakenCareOf);
        key_state.insert(Key::F25, KeyState::TakenCareOf);
        key_state.insert(Key::N0, KeyState::TakenCareOf);
        key_state.insert(Key::N1, KeyState::TakenCareOf);
        key_state.insert(Key::N2, KeyState::TakenCareOf);
        key_state.insert(Key::N3, KeyState::TakenCareOf);
        key_state.insert(Key::N4, KeyState::TakenCareOf);
        key_state.insert(Key::N5, KeyState::TakenCareOf);
        key_state.insert(Key::N6, KeyState::TakenCareOf);
        key_state.insert(Key::N7, KeyState::TakenCareOf);
        key_state.insert(Key::N8, KeyState::TakenCareOf);
        key_state.insert(Key::N9, KeyState::TakenCareOf);
        key_state.insert(Key::Apostrophe, KeyState::TakenCareOf);
        key_state.insert(Key::Backslash, KeyState::TakenCareOf);
        key_state.insert(Key::Comma, KeyState::TakenCareOf);
        key_state.insert(Key::Equal, KeyState::TakenCareOf);
        key_state.insert(Key::GraveAccent, KeyState::TakenCareOf);
        key_state.insert(Key::LeftBracket, KeyState::TakenCareOf);
        key_state.insert(Key::Minus, KeyState::TakenCareOf);
        key_state.insert(Key::Period, KeyState::TakenCareOf);
        key_state.insert(Key::RightBracket, KeyState::TakenCareOf);
        key_state.insert(Key::Semicolon, KeyState::TakenCareOf);
        key_state.insert(Key::Slash, KeyState::TakenCareOf);
        key_state.insert(Key::World1, KeyState::TakenCareOf);
        key_state.insert(Key::World2, KeyState::TakenCareOf);

        Self {
            key_state,
        }
    }
}

lazy_static!{
    pub static ref KEYBOARD: Arc<Mutex<Keyboard>> = Arc::new(Mutex::new(Keyboard::default()));
}

pub fn glfw_key_to_key(key: c_int) -> Key {
    match key as u32 {
        GLFW_KEY_W => Key::W,
        GLFW_KEY_A => Key::A,
        GLFW_KEY_S => Key::S,
        GLFW_KEY_D => Key::D,
        GLFW_KEY_SPACE => Key::Space,
        GLFW_KEY_LEFT_SHIFT => Key::LeftShift,
        GLFW_KEY_LEFT_CONTROL => Key::LeftControl,
        GLFW_KEY_LEFT_ALT => Key::LeftAlt,
        GLFW_KEY_RIGHT_ALT => Key::RightAlt,
        GLFW_KEY_LEFT => Key::Left,
        GLFW_KEY_RIGHT => Key::Right,
        GLFW_KEY_UP => Key::Up,
        GLFW_KEY_DOWN => Key::Down,
        GLFW_KEY_ESCAPE => Key::Escape,
        GLFW_KEY_TAB => Key::Tab,
        GLFW_KEY_ENTER => Key::Enter,
        GLFW_KEY_BACKSPACE => Key::Backspace,
        GLFW_KEY_DELETE => Key::Delete,
        GLFW_KEY_HOME => Key::Home,
        GLFW_KEY_END => Key::End,
        GLFW_KEY_PAGE_UP => Key::PageUp,
        GLFW_KEY_PAGE_DOWN => Key::PageDown,
        GLFW_KEY_INSERT => Key::Insert,
        GLFW_KEY_F1 => Key::F1,
        GLFW_KEY_F2 => Key::F2,
        GLFW_KEY_F3 => Key::F3,
        GLFW_KEY_F4 => Key::F4,
        GLFW_KEY_F5 => Key::F5,
        GLFW_KEY_F6 => Key::F6,
        GLFW_KEY_F7 => Key::F7,
        GLFW_KEY_F8 => Key::F8,
        GLFW_KEY_F9 => Key::F9,
        GLFW_KEY_F10 => Key::F10,
        GLFW_KEY_F11 => Key::F11,
        GLFW_KEY_F12 => Key::F12,
        GLFW_KEY_F13 => Key::F13,
        GLFW_KEY_F14 => Key::F14,
        GLFW_KEY_F15 => Key::F15,
        GLFW_KEY_F16 => Key::F16,
        GLFW_KEY_F17 => Key::F17,
        GLFW_KEY_F18 => Key::F18,
        GLFW_KEY_F19 => Key::F19,
        GLFW_KEY_F20 => Key::F20,
        GLFW_KEY_F21 => Key::F21,
        GLFW_KEY_F22 => Key::F22,
        GLFW_KEY_F23 => Key::F23,
        GLFW_KEY_F24 => Key::F24,
        GLFW_KEY_F25 => Key::F25,
        GLFW_KEY_KP_0 => Key::N0,
        GLFW_KEY_KP_1 => Key::N1,
        GLFW_KEY_KP_2 => Key::N2,
        GLFW_KEY_KP_3 => Key::N3,
        GLFW_KEY_KP_4 => Key::N4,
        GLFW_KEY_KP_5 => Key::N5,
        GLFW_KEY_KP_6 => Key::N6,
        GLFW_KEY_KP_7 => Key::N7,
        GLFW_KEY_KP_8 => Key::N8,
        GLFW_KEY_KP_9 => Key::N9,
        GLFW_KEY_KP_DECIMAL => Key::Period,
        GLFW_KEY_KP_DIVIDE => Key::Slash,
        GLFW_KEY_KP_SUBTRACT => Key::Minus,
        GLFW_KEY_KP_ENTER => Key::Enter,
        GLFW_KEY_KP_EQUAL => Key::Equal,
        _ => Key::Unknown,
    }
}

pub fn glfw_key_action_to_keystate(action: c_int) -> KeyState {
    match action as u32 {
        GLFW_PRESS => KeyState::Pressed,
        GLFW_RELEASE => KeyState::Released,
        GLFW_REPEAT => KeyState::Repeated,
        _ => KeyState::Unknown,
    }
}

unsafe extern "C" fn keyboard_callback(window: *mut GLFWwindow, key: c_int, scancode: c_int, action: c_int, mods: c_int) {
    let key = glfw_key_to_key(key);
    let keystate = glfw_key_action_to_keystate(action);
    let mut keyboard = KEYBOARD.lock().unwrap();
    keyboard.key_state.insert(key, keystate);
}

pub fn init(renderer: &mut ht_renderer) {
    unsafe {
        glfwSetKeyCallback(renderer.backend.window, Some(keyboard_callback));
    }
}

pub fn tick_keyboard() {
    {
        let mut keyboard = KEYBOARD.lock().unwrap();
        for (_, state) in keyboard.key_state.iter_mut() {
            *state = KeyState::TakenCareOf;
        }
    }
    unsafe {
        glfwPollEvents();
    }
}

pub fn check_key_pressed(key: Key) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(&key) {
        if *state == KeyState::Pressed {
            return true;
        }
    }
    false
}

pub fn check_key_released(key: Key) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(&key) {
        if *state == KeyState::Released {
            return true;
        }
    }
    false
}

pub fn check_key_repeated(key: Key) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(&key) {
        if *state == KeyState::Repeated {
            return true;
        }
    }
    false
}

pub fn check_key_down(key: Key) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(&key) {
        if *state == KeyState::Pressed || *state == KeyState::Repeated {
            return true;
        }
    }
    false
}