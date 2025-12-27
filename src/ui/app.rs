use crate::acquisition::{start_tcp_server, DataMessage};
use crate::ui::app_state::AppState;
use crate::ui::launch_loader;
use crate::ui::menu::{render_menu_bar, MenuAction};
use crate::ui::panels::{render_config_window, render_timeline, render_topic_panel};
use crate::ui::renderer::PlotRenderer;
use crate::ui::tiles::TiPlotBehavior;
use crossbeam_channel::unbounded;
use eframe::egui;
use egui_phosphor::regular as icons;
use std::path::PathBuf;

pub struct TiPlotApp {
    state: AppState,
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

impl TiPlotApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let gl = cc.gl.as_ref().expect("OpenGL not initialized").clone();
        let renderer = PlotRenderer::new(gl);

        let renderer = std::sync::Arc::new(std::sync::Mutex::new(renderer));

        let (tx, rx) = unbounded();
        start_tcp_server(tx, cc.egui_ctx.clone());

        setup_fonts(&cc.egui_ctx);

        let layouts_dir = if let Some(storage) = cc.storage {
            if let Some(path) = storage.get_string("layouts_dir") {
                PathBuf::from(path)
            } else {
                get_default_layouts_dir()
            }
        } else {
            get_default_layouts_dir()
        };

        Self {
            state: AppState::new(rx, layouts_dir, renderer),
        }
    }

    fn handle_menu_actions(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let action = self.state.ui.menu_state.show_save_dialog(ctx);
        self.process_menu_action(action, frame);
    }

    fn process_menu_action(&mut self, action: MenuAction, frame: &mut eframe::Frame) {
        match action {
            MenuAction::SaveLayout(name) => {
                if let Err(e) = self.state.layout.save_layout(
                    name,
                    &self.state.ui.layouts_dir,
                    &self.state.panels.view3d_panel.vehicles,
                ) {
                    self.state.ui.menu_state.error_message = Some(e);
                }
            }
            MenuAction::LoadLayout(path) => {
                if let Err(e) = self
                    .state
                    .layout
                    .load_layout(path, &mut self.state.panels.view3d_panel.vehicles)
                {
                    self.state.ui.menu_state.error_message = Some(e);
                }
            }
            MenuAction::SaveData => self.save_data(),
            MenuAction::LoadData => self.load_data(frame),
            MenuAction::ClearData => self.state.clear_all(),
            MenuAction::LaunchLoader => {
                if let Err(e) = launch_loader() {
                    self.state.ui.menu_state.error_message = Some(e);
                }
            }
            MenuAction::SetInterpolationMode(mode) => {
                self.state.layout.global_interpolation_mode = mode;
                self.apply_interpolation_mode_to_all_tiles(mode);
            }
            MenuAction::None => {}
        }
    }

    fn save_data(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("tiplot_data.arrow")
            .add_filter("Arrow Files", &["arrow"])
            .save_file()
        {
            match self.state.data.data_store.save_to_arrow(&path) {
                Ok(_) => {
                    self.state.data.data_file_path = Some(path.clone());
                    println!("✓ Data saved to: {}", path.display());
                }
                Err(e) => {
                    eprintln!("✗ Failed to save data: {}", e);
                    self.state.ui.menu_state.error_message = Some(format!("Failed to save: {}", e));
                }
            }
        }
    }

    fn load_data(&mut self, frame: &mut eframe::Frame) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Arrow Files", &["arrow"])
            .pick_file()
        {
            let mut data_store = crate::core::DataStore::new();
            match data_store.load_from_arrow(&path) {
                Ok(_) => {
                    self.state.data.data_store = data_store;
                    self.state.data.data_file_path = Some(path.clone());
                    println!("✓ Data loaded from: {}", path.display());

                    self.reupload_all_traces(frame);
                    self.update_time_bounds();
                }
                Err(e) => {
                    eprintln!("✗ Failed to load data: {}", e);
                    self.state.ui.menu_state.error_message = Some(format!("Failed to load: {}", e));
                }
            }
        }
    }

    fn reupload_all_traces(&mut self, _frame: &mut eframe::Frame) {
        let mut renderer = self.state.renderer.lock().unwrap();

        for (topic, cols) in &self.state.data.data_store.topics {
            if let Some(timestamps) = cols.get("timestamp") {
                for (col_name, values) in cols {
                    if col_name == "timestamp" {
                        continue;
                    }
                    renderer.upload_trace(topic, col_name, timestamps, values);
                }
            }
        }
    }

    fn update_time_bounds(&mut self) {
        let mut min_time = f32::MAX;
        let mut max_time = f32::MIN;

        for (_topic, cols) in &self.state.data.data_store.topics {
            if let Some(timestamps) = cols.get("timestamp") {
                if !timestamps.is_empty() {
                    min_time = min_time.min(timestamps[0]);
                    max_time = max_time.max(timestamps[timestamps.len() - 1]);
                }
            }
        }

        if min_time != f32::MAX && max_time != f32::MIN {
            self.state.timeline.update_bounds(min_time, max_time);
        }
    }

    fn apply_interpolation_mode_to_all_tiles(&mut self, mode: crate::ui::tiles::InterpolationMode) {
        fn update_tiles_recursive(
            tiles: &mut egui_tiles::Tiles<crate::ui::tiles::PlotTile>,
            tile_id: egui_tiles::TileId,
            mode: crate::ui::tiles::InterpolationMode,
        ) {
            if let Some(tile) = tiles.get_mut(tile_id) {
                match tile {
                    egui_tiles::Tile::Pane(plot_tile) => {
                        plot_tile.interpolation_mode = mode;
                        plot_tile.cached_tooltip_time = f32::NEG_INFINITY;
                        plot_tile.cached_tooltip_values.clear();
                    }
                    egui_tiles::Tile::Container(container) => {
                        let children = match container {
                            egui_tiles::Container::Linear(linear) => linear.children.clone(),
                            egui_tiles::Container::Tabs(tabs) => tabs.children.clone(),
                            egui_tiles::Container::Grid(grid) => grid.children().copied().collect(),
                        };
                        for child_id in children {
                            update_tiles_recursive(tiles, child_id, mode);
                        }
                    }
                }
            }
        }

        if let Some(root_id) = self.state.layout.tree.root {
            update_tiles_recursive(&mut self.state.layout.tree.tiles, root_id, mode);
        }
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.state.timeline.is_playing = !self.state.timeline.is_playing;
            }

            if i.key_pressed(egui::Key::ArrowLeft) {
                let min_interval = self.estimate_min_sample_interval();
                self.state.timeline.current_time = (self.state.timeline.current_time
                    - min_interval)
                    .max(self.state.timeline.min_time);
                self.state.timeline.is_playing = false;
            }

            if i.key_pressed(egui::Key::ArrowRight) {
                let min_interval = self.estimate_min_sample_interval();
                self.state.timeline.current_time = (self.state.timeline.current_time
                    + min_interval)
                    .min(self.state.timeline.max_time);
                self.state.timeline.is_playing = false;
            }
        });
    }

    fn estimate_min_sample_interval(&self) -> f32 {
        let mut min_interval = f32::MAX;

        for (_topic_name, cols) in &self.state.data.data_store.topics {
            if let Some(timestamps) = cols.get("timestamp") {
                if timestamps.len() >= 2 {
                    let samples_to_check = timestamps.len().min(100);
                    for i in 1..samples_to_check {
                        let interval = (timestamps[i] - timestamps[i - 1]).abs();
                        if interval > 0.0 && interval < min_interval {
                            min_interval = interval;
                        }
                    }
                }
            }
        }

        if min_interval == f32::MAX || min_interval <= 0.0 {
            0.01
        } else {
            min_interval
        }
    }

    fn process_data(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut renderer = self.state.renderer.lock().unwrap();

        let mut received_data = false;
        let mut batches_processed = 0;
        const MAX_BATCHES_PER_FRAME: usize = 5;

        while let Ok(msg) = self.state.data.rx.try_recv() {
            match msg {
                DataMessage::Metadata(meta) => {
                    if let (Some(min), Some(max)) = (meta.min_timestamp, meta.max_timestamp) {
                        let raw_min = min as f64 / 1_000_000.0;
                        let raw_max = max as f64 / 1_000_000.0;

                        if self.state.data.data_store.start_time == 0.0 {
                            self.state.data.data_store.start_time = raw_min as f32;
                            self.state.timeline.global_min = 0.0;
                            self.state.timeline.global_max =
                                (raw_max - self.state.data.data_store.start_time as f64) as f32;
                            self.state.timeline.min_time = self.state.timeline.global_min;
                            self.state.timeline.max_time = self.state.timeline.global_max;
                            self.state.timeline.last_viewport_width =
                                self.state.timeline.global_max - self.state.timeline.global_min;
                        } else {
                            self.state.timeline.global_min = 0.0;
                            self.state.timeline.global_max =
                                (raw_max - self.state.data.data_store.start_time as f64) as f32;

                            if self.state.timeline.lock_viewport {
                                self.state.timeline.max_time = self.state.timeline.global_max;
                                self.state.timeline.min_time = self.state.timeline.max_time
                                    - self.state.timeline.last_viewport_width;
                            } else {
                                self.state.timeline.max_time = self.state.timeline.global_max;
                            }

                            if self.state.timeline.lock_to_last {
                                self.state.timeline.current_time = self.state.timeline.max_time;
                            }
                        }
                    }
                    received_data = true;
                }
                DataMessage::NewBatch(topic, batch) => {
                    self.state.data.data_store.ingest(topic.clone(), batch);

                    if let Some(cols) = self.state.data.data_store.topics.get(&topic) {
                        if let Some(timestamps) = cols.get("timestamp") {
                            for (col_name, values) in cols {
                                if col_name == "timestamp" {
                                    continue;
                                }
                                renderer.upload_trace(&topic, col_name, timestamps, values);
                            }
                        }
                    }

                    received_data = true;
                    batches_processed += 1;

                    if batches_processed >= MAX_BATCHES_PER_FRAME {
                        break;
                    }
                }
            }
        }

        if received_data {
            self.state.data.receiving_data = true;
            self.state.data.last_data_time = Some(std::time::Instant::now());
            ctx.request_repaint();
        } else {
            if let Some(last_time) = self.state.data.last_data_time {
                if last_time.elapsed().as_millis() > 500 {
                    self.state.data.receiving_data = false;
                }
            }
        }

        if self.state.data.receiving_data {
            ctx.request_repaint();
        }
    }

    fn render_top_menu_bar(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let action = render_menu_bar(
                        ui,
                        &mut self.state.ui.menu_state,
                        &self.state.ui.layouts_dir,
                        self.state.layout.global_interpolation_mode,
                    );
                    self.process_menu_action(action, frame);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(3.0);

                        let indicator_radius = 6.0;
                        let indicator_color = if self.state.data.receiving_data {
                            egui::Color32::from_rgb(255, 50, 50)
                        } else {
                            egui::Color32::from_rgb(128, 128, 128)
                        };

                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(indicator_radius * 2.0 + 4.0, indicator_radius * 2.0),
                            egui::Sense::hover(),
                        );

                        ui.painter().circle_filled(
                            rect.center(),
                            indicator_radius,
                            indicator_color,
                        );

                        response.on_hover_text(if self.state.data.receiving_data {
                            "Receiving data..."
                        } else {
                            "Idle"
                        });

                        ui.add_space(8.0);

                        let fps_text = format!("{:.0} FPS", self.state.ui.current_fps);
                        let fps_color = if self.state.ui.current_fps >= 55.0 {
                            egui::Color32::from_rgb(100, 200, 100)
                        } else if self.state.ui.current_fps >= 30.0 {
                            egui::Color32::from_rgb(200, 200, 100)
                        } else {
                            egui::Color32::from_rgb(200, 100, 100)
                        };

                        ui.label(egui::RichText::new(fps_text).color(fps_color).monospace());
                    });
                });
            });
    }

    fn render_bottom_timeline_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("timeline_panel")
            .exact_height(60.0)
            .show(ctx, |ui| {
                self.state.timeline.last_viewport_width =
                    self.state.timeline.max_time - self.state.timeline.min_time;

                render_timeline(
                    ui,
                    self.state.timeline.global_min,
                    self.state.timeline.global_max,
                    &mut self.state.timeline.min_time,
                    &mut self.state.timeline.max_time,
                    &mut self.state.timeline.current_time,
                    &mut self.state.timeline.is_playing,
                    &mut self.state.timeline.playback_speed,
                    &mut self.state.timeline.lock_to_last,
                    &mut self.state.timeline.lock_viewport,
                    &mut self.state.timeline.always_show_playback_tooltip,
                );
            });
    }

    fn render_side_panels(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.state.panels.topic_panel_collapsed {
            egui::SidePanel::left("topics_panel_collapsed")
                .exact_width(30.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        if ui
                            .add(egui::Button::new(format!("{}", icons::SIDEBAR)))
                            .on_hover_text("Show topics panel")
                            .clicked()
                        {
                            self.state.panels.topic_panel_collapsed = false;
                        }
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.add_space(20.0);
                            let painter = ui.painter();
                            let center = ui.cursor().center();
                            painter.text(
                                center,
                                egui::Align2::CENTER_CENTER,
                                "Topics",
                                egui::FontId::default(),
                                ui.style().visuals.text_color(),
                            );
                        });
                    });
                });
        } else {
            egui::SidePanel::left("topics_panel")
                .min_width(200.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(format!("{}", icons::CARET_LEFT)))
                                .on_hover_text("Hide topics panel")
                                .clicked()
                            {
                                self.state.panels.topic_panel_collapsed = true;
                            }
                        });
                    });
                    ui.separator();
                    render_topic_panel(
                        ui,
                        &self.state.data.data_store,
                        &mut self.state.panels.topic_selection,
                        &mut self.state.layout.dragged_item,
                    );
                });
        }

        if self.state.panels.view3d_panel_collapsed {
            egui::SidePanel::right("view3d_panel_collapsed")
                .exact_width(30.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        if ui
                            .add(egui::Button::new(format!("{}", icons::CUBE_FOCUS)))
                            .on_hover_text("Show 3D view panel")
                            .clicked()
                        {
                            self.state.panels.view3d_panel_collapsed = false;
                        }
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.add_space(20.0);
                            let painter = ui.painter();
                            let center = ui.cursor().center();
                            painter.text(
                                center,
                                egui::Align2::CENTER_CENTER,
                                "3D View",
                                egui::FontId::default(),
                                ui.style().visuals.text_color(),
                            );
                        });
                    });
                });
        } else {
            egui::SidePanel::right("view3d_panel")
                .default_width(ctx.available_rect().width() * 0.5)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(format!("{}", icons::CARET_RIGHT)))
                                .on_hover_text("Hide 3D view panel")
                                .clicked()
                            {
                                self.state.panels.view3d_panel_collapsed = true;
                            }

                            if ui
                                .button(egui::RichText::new(icons::GEAR))
                                .on_hover_text("Open Configuration")
                                .clicked()
                            {
                                self.state.panels.view3d_panel.show_config_window =
                                    !self.state.panels.view3d_panel.show_config_window;
                            }
                        });
                    });
                    ui.separator();
                });
        }
    }

    fn render_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut behavior = TiPlotBehavior {
                min_time: &mut self.state.timeline.min_time,
                max_time: &mut self.state.timeline.max_time,
                global_min: self.state.timeline.global_min,
                global_max: self.state.timeline.global_max,
                current_time: &mut self.state.timeline.current_time,
                data_store: &self.state.data.data_store,
                topic_selection: &self.state.panels.topic_selection,
                split_request: &mut self.state.layout.split_request,
                dragged_item: &mut self.state.layout.dragged_item,
                reset_sizes_request: &mut self.state.layout.reset_sizes_request,
                is_playing: &self.state.timeline.is_playing,
                always_show_playback_tooltip: &self.state.timeline.always_show_playback_tooltip,
                renderer: &self.state.renderer,
            };
            self.state.layout.tree.ui(&mut behavior, ui);

            if !ui.input(|i| i.pointer.primary_down()) {
                self.state.layout.dragged_item = None;
            }
        });
    }

    fn render_configuration_window(&mut self, ctx: &egui::Context) {
        render_config_window(
            ctx,
            &mut self.state.panels.view3d_panel,
            &self.state.data.data_store,
        );
    }
}

impl eframe::App for TiPlotApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.state.ui.update_fps();
        self.process_data(ctx, frame);
        ctx.request_repaint();

        self.handle_keyboard_input(ctx);
        self.state.timeline.update_playback(ctx);

        self.handle_menu_actions(ctx, frame);
        self.render_top_menu_bar(ctx, frame);
        self.render_bottom_timeline_panel(ctx);
        self.render_side_panels(ctx, frame);
        self.render_central_panel(ctx);
        self.render_configuration_window(ctx);

        self.state.layout.handle_split_request();
        self.state.layout.handle_reset_sizes_request();
    }
}

fn get_default_layouts_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("io", "tilak", "TiPlot") {
        proj_dirs.config_dir().join("layouts")
    } else {
        PathBuf::from("layouts")
    }
}
