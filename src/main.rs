mod acquisition;
mod core;
mod ui;

use eframe::egui;

fn main() -> eframe::Result {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");
    let _guard = rt.enter();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("TiPlot"),
        ..Default::default()
    };

    eframe::run_native(
        "TiPlot",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::TiPlotApp::new(cc)))),
    )
}
