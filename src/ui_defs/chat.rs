use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use egui_glfw_gl::egui;
use egui_glfw_gl::egui::Ui;
use crate::worldmachine::WorldMachine;

lazy_static!{
    pub static ref CHAT_BUFFER: Arc<Mutex<ChatBuffer>> = Arc::new(Mutex::new(ChatBuffer {
        my_name: "awaiting initial name".to_string(),
        messages: VecDeque::new(),
        next_message_buffer: "".to_string(),
    }));
}

pub struct ChatBuffer {
    pub my_name: String,
    pub messages: VecDeque<ChatMessage>,
    pub next_message_buffer: String,
}

unsafe impl Send for ChatBuffer {}
unsafe impl Sync for ChatBuffer {}

pub struct ChatMessage {
    pub name: String,
    pub message: String,
}

unsafe impl Send for ChatMessage {}
unsafe impl Sync for ChatMessage {}

pub fn write_chat(name: String, msg: String) {
    let mut chat_buffer = CHAT_BUFFER.lock().unwrap();
    chat_buffer.messages.push_back(ChatMessage {
        name: name,
        message: msg,
    });
}

pub fn chat(ui: &mut Ui, wm: &mut WorldMachine) -> (Option<String>, Option<String>) {

    let mut set_name = None;

    ui.horizontal(|ui| {
        ui.label("your name is: ");
        if ui.text_edit_singleline(&mut CHAT_BUFFER.lock().unwrap().my_name).lost_focus() {
            let name = CHAT_BUFFER.lock().unwrap().my_name.clone();
            set_name = Some(name);
        }
    });

    ui.separator();

    for msg in CHAT_BUFFER.lock().unwrap().messages.iter() {
        ui.horizontal(|ui| {
            ui.label(msg.name.clone());
            ui.label(": ");
            let label = egui::Label::new(msg.message.clone());
            let label = label.wrap(true);
            ui.add(label);
        });
    }

    ui.separator();

    let mut send_message = None;

    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut CHAT_BUFFER.lock().unwrap().next_message_buffer);
        if ui.button("send").clicked() {
            let message = CHAT_BUFFER.lock().unwrap().next_message_buffer.clone();
            CHAT_BUFFER.lock().unwrap().next_message_buffer = "".to_string();
            write_chat("you".to_string(), message.clone());
            send_message = Some(message);
        }
    });

    (set_name, send_message)
}