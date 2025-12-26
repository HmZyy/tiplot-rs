use crate::core::DataStore;
use eframe::egui;
use rustc_hash::FxHashSet;

fn fuzzy_match(target: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let mut query_chars = query.chars();
    let mut current_query_char = match query_chars.next() {
        Some(c) => c,
        None => return true,
    };

    for target_char in target.chars() {
        if target_char == current_query_char {
            current_query_char = match query_chars.next() {
                Some(c) => c,
                None => return true,
            };
        }
    }

    false
}

#[derive(Default, Clone)]
pub struct TopicPanelSelection {
    pub selected: FxHashSet<(String, String)>,
    pub last_clicked: Option<(String, String)>,
    pub filter: String,
    was_filtering: bool,
}

impl TopicPanelSelection {
    pub fn clear(&mut self) {
        self.selected.clear();
        self.last_clicked = None;
    }

    pub fn toggle(&mut self, topic: &str, col: &str) {
        let key = (topic.to_string(), col.to_string());
        if self.selected.contains(&key) {
            self.selected.remove(&key);
        } else {
            self.selected.insert(key.clone());
        }
        self.last_clicked = Some(key);
    }

    pub fn select(&mut self, topic: &str, col: &str) {
        let key = (topic.to_string(), col.to_string());
        self.selected.insert(key.clone());
        self.last_clicked = Some(key);
    }

    pub fn select_range(&mut self, items: &[(String, String)], topic: &str, col: &str) {
        let target_key = (topic.to_string(), col.to_string());

        if let Some(last_key) = &self.last_clicked {
            let mut last_idx = None;
            let mut target_idx = None;

            for (i, (t, c)) in items.iter().enumerate() {
                if t == &target_key.0 && c == &target_key.1 {
                    target_idx = Some(i);
                }
                if t == &last_key.0 && c == &last_key.1 {
                    last_idx = Some(i);
                }

                if last_idx.is_some() && target_idx.is_some() {
                    break;
                }
            }

            if let (Some(start), Some(end)) = (last_idx, target_idx) {
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };

                for (t, c) in items.iter().skip(start).take(end - start + 1) {
                    self.selected.insert((t.clone(), c.clone()));
                }
            }
        }

        self.last_clicked = Some(target_key);
    }
}

#[inline]
fn format_value(value: f32) -> String {
    let abs = value.abs();
    if abs < 0.001 {
        format!("{:.6}", value)
    } else if abs < 1.0 {
        format!("{:.4}", value)
    } else if abs < 1000.0 {
        format!("{:.2}", value)
    } else {
        format!("{:.0}", value)
    }
}

#[derive(Clone)]
struct ColumnInfo {
    value_text: String,
}

impl ColumnInfo {
    fn compute(data_store: &DataStore, topic: &str, col: &str) -> Self {
        if let Some(data) = data_store.get_column(topic, col) {
            if data.is_empty() {
                Self {
                    value_text: "<empty>".to_string(),
                }
            } else if data.len() == 1 {
                Self {
                    value_text: format!("[{}]", format_value(data[0])),
                }
            } else {
                Self {
                    value_text: format!(
                        "[{} .. {}]",
                        format_value(data[0]),
                        format_value(data[data.len() - 1])
                    ),
                }
            }
        } else {
            Self {
                value_text: "<no data>".to_string(),
            }
        }
    }
}

pub fn render_topic_panel(
    ui: &mut egui::Ui,
    data_store: &DataStore,
    selection: &mut TopicPanelSelection,
    dragged_item: &mut Option<(String, String)>,
) {
    ui.set_max_width(350.0);

    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut selection.filter);
        if ui.button("✖").clicked() {
            selection.filter.clear();
        }
    });
    ui.separator();

    if data_store.is_empty() {
        ui.label("No data loaded yet.");
        ui.separator();
        return;
    }

    let topics = data_store.get_topics();
    let is_filtering = !selection.filter.is_empty();

    let just_stopped_filtering = selection.was_filtering && !is_filtering;
    selection.was_filtering = is_filtering;

    let (topic_filter, column_filter) = if let Some(slash_pos) = selection.filter.find('/') {
        let topic_part = &selection.filter[..slash_pos];
        let column_part = &selection.filter[slash_pos + 1..];
        (topic_part.to_lowercase(), Some(column_part.to_lowercase()))
    } else {
        (selection.filter.to_lowercase(), None)
    };

    let mut matching_items: Vec<(String, Vec<(String, ColumnInfo)>)> = Vec::new();

    for topic in &topics {
        let topic_matches = is_filtering && fuzzy_match(&topic.to_lowercase(), &topic_filter);
        let columns = data_store.get_columns(topic);

        let matching_columns: Vec<(String, ColumnInfo)> = if is_filtering {
            columns
                .iter()
                .filter_map(|col| {
                    let col_lower = col.to_lowercase();
                    let matches = if let Some(ref col_filter) = column_filter {
                        topic_matches && fuzzy_match(&col_lower, col_filter)
                    } else {
                        topic_matches || fuzzy_match(&col_lower, &topic_filter)
                    };

                    if matches {
                        Some(((*col).clone(), ColumnInfo::compute(data_store, topic, col)))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            columns
                .iter()
                .map(|col| ((*col).clone(), ColumnInfo::compute(data_store, topic, col)))
                .collect()
        };

        if !matching_columns.is_empty() {
            matching_items.push(((*topic).clone(), matching_columns));
        }
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.style_mut().interaction.selectable_labels = false;

            for (topic, columns) in &matching_items {
                egui::CollapsingHeader::new(topic.as_str())
                    .default_open(false)
                    .open(if is_filtering {
                        Some(true)
                    } else if just_stopped_filtering {
                        Some(false)
                    } else {
                        None
                    })
                    .show(ui, |ui| {
                        if columns.is_empty() {
                            ui.label("(no columns)");
                            return;
                        }

                        let items: Vec<(String, String)> = columns
                            .iter()
                            .map(|(col, _)| (topic.clone(), col.clone()))
                            .collect();

                        for (col, col_info) in columns {
                            let is_selected =
                                selection.selected.contains(&(topic.clone(), col.clone()));
                            let value_text = &col_info.value_text;

                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                                egui::Sense::click_and_drag(),
                            );

                            if ui.is_rect_visible(rect) {
                                if is_selected {
                                    ui.painter().rect_filled(
                                        rect,
                                        0.0,
                                        egui::Color32::from_rgb(70, 120, 200),
                                    );
                                }

                                let col_color = if is_selected {
                                    egui::Color32::WHITE
                                } else {
                                    ui.style().visuals.text_color()
                                };

                                let text_pos = rect.left_center() + egui::vec2(4.0, 0.0);
                                ui.painter().text(
                                    text_pos,
                                    egui::Align2::LEFT_CENTER,
                                    col.as_str(),
                                    egui::FontId::default(),
                                    col_color,
                                );

                                let value_color = if is_selected {
                                    egui::Color32::from_rgb(200, 200, 200)
                                } else {
                                    egui::Color32::GRAY
                                };

                                let value_pos = rect.right_center() - egui::vec2(4.0, 0.0);
                                ui.painter().text(
                                    value_pos,
                                    egui::Align2::RIGHT_CENTER,
                                    value_text.as_str(),
                                    egui::FontId::monospace(10.0),
                                    value_color,
                                );
                            }

                            if response.clicked() {
                                let modifiers = ui.input(|i| i.modifiers);

                                if modifiers.shift {
                                    selection.select_range(&items, topic, col);
                                } else if modifiers.ctrl || modifiers.command {
                                    selection.toggle(topic, col);
                                } else {
                                    selection.clear();
                                    selection.select(topic, col);
                                }
                            }

                            if response.dragged() {
                                *dragged_item = Some((topic.clone(), col.clone()));
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

                                let tooltip_text = if is_selected && selection.selected.len() > 1 {
                                    format!("📊 {} items", selection.selected.len())
                                } else {
                                    format!("📊 {}/{}", topic, col)
                                };

                                egui::show_tooltip_at_pointer(
                                    ui.ctx(),
                                    egui::LayerId::new(
                                        egui::Order::Middle,
                                        egui::Id::new("drag_tooltip"),
                                    ),
                                    egui::Id::new("drag_tooltip"),
                                    |ui| {
                                        ui.label(tooltip_text);
                                    },
                                );
                            }

                            if response.hovered() && dragged_item.is_none() {
                                let hover_text = if is_selected && selection.selected.len() > 1 {
                                    format!(
                                        "Drag to add {} selected items to a plot",
                                        selection.selected.len()
                                    )
                                } else {
                                    format!("Drag to add {} to a plot", col)
                                };
                                response.on_hover_text(hover_text);
                            }
                        }
                    });
            }
        });
}
