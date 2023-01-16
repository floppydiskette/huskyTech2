use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use egui_glfw_gl::egui;
use egui_glfw_gl::egui::{CentralPanel, Frame, Ui};
use gfx_maths::Vec3;
use crate::renderer::ht_renderer;

lazy_static!{
    pub static ref SHOW_DEBUG_LOCATION: AtomicBool = AtomicBool::new(true);
    pub static ref DEBUG_LOCATION: Arc<Mutex<Vec3>> = Arc::new(Mutex::new(Vec3::new(0.0, 0.0, 0.0)));
}

pub fn render(renderer: &mut ht_renderer) {
    CentralPanel::default()
        .frame(Frame::none())
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            if SHOW_DEBUG_LOCATION.load(Ordering::Relaxed) {
                render_debug_location(ui);
            }
        });
}

fn render_debug_location(ui: &mut Ui) {
    let debug_location = DEBUG_LOCATION.lock().unwrap();
    // label at top right
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.label(format!("x: {}, y: {}, z: {}", debug_location.x, debug_location.y, debug_location.z));
    });
}