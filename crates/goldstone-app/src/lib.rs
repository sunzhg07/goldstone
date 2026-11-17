//! Goldstone editor: an Inkscape-style GUI for many-body diagrams.

pub mod app;
mod doc_ops;
pub mod keys;
mod panels;
mod view;

use std::path::PathBuf;

/// Opens the editor window, optionally with a file.
pub fn run(path: Option<PathBuf>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Goldstone"),
        ..Default::default()
    };
    eframe::run_native(
        "Goldstone",
        options,
        Box::new(move |cc| Ok(Box::new(app::GoldstoneApp::new(cc, path)))),
    )
}
