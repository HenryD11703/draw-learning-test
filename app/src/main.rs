#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod brush;
mod canvas;
mod content;
mod curve;
mod grading;
mod lessons;
mod painting;
mod pressure;
mod progress;
mod smoothing;
mod theme;
mod view;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let shot_path = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Trazo Academia — aprende a dibujar y a usar Krita")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1000.0, 640.0]);
    if shot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };

    eframe::run_native(
        "Trazo Academia",
        options,
        Box::new(move |cc| {
            let shot = shot_path.map(|path| app::Screenshot { path, requested: false, done: Arc::new(AtomicBool::new(false)) });
            Ok(Box::new(app::TrazoApp::new(cc, shot)))
        }),
    )
}
