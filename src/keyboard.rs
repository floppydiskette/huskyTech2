use std::sync::{Arc, Mutex};
use glfw::{Action, WindowEvent};
use crate::optimisations::keyboardmap::KeyboardMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HTKey {
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
    pub key_state: KeyboardMap,
}

// todo! maybe there's a better way to do this
impl Default for Keyboard {
    fn default() -> Self {
        let mut key_state = KeyboardMap::default();
        key_state.insert(HTKey::W, KeyState::TakenCareOf);
        key_state.insert(HTKey::A, KeyState::TakenCareOf);
        key_state.insert(HTKey::S, KeyState::TakenCareOf);
        key_state.insert(HTKey::D, KeyState::TakenCareOf);
        key_state.insert(HTKey::Space, KeyState::TakenCareOf);
        key_state.insert(HTKey::LeftShift, KeyState::TakenCareOf);
        key_state.insert(HTKey::LeftControl, KeyState::TakenCareOf);
        key_state.insert(HTKey::LeftAlt, KeyState::TakenCareOf);
        key_state.insert(HTKey::RightAlt, KeyState::TakenCareOf);
        key_state.insert(HTKey::Left, KeyState::TakenCareOf);
        key_state.insert(HTKey::Right, KeyState::TakenCareOf);
        key_state.insert(HTKey::Up, KeyState::TakenCareOf);
        key_state.insert(HTKey::Down, KeyState::TakenCareOf);
        key_state.insert(HTKey::Escape, KeyState::TakenCareOf);
        key_state.insert(HTKey::Tab, KeyState::TakenCareOf);
        key_state.insert(HTKey::Enter, KeyState::TakenCareOf);
        key_state.insert(HTKey::Backspace, KeyState::TakenCareOf);
        key_state.insert(HTKey::Delete, KeyState::TakenCareOf);
        key_state.insert(HTKey::Home, KeyState::TakenCareOf);
        key_state.insert(HTKey::End, KeyState::TakenCareOf);
        key_state.insert(HTKey::PageUp, KeyState::TakenCareOf);
        key_state.insert(HTKey::PageDown, KeyState::TakenCareOf);
        key_state.insert(HTKey::Insert, KeyState::TakenCareOf);
        key_state.insert(HTKey::F1, KeyState::TakenCareOf);
        key_state.insert(HTKey::F2, KeyState::TakenCareOf);
        key_state.insert(HTKey::F3, KeyState::TakenCareOf);
        key_state.insert(HTKey::F4, KeyState::TakenCareOf);
        key_state.insert(HTKey::F5, KeyState::TakenCareOf);
        key_state.insert(HTKey::F6, KeyState::TakenCareOf);
        key_state.insert(HTKey::F7, KeyState::TakenCareOf);
        key_state.insert(HTKey::F8, KeyState::TakenCareOf);
        key_state.insert(HTKey::F9, KeyState::TakenCareOf);
        key_state.insert(HTKey::F10, KeyState::TakenCareOf);
        key_state.insert(HTKey::F11, KeyState::TakenCareOf);
        key_state.insert(HTKey::F12, KeyState::TakenCareOf);
        key_state.insert(HTKey::F13, KeyState::TakenCareOf);
        key_state.insert(HTKey::F14, KeyState::TakenCareOf);
        key_state.insert(HTKey::F15, KeyState::TakenCareOf);
        key_state.insert(HTKey::F16, KeyState::TakenCareOf);
        key_state.insert(HTKey::F17, KeyState::TakenCareOf);
        key_state.insert(HTKey::F18, KeyState::TakenCareOf);
        key_state.insert(HTKey::F19, KeyState::TakenCareOf);
        key_state.insert(HTKey::F20, KeyState::TakenCareOf);
        key_state.insert(HTKey::F21, KeyState::TakenCareOf);
        key_state.insert(HTKey::F22, KeyState::TakenCareOf);
        key_state.insert(HTKey::F23, KeyState::TakenCareOf);
        key_state.insert(HTKey::F24, KeyState::TakenCareOf);
        key_state.insert(HTKey::F25, KeyState::TakenCareOf);
        key_state.insert(HTKey::N0, KeyState::TakenCareOf);
        key_state.insert(HTKey::N1, KeyState::TakenCareOf);
        key_state.insert(HTKey::N2, KeyState::TakenCareOf);
        key_state.insert(HTKey::N3, KeyState::TakenCareOf);
        key_state.insert(HTKey::N4, KeyState::TakenCareOf);
        key_state.insert(HTKey::N5, KeyState::TakenCareOf);
        key_state.insert(HTKey::N6, KeyState::TakenCareOf);
        key_state.insert(HTKey::N7, KeyState::TakenCareOf);
        key_state.insert(HTKey::N8, KeyState::TakenCareOf);
        key_state.insert(HTKey::N9, KeyState::TakenCareOf);
        key_state.insert(HTKey::Apostrophe, KeyState::TakenCareOf);
        key_state.insert(HTKey::Backslash, KeyState::TakenCareOf);
        key_state.insert(HTKey::Comma, KeyState::TakenCareOf);
        key_state.insert(HTKey::Equal, KeyState::TakenCareOf);
        key_state.insert(HTKey::GraveAccent, KeyState::TakenCareOf);
        key_state.insert(HTKey::LeftBracket, KeyState::TakenCareOf);
        key_state.insert(HTKey::Minus, KeyState::TakenCareOf);
        key_state.insert(HTKey::Period, KeyState::TakenCareOf);
        key_state.insert(HTKey::RightBracket, KeyState::TakenCareOf);
        key_state.insert(HTKey::Semicolon, KeyState::TakenCareOf);
        key_state.insert(HTKey::Slash, KeyState::TakenCareOf);
        key_state.insert(HTKey::World1, KeyState::TakenCareOf);
        key_state.insert(HTKey::World2, KeyState::TakenCareOf);

        Self {
            key_state,
        }
    }
}

lazy_static!{
    pub static ref KEYBOARD: Arc<Mutex<Keyboard>> = Arc::new(Mutex::new(Keyboard::default()));
}

pub fn glfw_key_to_key(key: glfw::Key) -> HTKey {
    match key {
        glfw::Key::W => HTKey::W,
        glfw::Key::A => HTKey::A,
        glfw::Key::S => HTKey::S,
        glfw::Key::D => HTKey::D,
        glfw::Key::Space => HTKey::Space,
        glfw::Key::LeftShift => HTKey::LeftShift,
        glfw::Key::LeftControl => HTKey::LeftControl,
        glfw::Key::LeftAlt => HTKey::LeftAlt,
        glfw::Key::RightAlt => HTKey::RightAlt,
        glfw::Key::Left => HTKey::Left,
        glfw::Key::Right => HTKey::Right,
        glfw::Key::Up => HTKey::Up,
        glfw::Key::Down => HTKey::Down,
        glfw::Key::Escape => HTKey::Escape,
        glfw::Key::Tab => HTKey::Tab,
        glfw::Key::Enter => HTKey::Enter,
        glfw::Key::Backspace => HTKey::Backspace,
        glfw::Key::Delete => HTKey::Delete,
        glfw::Key::Home => HTKey::Home,
        glfw::Key::End => HTKey::End,
        glfw::Key::PageUp => HTKey::PageUp,
        glfw::Key::PageDown => HTKey::PageDown,
        glfw::Key::Insert => HTKey::Insert,
        glfw::Key::Comma => HTKey::Comma,
        glfw::Key::Period => HTKey::Period,
        glfw::Key::F1 => HTKey::F1,
        glfw::Key::F2 => HTKey::F2,
        glfw::Key::F3 => HTKey::F3,
        glfw::Key::F4 => HTKey::F4,
        glfw::Key::F5 => HTKey::F5,
        glfw::Key::F6 => HTKey::F6,
        glfw::Key::F7 => HTKey::F7,
        glfw::Key::F8 => HTKey::F8,
        glfw::Key::F9 => HTKey::F9,
        glfw::Key::F10 => HTKey::F10,
        glfw::Key::F11 => HTKey::F11,
        glfw::Key::F12 => HTKey::F12,
        glfw::Key::F13 => HTKey::F13,
        glfw::Key::F14 => HTKey::F14,
        glfw::Key::F15 => HTKey::F15,
        glfw::Key::F16 => HTKey::F16,
        glfw::Key::F17 => HTKey::F17,
        glfw::Key::F18 => HTKey::F18,
        glfw::Key::F19 => HTKey::F19,
        glfw::Key::F20 => HTKey::F20,
        glfw::Key::F21 => HTKey::F21,
        glfw::Key::F22 => HTKey::F22,
        glfw::Key::F23 => HTKey::F23,
        glfw::Key::F24 => HTKey::F24,
        glfw::Key::F25 => HTKey::F25,
        glfw::Key::Kp0 => HTKey::N0,
        glfw::Key::Kp1 => HTKey::N1,
        glfw::Key::Kp2 => HTKey::N2,
        glfw::Key::Kp3 => HTKey::N3,
        glfw::Key::Kp4 => HTKey::N4,
        glfw::Key::Kp5 => HTKey::N5,
        glfw::Key::Kp6 => HTKey::N6,
        glfw::Key::Kp7 => HTKey::N7,
        glfw::Key::Kp8 => HTKey::N8,
        glfw::Key::Kp9 => HTKey::N9,
        glfw::Key::KpDecimal => HTKey::Period,
        glfw::Key::KpDivide => HTKey::Slash,
        glfw::Key::KpSubtract => HTKey::Minus,
        glfw::Key::KpEnter => HTKey::Enter,
        glfw::Key::KpEqual => HTKey::Equal,
        _ => HTKey::Unknown,
    }
}

pub fn glfw_key_action_to_keystate(action: Action) -> KeyState {
    match action {
        Action::Press => KeyState::Pressed,
        Action::Release => KeyState::Released,
        Action::Repeat => KeyState::Repeated,
        _ => KeyState::Unknown,
    }
}

pub fn reset_keyboard_state() {
    let mut keyboard = KEYBOARD.lock().unwrap();
    keyboard.key_state.set_all(KeyState::TakenCareOf);
}

pub fn tick_keyboard(event: WindowEvent) {
    if let WindowEvent::Key(key, _, action, _) = event {
        let mut keyboard = KEYBOARD.lock().unwrap();
        let key = glfw_key_to_key(key);
        let keystate = glfw_key_action_to_keystate(action);
        keyboard.key_state.insert(key, keystate);
    }

}

pub fn check_key_pressed(key: HTKey) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(key) {
        if *state == KeyState::Pressed {
            return true;
        }
    }
    false
}

pub fn check_key_released(key: HTKey) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(key) {
        if *state == KeyState::Released {
            return true;
        }
    }
    false
}

pub fn check_key_repeated(key: HTKey) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(key) {
        if *state == KeyState::Repeated {
            return true;
        }
    }
    false
}

pub fn check_key_down(key: HTKey) -> bool {
    let keyboard = KEYBOARD.lock().unwrap();
    if let Some(state) = keyboard.key_state.get(key) {
        if *state == KeyState::Pressed || *state == KeyState::Repeated {
            return true;
        }
    }
    false
}