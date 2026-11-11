#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let icon = load_icon();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title("Sonixx")
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([900.0, 580.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "Sonixx",
        options,
        Box::new(|cc| Ok(Box::new(app::SonixxApp::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../../../assets/sonixx_logo.png")) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            egui::IconData {
                rgba: rgba.as_raw().clone(),
                width: rgba.width(),
                height: rgba.height(),
            }
        }
        Err(_) => egui::IconData {
            rgba: vec![0, 0, 0, 0],
            width: 1,
            height: 1,
        },
    }
}
