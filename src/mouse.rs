use std::sync::{Arc, Mutex};
use gfx_maths::Vec2;
use libsex::bindings::*;
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

unsafe extern "C" fn cursor_position_callback(window: *mut GLFWwindow, xpos: f64, ypos: f64) {
    let mut mouse = MOUSE.lock().unwrap();
    mouse.position.x = xpos as f32;
    mouse.position.y = ypos as f32;
}

unsafe extern "C" fn mouse_button_callback(window: *mut GLFWwindow, button: i32, action: i32, mods: i32) {
    let mut mouse = MOUSE.lock().unwrap();
    match action as u32 {
        GLFW_PRESS => {
            match button as u32 {
                GLFW_MOUSE_BUTTON_LEFT => mouse.buttons[0] = MouseButtonState::Pressed,
                GLFW_MOUSE_BUTTON_RIGHT => mouse.buttons[1] = MouseButtonState::Pressed,
                GLFW_MOUSE_BUTTON_MIDDLE => mouse.buttons[2] = MouseButtonState::Pressed,
                _ => {}
            }
        }
        GLFW_RELEASE => {
            match button as u32 {
                GLFW_MOUSE_BUTTON_LEFT => mouse.buttons[0] = MouseButtonState::Released,
                GLFW_MOUSE_BUTTON_RIGHT => mouse.buttons[1] = MouseButtonState::Released,
                GLFW_MOUSE_BUTTON_MIDDLE => mouse.buttons[2] = MouseButtonState::Released,
                _ => {}
            }
        }
        _ => {}
    }
}

pub fn init(renderer: &mut ht_renderer) {
    unsafe {
        glfwSetCursorPosCallback(renderer.backend.window, Some(cursor_position_callback));
        glfwSetMouseButtonCallback(renderer.backend.window, Some(mouse_button_callback));
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

pub fn tick_mouse() {
    {
        let mut mouse = MOUSE.lock().unwrap();
        for i in 0..3 {
            mouse.buttons[i] = MouseButtonState::TakenCareOf;
        }
    }
    unsafe {
        glfwPollEvents();
    }
}