use crate::core::DataStore;
use crate::ui::layout::LayoutData;
use crate::ui::panels::tabs::config::VehicleConfig;
use crate::ui::panels::tabs::gltf_loader::ModelCache;
use crate::ui::panels::{TopicPanelSelection, View3DPanel};
use crate::ui::renderer::PlotRenderer;
use crate::ui::tiles::{InterpolationMode, PlotTile};
use crossbeam_channel::Receiver;
use egui_tiles::{LinearDir, TileId, Tiles, Tree};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct TimelineState {
    pub min_time: f32,
    pub max_time: f32,
    pub global_min: f32,
    pub global_max: f32,
    pub current_time: f32,

    // Playback
    pub is_playing: bool,
    pub playback_speed: f32,
    pub last_update_time: Option<std::time::Instant>,

    // Timeline behavior
    pub lock_to_last: bool,
    pub lock_viewport: bool,
    pub always_show_playback_tooltip: bool,
    pub last_viewport_width: f32,
}

impl TimelineState {
    pub fn new() -> Self {
        Self {
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
        }
    }

    pub fn reset(&mut self) {
        self.min_time = 0.0;
        self.max_time = 10.0;
        self.global_min = 0.0;
        self.global_max = 10.0;
        self.current_time = 0.0;
        self.last_viewport_width = 10.0;
        self.is_playing = false;
        self.last_update_time = None;
    }

    pub fn update_bounds(&mut self, min: f32, max: f32) {
        self.global_min = 0.0;
        self.global_max = max;
        self.min_time = 0.0;
        self.max_time = max;
        self.current_time = min;
        self.last_viewport_width = max;
    }

    pub fn update_playback(&mut self, ctx: &egui::Context) {
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
    }
}

impl Default for TimelineState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PanelState {
    pub topic_panel_collapsed: bool,
    pub view3d_panel_collapsed: bool,
    pub topic_selection: TopicPanelSelection,
    pub view3d_panel: View3DPanel,
}

impl PanelState {
    pub fn new() -> Self {
        Self {
            topic_panel_collapsed: false,
            view3d_panel_collapsed: true,
            topic_selection: TopicPanelSelection::default(),
            view3d_panel: View3DPanel::new(),
        }
    }
}

impl Default for PanelState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DataState {
    pub data_store: DataStore,
    pub rx: Receiver<crate::acquisition::DataMessage>,
    pub receiving_data: bool,
    pub last_data_time: Option<std::time::Instant>,
    pub data_file_path: Option<PathBuf>,
}

impl DataState {
    pub fn new(rx: Receiver<crate::acquisition::DataMessage>) -> Self {
        Self {
            data_store: DataStore::new(),
            rx,
            receiving_data: false,
            last_data_time: None,
            data_file_path: None,
        }
    }

    pub fn clear(&mut self) {
        self.data_store = DataStore::new();
        self.data_file_path = None;
        self.receiving_data = false;
        self.last_data_time = None;
    }
}

pub struct LayoutState {
    pub tree: Tree<PlotTile>,
    pub dragged_item: Option<(String, String)>,
    pub split_request: Option<(TileId, LinearDir)>,
    pub reset_sizes_request: bool,
    pub global_interpolation_mode: InterpolationMode,
}

impl LayoutState {
    pub fn new() -> Self {
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(PlotTile::new());
        let tree = Tree::new("main_tree", root, tiles);

        Self {
            tree,
            dragged_item: None,
            split_request: None,
            reset_sizes_request: false,
            global_interpolation_mode: InterpolationMode::default(),
        }
    }

    pub fn save_layout(
        &self,
        name: String,
        layouts_dir: &PathBuf,
        vehicles: &[VehicleConfig],
    ) -> Result<(), String> {
        let layout = LayoutData::from_tree(name, &self.tree, vehicles);

        match layout.save_to_file(layouts_dir) {
            Ok(_) => {
                println!("✓ Layout '{}' saved successfully", layout.name);
                Ok(())
            }
            Err(e) => {
                let msg = format!("Failed to save: {}", e);
                eprintln!("✗ {}", msg);
                Err(msg)
            }
        }
    }

    pub fn load_layout(
        &mut self,
        path: PathBuf,
        vehicles: &mut Vec<VehicleConfig>,
    ) -> Result<(), String> {
        match LayoutData::load_from_file(&path) {
            Ok(layout) => match layout.to_tree() {
                Ok(tree) => {
                    self.tree = tree;
                    *vehicles = layout.vehicles;
                    println!("✓ Layout '{}' loaded successfully", layout.name);
                    Ok(())
                }
                Err(e) => {
                    let msg = format!("Failed to reconstruct tree: {}", e);
                    eprintln!("✗ {}", msg);
                    Err(msg)
                }
            },
            Err(e) => {
                let msg = format!("Failed to load layout: {}", e);
                eprintln!("✗ {}", msg);
                Err(msg)
            }
        }
    }

    pub fn clear_all_traces(&mut self) {
        fn clear_tiles_recursive(tiles: &mut Tiles<PlotTile>, tile_id: TileId) {
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
    }

    pub fn handle_split_request(&mut self) {
        if let Some((tile_id, direction)) = self.split_request.take() {
            let mut new_tile = PlotTile::new();
            new_tile.interpolation_mode = self.global_interpolation_mode;
            let new_tile_id = self.tree.tiles.insert_pane(new_tile);
            let parent_id = self.tree.tiles.parent_of(tile_id);

            if let Some(parent_id) = parent_id {
                let action = if let Some(egui_tiles::Tile::Container(parent_container)) =
                    self.tree.tiles.get(parent_id)
                {
                    match parent_container {
                        egui_tiles::Container::Linear(linear) => {
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
                        egui_tiles::Container::Tabs(tabs) => tabs
                            .children
                            .iter()
                            .position(|&id| id == tile_id)
                            .map(|pos| (true, pos)),
                        egui_tiles::Container::Grid(_) => Some((true, 0)),
                    }
                } else {
                    None
                };

                if let Some((needs_new_container, pos)) = action {
                    if needs_new_container {
                        let new_container = egui_tiles::Container::Linear(egui_tiles::Linear {
                            children: vec![tile_id, new_tile_id],
                            dir: direction,
                            ..Default::default()
                        });
                        let container_id = self.tree.tiles.insert_container(new_container);

                        if let Some(egui_tiles::Tile::Container(parent_container)) =
                            self.tree.tiles.get_mut(parent_id)
                        {
                            match parent_container {
                                egui_tiles::Container::Linear(linear) => {
                                    linear.children[pos] = container_id;
                                }
                                egui_tiles::Container::Tabs(tabs) => {
                                    tabs.children[pos] = container_id;
                                }
                                egui_tiles::Container::Grid(_) => {}
                            }
                        }
                    } else {
                        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(
                            linear,
                        ))) = self.tree.tiles.get_mut(parent_id)
                        {
                            linear.children.insert(pos + 1, new_tile_id);
                        }
                    }
                }
            } else {
                let new_container = egui_tiles::Container::Linear(egui_tiles::Linear {
                    children: vec![tile_id, new_tile_id],
                    dir: direction,
                    ..Default::default()
                });
                let container_id = self.tree.tiles.insert_container(new_container);
                self.tree.root = Some(container_id);
            }
        }
    }

    pub fn handle_reset_sizes_request(&mut self) {
        if !self.reset_sizes_request {
            return;
        }
        self.reset_sizes_request = false;

        fn reset_container_shares(tiles: &mut Tiles<PlotTile>, tile_id: TileId) {
            let children_to_process =
                if let Some(egui_tiles::Tile::Container(container)) = tiles.get(tile_id) {
                    match container {
                        egui_tiles::Container::Linear(linear) => Some(linear.children.clone()),
                        egui_tiles::Container::Tabs(tabs) => Some(tabs.children.clone()),
                        egui_tiles::Container::Grid(grid) => {
                            Some(grid.children().copied().collect::<Vec<_>>())
                        }
                    }
                } else {
                    None
                };

            if let Some(children) = children_to_process {
                if let Some(egui_tiles::Tile::Container(container)) = tiles.get_mut(tile_id) {
                    match container {
                        egui_tiles::Container::Linear(linear) => {
                            for &child_id in &children {
                                linear.shares.set_share(child_id, 1.0);
                            }
                        }
                        egui_tiles::Container::Tabs(_) => {}
                        egui_tiles::Container::Grid(grid) => {
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

impl Default for LayoutState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct UIState {
    pub menu_state: crate::ui::menu::MenuState,
    pub layouts_dir: PathBuf,
    pub frame_times: std::collections::VecDeque<std::time::Instant>,
    pub current_fps: f32,
}

impl UIState {
    pub fn new(layouts_dir: PathBuf) -> Self {
        Self {
            menu_state: crate::ui::menu::MenuState::default(),
            layouts_dir,
            frame_times: std::collections::VecDeque::with_capacity(60),
            current_fps: 0.0,
        }
    }

    pub fn update_fps(&mut self) {
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
}

pub struct AppState {
    pub timeline: TimelineState,
    pub panels: PanelState,
    pub data: DataState,
    pub layout: LayoutState,
    pub ui: UIState,
    pub model_cache: ModelCache,
    pub renderer: Arc<Mutex<PlotRenderer>>,
}

impl AppState {
    pub fn new(
        rx: Receiver<crate::acquisition::DataMessage>,
        layouts_dir: PathBuf,
        model_cache: ModelCache,
        renderer: Arc<Mutex<PlotRenderer>>,
    ) -> Self {
        Self {
            timeline: TimelineState::new(),
            panels: PanelState::new(),
            data: DataState::new(rx),
            layout: LayoutState::new(),
            ui: UIState::new(layouts_dir),
            model_cache,
            renderer,
        }
    }

    pub fn clear_all(&mut self) {
        self.data.clear();
        self.layout.clear_all_traces();
        self.timeline.reset();
    }
}
