use std::collections::VecDeque;
use std::ops::Mul;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use egui_glfw_gl::egui;
use egui_glfw_gl::egui::{CentralPanel, Frame, Rgba, SidePanel, Style, TopBottomPanel, Ui};
use gfx_maths::Vec3;
use crate::renderer::ht_renderer;
use crate::ui_defs::chat;
use crate::worldmachine::WorldMachine;

lazy_static!{
    pub static ref SHOW_UI: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    pub static ref SHOW_DEBUG_LOCATION: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref SHOW_FPS: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref SHOW_DEBUG_LOG: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    pub static ref DEBUG_LOCATION: Arc<Mutex<Vec3>> = Arc::new(Mutex::new(Vec3::new(0.0, 0.0, 0.0)));
    pub static ref BOB_T: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    pub static ref FPS: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    pub static ref DEBUG_LOG: Arc<Mutex<OnScreenDebugLog>> = Arc::new(Mutex::new(OnScreenDebugLog {
        buffer: VecDeque::new(),
    }));

    pub static ref SUNLUST_INFO: Arc<Mutex<SunlustInfo>> = Arc::new(Mutex::new(SunlustInfo {
        powered_by_opacity: 0.0,
        show_copyright: false,
        powered_by: None,
        copyright: None,
    }));
}

pub struct SunlustInfo {
    pub powered_by_opacity: f32,
    pub show_copyright: bool,
    powered_by: Option<egui::TextureHandle>,
    copyright: Option<egui::TextureHandle>,
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

pub async fn render(renderer: &mut ht_renderer, wm: &mut WorldMachine) {
    if !SHOW_UI.load(Ordering::Relaxed) {
        return;
    }

    let (mut set_name, mut send_message) = (None, None);

    egui::Window::new("chat")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::new(30.0, -100.0))
        .fixed_size(egui::Vec2::new(400.0, 400.0))
        .frame(Frame::dark_canvas(&Style::default()))
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            let (name, message) = chat::chat(ui, wm);
            set_name = name;
            send_message = message;
        });
    if let Some(name) = set_name {
        wm.set_name(name).await;
    }
    if let Some(message) = send_message {
        wm.send_chat_message(message).await;
    }

    egui::Window::new("right debug")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-30.0, 10.0))
        .fixed_size(egui::Vec2::new(400.0, 400.0))
        .frame(Frame::dark_canvas(&Style::default()))
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            // right align
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

pub fn init_sunlust(renderer: &mut ht_renderer) {
    SidePanel::left("loading_ctx")
        .frame(Frame::none())
        .show_separator_line(false)
        .resizable(false)
        .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
            let mut sunlust_info = SUNLUST_INFO.lock().unwrap();
            let powered_by_data = crate::textures::load_image("base/textures/ui/poweredby.png").expect("failed to load base/textures/ui/poweredby.png!");
            let copyright_data = crate::textures::load_image("base/textures/ui/developedby.png").expect("failed to load base/textures/ui/developedby.png!");
            let powered_by_image = egui::ColorImage::from_rgba_unmultiplied([powered_by_data.dimensions.0 as _, powered_by_data.dimensions.1 as _], &powered_by_data.data);
            let copyright_image = egui::ColorImage::from_rgba_unmultiplied([copyright_data.dimensions.0 as _, copyright_data.dimensions.1 as _], &copyright_data.data);
            sunlust_info.powered_by.replace(ui.ctx().load_texture("powered_by", powered_by_image, egui::TextureOptions::NEAREST));
            sunlust_info.copyright.replace(ui.ctx().load_texture("copyright", copyright_image, egui::TextureOptions::NEAREST));
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

pub fn render_sunlust(renderer: &mut ht_renderer) {
    let mut sunlust_info = SUNLUST_INFO.lock().unwrap();

    let window_size = renderer.window_size;
    let poweredby_width = window_size.y / 2.0;
    let poweredby_height = poweredby_width / 2.0;

    if !sunlust_info.show_copyright {
        TopBottomPanel::bottom("powered_by")
            .frame(Frame::none())
            .show_separator_line(false)
            .resizable(false)
            .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
                if let Some(poweredby) = &sunlust_info.powered_by {
                    let image = egui::Image::new(poweredby, [poweredby_width, poweredby_height]);
                    let tint = Rgba::from_white_alpha(sunlust_info.powered_by_opacity);
                    let image = image.tint(tint);
                    ui.add(image);
                }
            });
    } else {
        TopBottomPanel::bottom("copyright")
            .frame(Frame::none())
            .show_separator_line(false)
            .resizable(false)
            .show(&renderer.backend.egui_context.lock().unwrap(), |ui| {
                if let Some(copyright) = &sunlust_info.copyright {
                    let image = egui::Image::new(copyright, [window_size.x, window_size.y]);
                    ui.add(image);
                }
            });
    }

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
    let bob_t = BOB_T.lock().unwrap();
    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.label(format!("FPS: {}", *fps as u32));
        ui.label(format!("BOB_T: {}", *bob_t));
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