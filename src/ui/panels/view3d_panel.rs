use crate::core::DataStore;
use crate::ui::panels::scene::config::{render_configuration_tab, VehicleConfig};
use eframe::egui;

#[derive(Clone)]
pub struct View3DPanel {
    pub vehicles: Vec<VehicleConfig>,
    pub show_config_window: bool,
}

impl View3DPanel {
    pub fn new() -> Self {
        let default_vehicle = VehicleConfig::default();
        Self {
            vehicles: vec![default_vehicle],
            show_config_window: false,
        }
    }
}

impl Default for View3DPanel {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_config_window(
    ctx: &egui::Context,
    panel_state: &mut View3DPanel,
    data_store: &DataStore,
) {
    egui::Window::new("Vehicle Configuration")
        .id(egui::Id::new("vehicle_config_window"))
        .open(&mut panel_state.show_config_window)
        .default_width(500.0)
        .default_height(600.0)
        .resizable(true)
        .collapsible(false)
        .scroll([false, true])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            render_configuration_tab(ui, &mut panel_state.vehicles, data_store);
        });
}
