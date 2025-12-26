use crate::acquisition::{start_tcp_server, DataMessage};
use crate::core::DataStore;
use crate::ui::launch_loader;
use crate::ui::layout::LayoutData;
use crate::ui::menu::{render_menu_bar, MenuAction, MenuState};
use crate::ui::panels::tabs::gltf_loader::ModelCache;
use crate::ui::panels::{
    render_config_window, render_timeline, render_topic_panel, render_view3d_panel,
    TopicPanelSelection, View3DPanel,
};
use crate::ui::renderer::PlotRenderer;
use crate::ui::tiles::{InterpolationMode, PlotTile, TiPlotBehavior};
use crossbeam_channel::{unbounded, Receiver};
use eframe::egui;
use egui_phosphor::regular as icons;
use egui_tiles::{Container, Linear, LinearDir, Tile, TileId, Tiles, Tree};
use std::path::PathBuf;

pub struct TiPlotApp {
    data_store: DataStore,
    rx: Receiver<DataMessage>,

    /// Tile tree
    tree: Tree<PlotTile>,
    dragged_item: Option<(String, String)>,

    /// Topic panel selection state
    topic_selection: TopicPanelSelection,

    /// 3D view panel state
    view3d_panel: View3DPanel,
    model_cache: ModelCache,

    // Global view state
    min_time: f32,
    max_time: f32,
    global_min: f32,
    global_max: f32,

    // Current timeline cursor position
    current_time: f32,

    // Playback controls
    is_playing: bool,
    playback_speed: f32,
    last_update_time: Option<std::time::Instant>,

    // Timeline locks
    lock_to_last: bool,
    lock_viewport: bool,
    always_show_playback_tooltip: bool,
    last_viewport_width: f32,

    split_request: Option<(TileId, LinearDir)>,
    reset_sizes_request: bool,

    topic_panel_collapsed: bool,
    view3d_panel_collapsed: bool,

    // Layout management
    menu_state: MenuState,
    layouts_dir: PathBuf,

    receiving_data: bool,
    last_data_time: Option<std::time::Instant>,

    data_file_path: Option<PathBuf>,

    // Fps tracking
    frame_times: std::collections::VecDeque<std::time::Instant>,
    current_fps: f32,

    global_interpolation_mode: InterpolationMode,
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

impl TiPlotApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        if let Some(wgpu_state) = cc.wgpu_render_state.as_ref() {
            let renderer = PlotRenderer::new(&wgpu_state.device, wgpu_state.target_format);
            wgpu_state
                .renderer
                .write()
                .callback_resources
                .insert(renderer);
        }

        let (tx, rx) = unbounded();
        start_tcp_server(tx, cc.egui_ctx.clone());

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(PlotTile::new());
        let tree = Tree::new("main_tree", root, tiles);

        let mut model_cache = ModelCache::new();

        const FIXED_WING_GLB: &[u8] = include_bytes!("../../assets/models/FixedWing.glb");
        const QUAD_COPTER_GLB: &[u8] = include_bytes!("../../assets/models/QuadCopter.glb");
        const DELTA_WING_GLB: &[u8] = include_bytes!("../../assets/models/DeltaWing.glb");

        if let Err(e) = model_cache.load_from_bytes("FixedWing", FIXED_WING_GLB) {
            eprintln!("✗ Failed to load Fixed Wing model: {}", e);
        }
        if let Err(e) = model_cache.load_from_bytes("QuadCopter", QUAD_COPTER_GLB) {
            eprintln!("✗ Failed to load Quadcopter model: {}", e);
        }
        if let Err(e) = model_cache.load_from_bytes("DeltaWing", DELTA_WING_GLB) {
            eprintln!("✗ Failed to load Delta Wing model: {}", e);
        }

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
            data_store: DataStore::new(),
            rx,
            tree,
            topic_selection: TopicPanelSelection::default(),
            view3d_panel: View3DPanel::new(),
            model_cache,
            dragged_item: None,
            min_time: 0.0,
            max_time: 10.0,
            global_min: 0.0,
            global_max: 10.0,
            current_time: 0.0,
            is_playing: false,
            playback_speed: 10.0,
            last_update_time: None,
            lock_to_last: true,
            lock_viewport: false,
            always_show_playback_tooltip: false,
            last_viewport_width: 10.0,
            split_request: None,
            reset_sizes_request: false,
            topic_panel_collapsed: false,
            view3d_panel_collapsed: true,
            menu_state: MenuState::default(),
            layouts_dir,
            receiving_data: false,
            last_data_time: None,
            data_file_path: None,
            frame_times: std::collections::VecDeque::with_capacity(60),
            current_fps: 0.0,
            global_interpolation_mode: InterpolationMode::default(),
        }
    }

    fn save_layout(&mut self, name: String) {
        let layout = LayoutData::from_tree(name, &self.tree, &self.view3d_panel.vehicles);

        match layout.save_to_file(&self.layouts_dir) {
            Ok(_) => {
                println!("✓ Layout '{}' saved successfully", layout.name);
            }
            Err(e) => {
                eprintln!("✗ Failed to save layout: {}", e);
                self.menu_state.error_message = Some(format!("Failed to save: {}", e));
            }
        }
    }

    fn load_layout(&mut self, path: PathBuf) {
        match LayoutData::load_from_file(&path) {
            Ok(layout) => match layout.to_tree() {
                Ok(tree) => {
                    self.tree = tree;
                    self.view3d_panel.vehicles = layout.vehicles;
                    println!("✓ Layout '{}' loaded successfully", layout.name);
                }
                Err(e) => {
                    eprintln!("✗ Failed to reconstruct tree: {}", e);
                    self.menu_state.error_message = Some(format!("Failed to load layout: {}", e));
                }
            },
            Err(e) => {
                eprintln!("✗ Failed to load layout: {}", e);
                self.menu_state.error_message = Some(format!("Failed to load: {}", e));
            }
        }
    }

    fn save_data(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("tiplot_data.arrow")
            .add_filter("Arrow Files", &["arrow"])
            .save_file()
        {
            match self.data_store.save_to_arrow(&path) {
                Ok(_) => {
                    self.data_file_path = Some(path.clone());
                    println!("✓ Data saved to: {}", path.display());
                }
                Err(e) => {
                    eprintln!("✗ Failed to save data: {}", e);
                    self.menu_state.error_message = Some(format!("Failed to save: {}", e));
                }
            }
        }
    }

    fn load_data(&mut self, frame: &mut eframe::Frame) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Arrow Files", &["arrow"])
            .pick_file()
        {
            let mut data_store = DataStore::new();
            match data_store.load_from_arrow(&path) {
                Ok(_) => {
                    self.data_store = data_store;
                    self.data_file_path = Some(path.clone());
                    println!("✓ Data loaded from: {}", path.display());

                    self.reupload_all_traces(frame);
                    self.update_time_bounds();
                }
                Err(e) => {
                    eprintln!("✗ Failed to load data: {}", e);
                    self.menu_state.error_message = Some(format!("Failed to load: {}", e));
                }
            }
        }
    }

    fn clear_data(&mut self) {
        self.data_store = DataStore::new();

        fn clear_tiles_recursive(
            tiles: &mut egui_tiles::Tiles<PlotTile>,
            tile_id: egui_tiles::TileId,
        ) {
            if let Some(tile) = tiles.get_mut(tile_id) {
                match tile {
                    egui_tiles::Tile::Pane(plot_tile) => {
                        plot_tile.traces.clear();
                        plot_tile.cached_tooltip_values.clear();
                        plot_tile.cached_tooltip_time = f32::NEG_INFINITY;
                    }
                    egui_tiles::Tile::Container(container) => {
                        let children = match container {
                            egui_tiles::Container::Linear(linear) => linear.children.clone(),
                            egui_tiles::Container::Tabs(tabs) => tabs.children.clone(),
                            egui_tiles::Container::Grid(grid) => grid.children().copied().collect(),
                        };
                        for child_id in children {
                            clear_tiles_recursive(tiles, child_id);
                        }
                    }
                }
            }
        }

        if let Some(root_id) = self.tree.root {
            clear_tiles_recursive(&mut self.tree.tiles, root_id);
        }

        // Reset timeline state
        self.min_time = 0.0;
        self.max_time = 10.0;
        self.global_min = 0.0;
        self.global_max = 10.0;
        self.current_time = 0.0;
        self.last_viewport_width = 10.0;

        // Reset playback
        self.is_playing = false;
        self.last_update_time = None;

        // Clear data file path
        self.data_file_path = None;

        // Reset data receiving state
        self.receiving_data = false;
        self.last_data_time = None;

        // TODO: Clear 3D view vehicles data (reset to defaults)
        for _vehicle in &mut self.view3d_panel.vehicles {}
    }

    fn reupload_all_traces(&mut self, frame: &mut eframe::Frame) {
        let wgpu_state = frame.wgpu_render_state().expect("WGPU not initialized");
        let device = &wgpu_state.device;

        let mut renderer_lock = wgpu_state.renderer.write();
        let renderer = renderer_lock
            .callback_resources
            .get_mut::<PlotRenderer>()
            .unwrap();

        for (topic, cols) in &self.data_store.topics {
            if let Some(timestamps) = cols.get("timestamp") {
                for (col_name, values) in cols {
                    if col_name == "timestamp" {
                        continue;
                    }
                    renderer.upload_trace(device, topic, col_name, timestamps, values);
                }
            }
        }
    }

    fn update_time_bounds(&mut self) {
        let mut min_time = f32::MAX;
        let mut max_time = f32::MIN;

        for (_topic, cols) in &self.data_store.topics {
            if let Some(timestamps) = cols.get("timestamp") {
                if !timestamps.is_empty() {
                    min_time = min_time.min(timestamps[0]);
                    max_time = max_time.max(timestamps[timestamps.len() - 1]);
                }
            }
        }

        if min_time != f32::MAX && max_time != f32::MIN {
            self.global_min = 0.0;
            self.global_max = max_time;
            self.min_time = 0.0;
            self.max_time = max_time;
            self.current_time = min_time;
            self.last_viewport_width = max_time;
        }
    }

    fn handle_split_request(&mut self) {
        if let Some((tile_id, direction)) = self.split_request.take() {
            let mut new_tile = PlotTile::new();
            new_tile.interpolation_mode = self.global_interpolation_mode;
            let new_tile_id = self.tree.tiles.insert_pane(new_tile);
            let parent_id = self.tree.tiles.parent_of(tile_id);

            if let Some(parent_id) = parent_id {
                let action = if let Some(Tile::Container(parent_container)) =
                    self.tree.tiles.get(parent_id)
                {
                    match parent_container {
                        Container::Linear(linear) => {
                            if linear.dir == direction {
                                linear
                                    .children
                                    .iter()
                                    .position(|&id| id == tile_id)
                                    .map(|pos| (false, pos))
                            } else {
                                linear
                                    .children
                                    .iter()
                                    .position(|&id| id == tile_id)
                                    .map(|pos| (true, pos))
                            }
                        }
                        Container::Tabs(tabs) => tabs
                            .children
                            .iter()
                            .position(|&id| id == tile_id)
                            .map(|pos| (true, pos)),
                        Container::Grid(_) => Some((true, 0)),
                    }
                } else {
                    None
                };

                if let Some((needs_new_container, pos)) = action {
                    if needs_new_container {
                        let new_container = Container::Linear(Linear {
                            children: vec![tile_id, new_tile_id],
                            dir: direction,
                            ..Default::default()
                        });
                        let container_id = self.tree.tiles.insert_container(new_container);

                        if let Some(Tile::Container(parent_container)) =
                            self.tree.tiles.get_mut(parent_id)
                        {
                            match parent_container {
                                Container::Linear(linear) => {
                                    linear.children[pos] = container_id;
                                }
                                Container::Tabs(tabs) => {
                                    tabs.children[pos] = container_id;
                                }
                                Container::Grid(_) => {}
                            }
                        }
                    } else {
                        if let Some(Tile::Container(Container::Linear(linear))) =
                            self.tree.tiles.get_mut(parent_id)
                        {
                            linear.children.insert(pos + 1, new_tile_id);
                        }
                    }
                }
            } else {
                let new_container = Container::Linear(Linear {
                    children: vec![tile_id, new_tile_id],
                    dir: direction,
                    ..Default::default()
                });
                let container_id = self.tree.tiles.insert_container(new_container);
                self.tree.root = Some(container_id);
            }
        }
    }

    fn handle_reset_sizes_request(&mut self) {
        if self.reset_sizes_request {
            self.reset_sizes_request = false;

            fn reset_container_shares(tiles: &mut Tiles<PlotTile>, tile_id: TileId) {
                let children_to_process = if let Some(Tile::Container(container)) =
                    tiles.get(tile_id)
                {
                    match container {
                        Container::Linear(linear) => Some(linear.children.clone()),
                        Container::Tabs(tabs) => Some(tabs.children.clone()),
                        Container::Grid(grid) => Some(grid.children().copied().collect::<Vec<_>>()),
                    }
                } else {
                    None
                };

                if let Some(children) = children_to_process {
                    if let Some(Tile::Container(container)) = tiles.get_mut(tile_id) {
                        match container {
                            Container::Linear(linear) => {
                                for &child_id in &children {
                                    linear.shares.set_share(child_id, 1.0);
                                }
                            }
                            Container::Tabs(_) => {
                                // Nothing to do for tabs
                            }
                            Container::Grid(grid) => {
                                let num_children = children.len();
                                if num_children > 0 {
                                    let cols = (num_children as f32).sqrt().ceil() as usize;
                                    grid.col_shares = vec![1.0; cols];
                                    grid.row_shares = vec![1.0; (num_children + cols - 1) / cols];
                                }
                            }
                        }
                    }

                    for &child_id in &children {
                        reset_container_shares(tiles, child_id);
                    }
                }
            }

            if let Some(root_id) = self.tree.root {
                reset_container_shares(&mut self.tree.tiles, root_id);
            }
        }
    }

    fn estimate_min_sample_interval(&self) -> f32 {
        let mut min_interval = f32::MAX;

        for (_topic_name, cols) in &self.data_store.topics {
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

    fn process_data(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let wgpu_state = frame.wgpu_render_state().expect("WGPU not initialized");
        let device = &wgpu_state.device;

        let mut renderer_lock = wgpu_state.renderer.write();
        let renderer = renderer_lock
            .callback_resources
            .get_mut::<PlotRenderer>()
            .unwrap();

        let mut received_data = false;
        let mut batches_processed = 0;
        const MAX_BATCHES_PER_FRAME: usize = 5;

        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                DataMessage::Metadata(meta) => {
                    if let (Some(min), Some(max)) = (meta.min_timestamp, meta.max_timestamp) {
                        let raw_min = min as f64 / 1_000_000.0;
                        let raw_max = max as f64 / 1_000_000.0;

                        if self.data_store.start_time == 0.0 {
                            self.data_store.start_time = raw_min as f32;
                            self.global_min = 0.0;
                            self.global_max = (raw_max - self.data_store.start_time as f64) as f32;
                            self.min_time = self.global_min;
                            self.max_time = self.global_max;
                            self.last_viewport_width = self.global_max - self.global_min;
                        } else {
                            self.global_min = 0.0;
                            self.global_max = (raw_max - self.data_store.start_time as f64) as f32;

                            if self.lock_viewport {
                                self.max_time = self.global_max;
                                self.min_time = self.max_time - self.last_viewport_width;
                            } else {
                                self.max_time = self.global_max;
                            }

                            if self.lock_to_last {
                                self.current_time = self.max_time;
                            }
                        }
                    }
                    received_data = true;
                }
                DataMessage::NewBatch(topic, batch) => {
                    self.data_store.ingest(topic.clone(), batch);

                    if let Some(cols) = self.data_store.topics.get(&topic) {
                        if let Some(timestamps) = cols.get("timestamp") {
                            for (col_name, values) in cols {
                                if col_name == "timestamp" {
                                    continue;
                                }
                                renderer.upload_trace(device, &topic, col_name, timestamps, values);
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
            self.receiving_data = true;
            self.last_data_time = Some(std::time::Instant::now());
            ctx.request_repaint();
        } else {
            if let Some(last_time) = self.last_data_time {
                if last_time.elapsed().as_millis() > 500 {
                    self.receiving_data = false;
                }
            }
        }

        if self.receiving_data {
            ctx.request_repaint();
        }
    }

    fn update_fps(&mut self) {
        let now = std::time::Instant::now();
        self.frame_times.push_back(now);

        while self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        if self.frame_times.len() >= 2 {
            let elapsed = now.duration_since(self.frame_times[0]).as_secs_f32();
            if elapsed > 0.0 {
                self.current_fps = (self.frame_times.len() - 1) as f32 / elapsed;
            }
        }
    }

    fn apply_interpolation_mode_to_all_tiles(&mut self, mode: InterpolationMode) {
        fn update_tiles_recursive(
            tiles: &mut egui_tiles::Tiles<PlotTile>,
            tile_id: egui_tiles::TileId,
            mode: InterpolationMode,
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

        if let Some(root_id) = self.tree.root {
            update_tiles_recursive(&mut self.tree.tiles, root_id, mode);
        }
    }
}

impl eframe::App for TiPlotApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update_fps();
        self.process_data(ctx, frame);

        // TODO: expose this as global setting: continuous rendering
        ctx.request_repaint();

        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.is_playing = !self.is_playing;
            }

            if i.key_pressed(egui::Key::ArrowLeft) {
                let min_interval = self.estimate_min_sample_interval();
                self.current_time = (self.current_time - min_interval).max(self.min_time);
                self.is_playing = false;
            }

            if i.key_pressed(egui::Key::ArrowRight) {
                let min_interval = self.estimate_min_sample_interval();
                self.current_time = (self.current_time + min_interval).min(self.max_time);
                self.is_playing = false;
            }
        });

        if self.is_playing {
            let now = std::time::Instant::now();
            if let Some(last_time) = self.last_update_time {
                let elapsed = now.duration_since(last_time).as_secs_f32();
                let time_delta = elapsed * self.playback_speed;
                self.current_time += time_delta;
                if self.current_time > self.max_time {
                    self.current_time = self.min_time;
                }
            }
            self.last_update_time = Some(now);
            ctx.request_repaint();
        } else {
            self.last_update_time = None;
        }

        match self.menu_state.show_save_dialog(ctx) {
            MenuAction::SaveLayout(name) => {
                self.save_layout(name);
            }
            _ => {}
        }

        egui::TopBottomPanel::top("menu_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let action = render_menu_bar(
                        ui,
                        &mut self.menu_state,
                        &self.layouts_dir,
                        self.global_interpolation_mode,
                    );
                    match action {
                        MenuAction::SaveLayout(name) => {
                            self.save_layout(name);
                        }
                        MenuAction::LoadLayout(path) => {
                            self.load_layout(path);
                        }
                        MenuAction::SaveData => {
                            self.save_data();
                        }
                        MenuAction::LoadData => {
                            self.load_data(frame);
                        }
                        MenuAction::ClearData => {
                            self.clear_data();
                        }
                        MenuAction::LaunchLoader => {
                            if let Ok(cmd) = std::env::var("TIPLOT_LOADER_COMMAND") {
                                #[cfg(unix)]
                                {
                                    match Command::new("sh").arg("-c").arg(&cmd).spawn() {
                                        Ok(_) => {
                                            eprintln!("Launched loader: {}", cmd);
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to launch loader: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        MenuAction::SetInterpolationMode(mode) => {
                            self.global_interpolation_mode = mode;
                            self.apply_interpolation_mode_to_all_tiles(mode);
                        }
                        MenuAction::None => {}
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(3.0);

                        let indicator_radius = 6.0;
                        let indicator_color = if self.receiving_data {
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

                        response.on_hover_text(if self.receiving_data {
                            "Receiving data..."
                        } else {
                            "Idle"
                        });

                        ui.add_space(8.0);

                        // FPS Indicator
                        let fps_text = format!("{:.0} FPS", self.current_fps);
                        let fps_color = if self.current_fps >= 55.0 {
                            egui::Color32::from_rgb(100, 200, 100)
                        } else if self.current_fps >= 30.0 {
                            egui::Color32::from_rgb(200, 200, 100)
                        } else {
                            egui::Color32::from_rgb(200, 100, 100)
                        };

                        ui.label(egui::RichText::new(fps_text).color(fps_color).monospace());
                    });
                });
            });

        egui::TopBottomPanel::bottom("timeline_panel")
            .exact_height(60.0)
            .show(ctx, |ui| {
                self.last_viewport_width = self.max_time - self.min_time;

                render_timeline(
                    ui,
                    self.global_min,
                    self.global_max,
                    &mut self.min_time,
                    &mut self.max_time,
                    &mut self.current_time,
                    &mut self.is_playing,
                    &mut self.playback_speed,
                    &mut self.lock_to_last,
                    &mut self.lock_viewport,
                    &mut self.always_show_playback_tooltip,
                );
            });

        if self.topic_panel_collapsed {
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
                            self.topic_panel_collapsed = false;
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
                                self.topic_panel_collapsed = true;
                            }
                        });
                    });
                    ui.separator();
                    render_topic_panel(
                        ui,
                        &self.data_store,
                        &mut self.topic_selection,
                        &mut self.dragged_item,
                    );
                });
        }

        if self.view3d_panel_collapsed {
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
                            self.view3d_panel_collapsed = false;
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
                                self.view3d_panel_collapsed = true;
                            }

                            if ui
                                .button(egui::RichText::new(icons::GEAR))
                                .on_hover_text("Open Configuration")
                                .clicked()
                            {
                                self.view3d_panel.show_config_window =
                                    !self.view3d_panel.show_config_window;
                            }
                        });
                    });
                    ui.separator();
                    render_view3d_panel(
                        ui,
                        frame,
                        &mut self.view3d_panel,
                        &self.data_store,
                        self.current_time,
                        &self.model_cache,
                    );
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut behavior = TiPlotBehavior {
                min_time: &mut self.min_time,
                max_time: &mut self.max_time,
                global_min: self.global_min,
                global_max: self.global_max,
                current_time: &mut self.current_time,
                data_store: &self.data_store,
                topic_selection: &self.topic_selection,
                split_request: &mut self.split_request,
                dragged_item: &mut self.dragged_item,
                reset_sizes_request: &mut self.reset_sizes_request,
                is_playing: &self.is_playing,
                always_show_playback_tooltip: &self.always_show_playback_tooltip,
            };
            self.tree.ui(&mut behavior, ui);

            if !ui.input(|i| i.pointer.primary_down()) {
                self.dragged_item = None;
            }
        });

        // Render the configuration window after all panels
        render_config_window(ctx, &mut self.view3d_panel, &self.data_store);

        self.handle_split_request();

        self.handle_reset_sizes_request();
    }
}

fn get_default_layouts_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("io", "tilak", "TiPlot") {
        proj_dirs.config_dir().join("layouts")
    } else {
        PathBuf::from("layouts")
    }
}
