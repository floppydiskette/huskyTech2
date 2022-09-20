use std::sync::{Arc, Mutex};
use gfx_maths::Vec2;
use glfw::{Action, MouseButton, WindowEvent};
use crate::ht_renderer;

#[derive(Copy, Clone, Debug)]
pub enum MouseButtonState {
    Pressed,
    Released,
    TakenCareOf,
}

pub struct Mouse {
    pub position: Vec2,
    pub buttons: [MouseButtonState; 3],
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            position: Vec2::new(0.0, 0.0),
            buttons: [MouseButtonState::TakenCareOf; 3],
        }
    }
}

lazy_static! {
    pub static ref MOUSE: Arc<Mutex<Mouse>> = Arc::new(Mutex::new(Mouse::default()));
}

fn cursor_position_callback(xpos: f64, ypos: f64) {
    let mut mouse = MOUSE.lock().unwrap();
    mouse.position.x = xpos as f32;
    mouse.position.y = ypos as f32;
}

fn mouse_button_callback(button: MouseButton, action: Action) {
    let mut mouse = MOUSE.lock().unwrap();
    match action {
        Action::Press => {
            match button {
                MouseButton::Button1 => mouse.buttons[0] = MouseButtonState::Pressed,
                MouseButton::Button2 => mouse.buttons[1] = MouseButtonState::Pressed,
                MouseButton::Button3 => mouse.buttons[2] = MouseButtonState::Pressed,
                _ => {}
            }
        }
        Action::Release => {
            match button {
                MouseButton::Button1 => mouse.buttons[0] = MouseButtonState::Released,
                MouseButton::Button2 => mouse.buttons[1] = MouseButtonState::Released,
                MouseButton::Button3 => mouse.buttons[2] = MouseButtonState::Released,
                _ => {}
            }
        }
        _ => {}
    }
}

pub fn get_mouse_pos() -> Vec2 {
    let mouse = MOUSE.lock().unwrap();
    mouse.position
}

pub fn get_mouse_button_state(button: u32) -> MouseButtonState {
    let mouse = MOUSE.lock().unwrap();
    mouse.buttons[button as usize]
}

pub fn reset_mouse_state() {
    let mut mouse = MOUSE.lock().unwrap();
    for i in 0..3 {
        mouse.buttons[i] = MouseButtonState::TakenCareOf;
    }
}

pub fn tick_mouse(event: WindowEvent) {
    match event {
        WindowEvent::CursorPos(x, y) => unsafe {
            cursor_position_callback(x, y);
        },
        WindowEvent::MouseButton(button, action, mods) => unsafe {
            mouse_button_callback(button, action);
        },
        _ => {}
    }
}