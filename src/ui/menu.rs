use crate::ui::{is_loader_available, layout::LayoutData, tiles::InterpolationMode};
use eframe::egui;
use egui_phosphor::regular as icons;
use std::path::PathBuf;

#[derive(Default)]
pub struct MenuState {
    pub save_dialog_open: bool,
    pub save_layout_name: String,
    pub error_message: Option<String>,
}

pub enum MenuAction {
    None,
    SaveLayout(String),
    LoadLayout(PathBuf),
    SaveData,
    LoadData,
    ClearData,
    LaunchLoader,
    SetInterpolationMode(InterpolationMode),
}

impl MenuState {
    pub fn show_save_dialog(&mut self, ctx: &egui::Context) -> MenuAction {
        if !self.save_dialog_open {
            return MenuAction::None;
        }

        let mut action = MenuAction::None;
        let mut keep_open = true;

        egui::Window::new("Save Layout")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(10.0);

                ui.label("Layout Name:");
                let response = ui.text_edit_singleline(&mut self.save_layout_name);

                if self.save_dialog_open {
                    response.request_focus();
                }

                ui.add_space(10.0);

                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, err);
                    ui.add_space(5.0);
                }

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        keep_open = false;
                        self.save_layout_name.clear();
                        self.error_message = None;
                    }

                    if ui.button("Save").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        if self.save_layout_name.trim().is_empty() {
                            self.error_message = Some("Layout name cannot be empty".to_string());
                        } else {
                            action = MenuAction::SaveLayout(self.save_layout_name.clone());
                            keep_open = false;
                            self.save_layout_name.clear();
                            self.error_message = None;
                        }
                    }
                });

                ui.add_space(5.0);
            });

        if !keep_open {
            self.save_dialog_open = false;
        }

        action
    }
}

pub fn render_menu_bar(
    ui: &mut egui::Ui,
    menu_state: &mut MenuState,
    layouts_dir: &PathBuf,
    current_interpolation_mode: InterpolationMode,
) -> MenuAction {
    let mut action = MenuAction::None;

    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
            if is_loader_available() {
                if ui
                    .button(format!("{} Launch Loader", icons::ROCKET_LAUNCH))
                    .clicked()
                {
                    action = MenuAction::LaunchLoader;
                    ui.close_menu();
                }
                ui.separator();
            }

            ui.menu_button(format!("{} Data", icons::DATABASE), |ui| {
                if ui
                    .button(format!("{} Save Data...", icons::FLOPPY_DISK))
                    .clicked()
                {
                    action = MenuAction::SaveData;
                    ui.close_menu();
                }

                if ui
                    .button(format!("{} Load Data...", icons::FOLDER_OPEN))
                    .clicked()
                {
                    action = MenuAction::LoadData;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button(format!("{} Clear", icons::TRASH)).clicked() {
                    action = MenuAction::ClearData;
                    ui.close_menu();
                }
            });

            ui.separator();

            if ui.button(format!("{} Exit", icons::SIGN_OUT)).clicked() {
                std::process::exit(0);
            }
        });

        ui.menu_button("Edit", |ui| {
            ui.menu_button(
                format!("{} Interpolation Method", icons::CHART_LINE),
                |ui| {
                    let modes = [
                        (InterpolationMode::PreviousPoint, "Previous Point"),
                        (InterpolationMode::Linear, "Linear"),
                        (InterpolationMode::NextPoint, "Next Point"),
                    ];

                    for (mode, label) in modes {
                        if ui
                            .selectable_label(current_interpolation_mode == mode, label)
                            .clicked()
                        {
                            action = MenuAction::SetInterpolationMode(mode);
                            ui.close_menu();
                        }
                    }
                },
            );
        });

        ui.menu_button("Layout", |ui| {
            if ui
                .button(format!("{} Save Layout", icons::FLOPPY_DISK))
                .clicked()
            {
                menu_state.save_dialog_open = true;
                ui.close_menu();
            }

            ui.separator();

            ui.menu_button(format!("{} Load Layout", icons::FOLDER_OPEN), |ui| {
                match LayoutData::list_layouts(layouts_dir) {
                    Ok(layouts) => {
                        if layouts.is_empty() {
                            ui.label(egui::RichText::new("No saved layouts").italics().weak());
                        } else {
                            for (name, path) in layouts {
                                if ui.button(&name).clicked() {
                                    action = MenuAction::LoadLayout(path);
                                    ui.close_menu();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        ui.label(
                            egui::RichText::new(format!("Error: {}", e)).color(egui::Color32::RED),
                        );
                    }
                }
            });
        });
    });

    action
}
