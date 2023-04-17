use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use egui_glfw_gl::egui;
use egui_glfw_gl::egui::{CentralPanel, Frame, SidePanel, TopBottomPanel, Ui};
use gfx_maths::Vec3;
use crate::renderer::ht_renderer;

lazy_static!{
    pub static ref SHOW_UI: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    pub static ref SHOW_DEBUG_LOCATION: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref SHOW_FPS: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref SHOW_DEBUG_LOG: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref DEBUG_LOCATION: Arc<Mutex<Vec3>> = Arc::new(Mutex::new(Vec3::new(0.0, 0.0, 0.0)));
    pub static ref FPS: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    pub static ref DEBUG_LOG: Arc<Mutex<OnScreenDebugLog>> = Arc::new(Mutex::new(OnScreenDebugLog {
        buffer: VecDeque::new(),
    }));
    pub static ref DEBUG_SHADOW_VOLUME_FACE_ANGLE: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
}

pub struct OnScreenDebugLog {
    buffer: VecDeque<String>,
}

impl OnScreenDebugLog {
    const MAX_LOG_SIZE: usize = 10;

    pub fn log(&mut self, message: String) {
        self.buffer.push_back(message);
        if self.buffer.len() > Self::MAX_LOG_SIZE {
            self.buffer.pop_front();
        }
    }

    pub fn get(&mut self) -> Vec<String> {
        self.buffer.iter().cloned().collect()
    }
}

pub fn debug_log(message: impl ToString) {
    DEBUG_LOG.lock().unwrap().log(message.to_string());
}

pub fn render(renderer: &mut ht_renderer) {
    if !SHOW_UI.load(Ordering::Relaxed) {
        return;
    }

    SidePanel::left("left_debug")
        .frame(Frame::none())
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            // left align
            if SHOW_DEBUG_LOG.load(Ordering::Relaxed) {
                render_debug_log(ui);
            }
        });

    SidePanel::right("right_debug")
        .frame(Frame::none())
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            // right align
            if SHOW_DEBUG_LOCATION.load(Ordering::Relaxed) {
                render_debug_location(ui);
            }
            if SHOW_FPS.load(Ordering::Relaxed) {
                render_fps(ui);
            }
        });

    let mut sv_face_angle = DEBUG_SHADOW_VOLUME_FACE_ANGLE.lock().unwrap();
    egui::Window::new("Shadow Volume")
        .resizable(false)
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            ui.add(egui::Slider::new(&mut *sv_face_angle, -90.0..=90.0).text("Face Angle"));
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
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.label(format!("x: {}, y: {}, z: {}", debug_location.x, debug_location.y, debug_location.z));
    });
}

fn render_fps(ui: &mut Ui) {
    let fps = FPS.lock().unwrap();
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.label(format!("FPS: {}", *fps as u32));
    });
}

fn render_debug_log(ui: &mut Ui) {
    let mut debug_log = DEBUG_LOG.lock().unwrap();
    let log = debug_log.get();
    ui.add_space(10.0);
    for message in log {
        ui.allocate_ui_with_layout(egui::Vec2::new(200.0, 200.0), egui::Layout::left_to_right(egui::Align::LEFT), |ui| {
            ui.add_space(10.0);
            ui.label(message);
        });
    }
}