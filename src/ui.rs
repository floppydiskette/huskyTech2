use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use egui_glfw_gl::egui;
use egui_glfw_gl::egui::{CentralPanel, Frame, Ui};
use gfx_maths::Vec3;
use crate::renderer::ht_renderer;

lazy_static!{
    pub static ref SHOW_UI: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    pub static ref SHOW_DEBUG_LOCATION: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref SHOW_FPS: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref DEBUG_LOCATION: Arc<Mutex<Vec3>> = Arc::new(Mutex::new(Vec3::new(0.0, 0.0, 0.0)));
    pub static ref FPS: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
}

pub fn render(renderer: &mut ht_renderer) {
    if !SHOW_UI.load(Ordering::Relaxed) {
        return;
    }

    CentralPanel::default()
        .frame(Frame::none())
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            if SHOW_DEBUG_LOCATION.load(Ordering::Relaxed) {
                render_debug_location(ui);
            }
            if SHOW_FPS.load(Ordering::Relaxed) {
                render_fps(ui);
            }
        });

    let egui::FullOutput {
        platform_output,
        repaint_after: _,
        textures_delta,
        shapes,
    } = renderer.backend.egui_context.lock().unwrap().end_frame();

    //Handle cut, copy text from egui
    if !platform_output.copied_text.is_empty() {
        egui_glfw_gl::copy_to_clipboard(&mut renderer.backend.input_state.lock().unwrap(), platform_output.copied_text);
    }

    let clipped_shapes = renderer.backend.egui_context.lock().unwrap().tessellate(shapes);
    renderer.backend.painter.lock().unwrap().paint_and_update_textures(1.0, &clipped_shapes, &textures_delta);
}

fn render_debug_location(ui: &mut Ui) {
    let debug_location = DEBUG_LOCATION.lock().unwrap();
    // label at top right
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.label(format!("x: {}, y: {}, z: {}", debug_location.x, debug_location.y, debug_location.z));
    });
}

fn render_fps(ui: &mut Ui) {
    let fps = FPS.lock().unwrap();
    // label at top right
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.label(format!("FPS: {}", *fps as u32));
    });
}