use crate::ui::panels::scene::config::VehicleConfig;
use crate::ui::tiles::PlotTile;
use anyhow::{Context, Result};
use egui_tiles::{Container, Tile, Tiles, Tree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializablePlotTile {
    pub traces: Vec<SerializableTrace>,
    pub show_legend: bool,
    pub show_hover_tooltip: bool,
    pub scatter_mode: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableTrace {
    pub topic: String,
    pub col: String,
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableTile {
    pub id: String,
    pub kind: SerializableTileKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SerializableTileKind {
    Pane(SerializablePlotTile),
    Container(SerializableContainer),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableContainer {
    pub kind: String, // "Linear", "Tabs"
    pub children: Vec<String>,
    pub direction: Option<String>, // "Horizontal", "Vertical"
    pub shares: Option<Vec<f32>>,
    pub active_tab: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutData {
    pub name: String,
    pub version: u32,
    pub root_id: Option<String>,
    pub tiles: HashMap<String, SerializableTile>,
    pub vehicles: Vec<VehicleConfig>,
}

impl LayoutData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            version: 1,
            root_id: None,
            tiles: HashMap::new(),
            vehicles: Vec::new(),
        }
    }

    pub fn save_to_file(&self, layouts_dir: &Path) -> Result<()> {
        fs::create_dir_all(layouts_dir).context("Failed to create layouts directory")?;

        let filename = format!("{}.json", sanitize_filename(&self.name));
        let path = layouts_dir.join(filename);

        let json = serde_json::to_string_pretty(self).context("Failed to serialize layout")?;
        fs::write(&path, json).context("Failed to write layout file")?;

        Ok(())
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let json = fs::read_to_string(path).context("Failed to read layout file")?;
        let layout: LayoutData =
            serde_json::from_str(&json).context("Failed to deserialize layout")?;
        Ok(layout)
    }

    pub fn list_layouts(layouts_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
        if !layouts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut layouts = Vec::new();

        for entry in fs::read_dir(layouts_dir).context("Failed to read layouts directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(layout) = Self::load_from_file(&path) {
                    layouts.push((layout.name, path));
                }
            }
        }

        layouts.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(layouts)
    }

    pub fn from_tree(name: String, tree: &Tree<PlotTile>, vehicles: &[VehicleConfig]) -> Self {
        let mut layout = Self::new(name);
        layout.vehicles = vehicles.to_vec();

        if let Some(root_id) = tree.root {
            layout.root_id = Some(format!("{:?}", root_id));
            Self::serialize_tile_recursive(root_id, &tree.tiles, &mut layout.tiles);
        }

        layout
    }

    fn serialize_tile_recursive(
        tile_id: egui_tiles::TileId,
        tiles: &Tiles<PlotTile>,
        output: &mut HashMap<String, SerializableTile>,
    ) {
        let id_str = format!("{:?}", tile_id);

        if let Some(tile) = tiles.get(tile_id) {
            let kind = match tile {
                Tile::Pane(plot_tile) => {
                    let traces = plot_tile
                        .traces
                        .iter()
                        .map(|t| SerializableTrace {
                            topic: t.topic.clone(),
                            col: t.col.clone(),
                            color: t.color,
                        })
                        .collect();

                    SerializableTileKind::Pane(SerializablePlotTile {
                        traces,
                        show_legend: plot_tile.show_legend,
                        show_hover_tooltip: plot_tile.show_hover_tooltip,
                        scatter_mode: plot_tile.scatter_mode,
                    })
                }
                Tile::Container(container) => {
                    let (kind, children, direction, shares, active_tab) = match container {
                        Container::Linear(linear) => {
                            let dir = match linear.dir {
                                egui_tiles::LinearDir::Horizontal => "Horizontal",
                                egui_tiles::LinearDir::Vertical => "Vertical",
                            };

                            let shares_vec: Vec<f32> =
                                linear.shares.iter().map(|(_, &share)| share).collect();

                            (
                                "Linear",
                                &linear.children,
                                Some(dir.to_string()),
                                Some(shares_vec),
                                None,
                            )
                        }
                        Container::Tabs(tabs) => {
                            let active_idx = tabs.active.and_then(|active_id| {
                                tabs.children.iter().position(|&id| id == active_id)
                            });

                            ("Tabs", &tabs.children, None, None, active_idx)
                        }
                        Container::Grid(_) => {
                            return;
                        }
                    };

                    for &child_id in children {
                        Self::serialize_tile_recursive(child_id, tiles, output);
                    }

                    SerializableTileKind::Container(SerializableContainer {
                        kind: kind.to_string(),
                        children: children.iter().map(|id| format!("{:?}", id)).collect(),
                        direction,
                        shares,
                        active_tab,
                    })
                }
            };

            output.insert(id_str.clone(), SerializableTile { id: id_str, kind });
        }
    }

    pub fn to_tree(&self) -> Result<Tree<PlotTile>> {
        let mut tiles = Tiles::default();
        let mut id_map: HashMap<String, egui_tiles::TileId> = HashMap::new();

        for (id_str, ser_tile) in &self.tiles {
            if let SerializableTileKind::Pane(plot_tile) = &ser_tile.kind {
                let mut tile = PlotTile::new();
                tile.show_legend = plot_tile.show_legend;
                tile.show_hover_tooltip = plot_tile.show_hover_tooltip;
                tile.scatter_mode = plot_tile.scatter_mode;

                for trace in &plot_tile.traces {
                    tile.add_trace(trace.topic.clone(), trace.col.clone(), trace.color);
                }

                let tile_id = tiles.insert_pane(tile);
                id_map.insert(id_str.clone(), tile_id);
            }
        }

        let max_iterations = self.tiles.len();
        for _ in 0..max_iterations {
            let mut made_progress = false;

            for (id_str, ser_tile) in &self.tiles {
                if id_map.contains_key(id_str) {
                    continue;
                }

                if let SerializableTileKind::Container(container) = &ser_tile.kind {
                    let children: Vec<egui_tiles::TileId> = container
                        .children
                        .iter()
                        .filter_map(|child_str| id_map.get(child_str).copied())
                        .collect();

                    if children.len() != container.children.len() {
                        continue;
                    }

                    if children.is_empty() {
                        continue;
                    }

                    let container_id = match container.kind.as_str() {
                        "Linear" => {
                            let dir = match container.direction.as_deref() {
                                Some("Horizontal") => egui_tiles::LinearDir::Horizontal,
                                _ => egui_tiles::LinearDir::Vertical,
                            };

                            let linear = egui_tiles::Linear {
                                children,
                                dir,
                                shares: egui_tiles::Shares::default(),
                            };
                            tiles.insert_container(linear)
                        }
                        "Tabs" => {
                            let active = container
                                .active_tab
                                .and_then(|idx| children.get(idx).copied());

                            let tabs = egui_tiles::Tabs { children, active };
                            tiles.insert_container(tabs)
                        }
                        _ => continue,
                    };

                    id_map.insert(id_str.clone(), container_id);
                    made_progress = true;
                }
            }

            if !made_progress {
                break;
            }
        }

        let root = self
            .root_id
            .as_ref()
            .and_then(|id_str| id_map.get(id_str).copied())
            .context("No root tile found in layout")?;

        Ok(Tree::new("main_tree", root, tiles))
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
