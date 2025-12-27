use std::sync::Arc;

use super::PlotTile;
use crate::core::DataStore;
use crate::ui::panels::TopicPanelSelection;
use crate::ui::renderer::PlotRenderer;
use crate::ui::tiles::render_cursor_tooltip;
use crate::ui::{calculate_grid_step, get_trace_color};
use eframe::egui;
use egui_phosphor::regular as icons;
use egui_tiles::{Behavior, LinearDir, TileId, UiResponse};

pub struct TiPlotBehavior<'a> {
    pub min_time: &'a mut f32,
    pub max_time: &'a mut f32,
    pub global_min: f32,
    pub global_max: f32,
    pub current_time: &'a mut f32,
    pub data_store: &'a DataStore,
    pub topic_selection: &'a TopicPanelSelection,
    pub dragged_item: &'a mut Option<(String, String)>,
    pub split_request: &'a mut Option<(TileId, LinearDir)>,
    pub reset_sizes_request: &'a mut bool,
    pub is_playing: &'a bool,
    pub always_show_playback_tooltip: &'a bool,
    pub renderer: &'a std::sync::Arc<std::sync::Mutex<PlotRenderer>>,
}

impl<'a> Behavior<PlotTile> for TiPlotBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &PlotTile) -> egui::WidgetText {
        format!("Graph ({})", pane.trace_count()).into()
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, tile: &mut PlotTile) -> UiResponse {
        let rect = ui.available_rect_before_wrap();

        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 20));
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
        );

        let response = ui.interact(
            rect,
            ui.id().with("plot_interaction"),
            egui::Sense::click_and_drag(),
        );

        let right_mouse_down = ui.input(|i| i.pointer.secondary_down());

        let mut context_menu_showing = false;

        response.context_menu(|ui| {
            context_menu_showing = true;

            if ui
                .button(format!("{} Clear All Traces", icons::TRASH))
                .clicked()
            {
                tile.traces.clear();
                tile.cached_tooltip_values.clear();
                tile.cached_tooltip_time = f32::NEG_INFINITY;
                ui.close_menu();
            }

            if !tile.traces.is_empty() {
                ui.menu_button(format!("{} Remove Trace", icons::MINUS_CIRCLE), |ui| {
                    let mut trace_to_remove: Option<usize> = None;

                    for (idx, trace) in tile.traces.iter().enumerate() {
                        let trace_label = format!("{}/{}", trace.topic, trace.col);

                        ui.horizontal(|ui| {
                            let swatch_size = egui::vec2(10.0, 10.0);
                            let (swatch_rect, _) =
                                ui.allocate_exact_size(swatch_size, egui::Sense::hover());
                            ui.painter().rect_filled(
                                swatch_rect,
                                2.0,
                                egui::Color32::from_rgb(
                                    (trace.color[0] * 255.0) as u8,
                                    (trace.color[1] * 255.0) as u8,
                                    (trace.color[2] * 255.0) as u8,
                                ),
                            );

                            if ui.button(&trace_label).clicked() {
                                trace_to_remove = Some(idx);
                            }
                        });
                    }

                    if let Some(idx) = trace_to_remove {
                        tile.traces.remove(idx);
                        tile.cached_tooltip_values.clear();
                        tile.cached_tooltip_time = f32::NEG_INFINITY;
                        ui.close_menu();
                    }
                });
            }

            ui.separator();

            if ui
                .button(format!(
                    "{} Split Horizontally",
                    icons::SQUARE_SPLIT_HORIZONTAL
                ))
                .clicked()
            {
                *self.split_request = Some((tile_id, LinearDir::Horizontal));
                ui.close_menu();
            }

            if ui
                .button(format!("{} Split Vertically", icons::SQUARE_SPLIT_VERTICAL))
                .clicked()
            {
                *self.split_request = Some((tile_id, LinearDir::Vertical));
                ui.close_menu();
            }

            ui.separator();

            if ui
                .checkbox(&mut tile.show_legend, format!("Show Legend"))
                .clicked()
            {
                ui.close_menu();
            }
            if ui
                .checkbox(&mut tile.show_hover_tooltip, format!("Show Tooltip"))
                .clicked()
            {
                ui.close_menu();
            }

            if ui
                .checkbox(&mut tile.show_hover_circles, format!("Show Hover Circles"))
                .clicked()
            {
                ui.close_menu();
            }

            if ui
                .checkbox(&mut tile.scatter_mode, "Scatter Mode")
                .clicked()
            {
                ui.close_menu();
            }

            ui.separator();

            if ui
                .button(format!("{} Reset Tile Sizes", icons::ARROWS_OUT))
                .clicked()
            {
                *self.reset_sizes_request = true;
                ui.close_menu();
            }

            if ui
                .button(format!("{} Reset View", icons::ARROWS_OUT_LINE_HORIZONTAL))
                .clicked()
            {
                *self.min_time = self.global_min;
                *self.max_time = self.global_max;
                ui.close_menu();
            }

            ui.separator();

            if ui.button(format!("{} Plot Info", icons::INFO)).clicked() {
                tile.show_info_window = true;
                ui.close_menu();
            }
        });

        let modifiers = ui.input(|i| i.modifiers);
        if modifiers.alt && response.hovered() {
            if let Some(pointer_pos) = response.hover_pos() {
                let width = rect.width();
                if width > 0.0 {
                    let x_pct = ((pointer_pos.x - rect.left()) / width).clamp(0.0, 1.0);
                    *self.current_time = *self.min_time + x_pct * (*self.max_time - *self.min_time);
                }
            }
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
        }

        if response.dragged() && !modifiers.alt {
            let delta = response.drag_delta();
            let width = rect.width();
            if width > 0.0 {
                let view_width = *self.max_time - *self.min_time;
                let dt = -delta.x * (view_width / width);

                let mut new_min = *self.min_time + dt;
                let mut new_max = *self.max_time + dt;

                if new_min < self.global_min {
                    let offset = self.global_min - new_min;
                    new_min = self.global_min;
                    new_max += offset;
                }
                if new_max > self.global_max {
                    let offset = new_max - self.global_max;
                    new_max = self.global_max;
                    new_min -= offset;
                }

                new_min = new_min.max(self.global_min);
                new_max = new_max.min(self.global_max);

                *self.min_time = new_min;
                *self.max_time = new_max;
            }
        }

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let factor = 1.0 - (scroll * 0.01);

                if let Some(pointer_pos) = response.hover_pos() {
                    let t = (pointer_pos.x - rect.left()) / rect.width();
                    let center = *self.min_time + t * (*self.max_time - *self.min_time);

                    let span = *self.max_time - *self.min_time;
                    let new_span = span * factor;

                    let mut new_min = center - new_span * t;
                    let mut new_max = center + new_span * (1.0 - t);

                    let global_span = self.global_max - self.global_min;
                    if new_max - new_min > global_span {
                        new_min = self.global_min;
                        new_max = self.global_max;
                    } else {
                        if new_min < self.global_min {
                            let offset = self.global_min - new_min;
                            new_min = self.global_min;
                            new_max += offset;
                        }
                        if new_max > self.global_max {
                            let offset = new_max - self.global_max;
                            new_max = self.global_max;
                            new_min -= offset;
                        }
                        new_min = new_min.max(self.global_min);
                        new_max = new_max.min(self.global_max);
                    }

                    let min_sample_interval = self.estimate_min_sample_interval();
                    let min_span = min_sample_interval * 2.0;

                    if new_max - new_min >= min_span {
                        *self.min_time = new_min;
                        *self.max_time = new_max;
                    }
                }
            }
        }

        if self.dragged_item.is_some() && response.hovered() {
            ui.painter()
                .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::GOLD));
            if ui.input(|i| i.pointer.any_released()) {
                if let Some((topic, col)) = self.dragged_item.take() {
                    if self
                        .topic_selection
                        .selected
                        .contains(&(topic.clone(), col.clone()))
                        && self.topic_selection.selected.len() > 1
                    {
                        let mut selected_items: Vec<(String, String)> =
                            self.topic_selection.selected.iter().cloned().collect();
                        selected_items.sort_by(|a, b| {
                            let a_key = format!("{}/{}", a.0, a.1);
                            let b_key = format!("{}/{}", b.0, b.1);
                            natord::compare(&a_key, &b_key)
                        });
                        for (sel_topic, sel_col) in selected_items {
                            if !tile
                                .traces
                                .iter()
                                .any(|t| t.topic == sel_topic && t.col == sel_col)
                            {
                                let color = get_trace_color(tile.traces.len());
                                tile.add_trace(sel_topic, sel_col, color);
                            }
                        }
                    } else {
                        if !tile.traces.iter().any(|t| t.topic == topic && t.col == col) {
                            let color = get_trace_color(tile.traces.len());
                            tile.add_trace(topic, col, color);
                        }
                    }
                }
            }
        }

        let (min_y, max_y) = self.calculate_y_bounds(tile);

        self.draw_grid(ui, rect, min_y, max_y);

        for trace in &tile.traces {
            let renderer = self.renderer.clone();
            let topic = trace.topic.clone();
            let col = trace.col.clone();
            let bounds = [*self.min_time, *self.max_time, min_y, max_y];
            let color = trace.color;
            let scatter_mode = tile.scatter_mode;

            let callback = egui::PaintCallback {
                rect,
                callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                    use eframe::glow::HasContext as _;

                    let gl = painter.gl();
                    let renderer = renderer.lock().unwrap();

                    unsafe {
                        // Save/set OpenGL state
                        gl.enable(glow::BLEND);
                        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                        gl.disable(glow::DEPTH_TEST);
                        gl.disable(glow::SCISSOR_TEST);

                        // Render the trace
                        renderer.render_trace(&topic, &col, bounds, color, scatter_mode);
                    }
                })),
            };

            ui.painter().add(callback);
        }

        // Draw playback cursor
        if *self.current_time >= *self.min_time && *self.current_time <= *self.max_time {
            let time_span = *self.max_time - *self.min_time;
            if time_span > 0.0 {
                let cursor_norm = (*self.current_time - *self.min_time) / time_span;
                let cursor_x = rect.min.x + cursor_norm * rect.width();

                ui.painter().line_segment(
                    [
                        egui::pos2(cursor_x, rect.min.y),
                        egui::pos2(cursor_x, rect.max.y),
                    ],
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 165, 0)),
                );
            }
        }

        // Handle cursors and tooltips
        if *self.always_show_playback_tooltip || modifiers.alt {
            self.handle_playback_cursor(ui, rect, tile, min_y, max_y);
        } else if !context_menu_showing {
            if *self.is_playing {
                self.handle_playback_cursor(ui, rect, tile, min_y, max_y);
            } else if !right_mouse_down {
                self.handle_cursor(ui, rect, tile, min_y, max_y);
            }
        }

        self.draw_legend(ui, rect, tile);

        // Show info window if requested
        if tile.show_info_window {
            egui::Window::new(format!("Plot Info {:?}", tile_id))
                .collapsible(false)
                .resizable(true)
                .default_width(400.0)
                .max_height(600.0)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Total: {} trace(s)", tile.traces.len()));
                    if tile.traces.len() > 0 {
                        ui.separator();
                    }

                    egui::ScrollArea::vertical()
                        .max_height(500.0)
                        .show(ui, |ui| {
                            for (idx, trace) in tile.traces.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let swatch_size = egui::vec2(12.0, 12.0);
                                    let (swatch_rect, _) =
                                        ui.allocate_exact_size(swatch_size, egui::Sense::hover());
                                    ui.painter().rect_filled(
                                        swatch_rect,
                                        2.0,
                                        egui::Color32::from_rgb(
                                            (trace.color[0] * 255.0) as u8,
                                            (trace.color[1] * 255.0) as u8,
                                            (trace.color[2] * 255.0) as u8,
                                        ),
                                    );

                                    ui.label(format!("{} / {}", trace.topic, trace.col));
                                });

                                if idx < tile.traces.len() - 1 {
                                    ui.add_space(4.0);
                                }
                            }
                        });

                    if tile.traces.len() > 0 {
                        ui.separator();
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            tile.show_info_window = false;
                        }
                    });
                });
        }

        UiResponse::None
    }

    fn is_tab_closable(&self, tiles: &egui_tiles::Tiles<PlotTile>, _tile_id: TileId) -> bool {
        let pane_count = tiles
            .tiles()
            .filter(|tile| matches!(tile, egui_tiles::Tile::Pane(_)))
            .count();

        pane_count > 1
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        egui::Color32::from_rgb(30, 30, 30)
    }

    fn drag_preview_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(100, 150, 255, 180)
    }

    fn retain_pane(&mut self, _pane: &PlotTile) -> bool {
        true
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

impl<'a> TiPlotBehavior<'a> {
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
            0.001
        } else {
            min_interval
        }
    }

    fn calculate_y_bounds(&self, tile: &PlotTile) -> (f32, f32) {
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut has_data = false;

        for trace in &tile.traces {
            if let (Some(times), Some(vals)) = (
                self.data_store.get_column(&trace.topic, "timestamp"),
                self.data_store.get_column(&trace.topic, &trace.col),
            ) {
                if times.is_empty() || vals.is_empty() {
                    continue;
                }

                let start_idx = times.partition_point(|&t| t < *self.min_time);
                let end_idx = times.partition_point(|&t| t <= *self.max_time);

                for i in start_idx..end_idx.min(vals.len()) {
                    let v = vals[i];
                    if v < min_y {
                        min_y = v;
                    }
                    if v > max_y {
                        max_y = v;
                    }
                    has_data = true;
                }
            }
        }

        if !has_data {
            return (-1.0, 1.0);
        }

        let range = max_y - min_y;
        let pad = if range == 0.0 { 1.0 } else { range * 0.1 };
        (min_y - pad, max_y + pad)
    }

    fn draw_grid(&self, ui: &mut egui::Ui, rect: egui::Rect, min_y: f32, max_y: f32) {
        let grid_color = egui::Color32::from_gray(45);
        let text_color = egui::Color32::from_gray(150);
        let font_id = egui::FontId::proportional(10.0);

        let time_span = *self.max_time - *self.min_time;
        if time_span > 0.0 {
            let t_step = calculate_grid_step(time_span, 10);
            let first_t = (*self.min_time / t_step).ceil() * t_step;

            let mut t = first_t;
            while t <= *self.max_time {
                let x_norm = (t - *self.min_time) / time_span;
                let x_px = rect.min.x + x_norm * rect.width();

                if x_px >= rect.min.x && x_px <= rect.max.x {
                    ui.painter().line_segment(
                        [egui::pos2(x_px, rect.min.y), egui::pos2(x_px, rect.max.y)],
                        egui::Stroke::new(1.0, grid_color),
                    );

                    ui.painter().text(
                        egui::pos2(x_px + 2.0, rect.max.y - 12.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{:.1}", t),
                        font_id.clone(),
                        text_color,
                    );
                }
                t += t_step;
            }
        }

        let val_span = max_y - min_y;
        if val_span > 0.0 {
            let v_step = calculate_grid_step(val_span, 8);
            let first_v = (min_y / v_step).ceil() * v_step;

            let mut v = first_v;
            while v <= max_y {
                let y_norm = 1.0 - (v - min_y) / val_span;
                let y_px = rect.min.y + y_norm * rect.height();

                if y_px >= rect.min.y && y_px <= rect.max.y {
                    ui.painter().line_segment(
                        [egui::pos2(rect.min.x, y_px), egui::pos2(rect.max.x, y_px)],
                        egui::Stroke::new(1.0, grid_color),
                    );

                    ui.painter().text(
                        egui::pos2(rect.min.x + 2.0, y_px - 2.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{:.2}", v),
                        font_id.clone(),
                        text_color,
                    );
                }
                v += v_step;
            }
        }
    }

    fn handle_cursor(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        tile: &mut PlotTile,
        min_y: f32,
        max_y: f32,
    ) {
        let is_dragging = ui.input(|i| i.pointer.primary_down());
        if is_dragging {
            return;
        }

        if *self.is_playing {
            return;
        }

        if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if !rect.contains(pointer_pos) {
                return;
            }

            let view_width = *self.max_time - *self.min_time;
            let x_pct = (pointer_pos.x - rect.min.x) / rect.width();
            let hover_time = *self.min_time + x_pct * view_width;

            ui.painter().line_segment(
                [
                    egui::pos2(pointer_pos.x, rect.min.y),
                    egui::pos2(pointer_pos.x, rect.max.y),
                ],
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );

            if tile.show_hover_circles || tile.show_hover_tooltip {
                tile.update_tooltip_cache(hover_time, self.data_store, false);
            }

            if tile.show_hover_circles {
                let val_span = max_y - min_y;
                if val_span > 0.0 {
                    for (i, trace) in tile.traces.iter().enumerate() {
                        if let Some(Some(value)) = tile.cached_tooltip_values.get(i) {
                            let y_norm = 1.0 - (value - min_y) / val_span;
                            let y_px = rect.min.y + y_norm * rect.height();

                            if y_px >= rect.min.y && y_px <= rect.max.y {
                                let point_pos = egui::pos2(pointer_pos.x, y_px);
                                let trace_color = egui::Color32::from_rgb(
                                    (trace.color[0] * 255.0) as u8,
                                    (trace.color[1] * 255.0) as u8,
                                    (trace.color[2] * 255.0) as u8,
                                );

                                ui.painter().circle_filled(point_pos, 3.0, trace_color);

                                ui.painter().circle_stroke(
                                    point_pos,
                                    3.0,
                                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                                );
                            }
                        }
                    }
                }
            }

            if tile.show_hover_tooltip {
                render_cursor_tooltip(ui, rect, pointer_pos, hover_time, tile);
            }
        }
    }

    fn handle_playback_cursor(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        tile: &mut PlotTile,
        min_y: f32,
        max_y: f32,
    ) {
        // Only show if current_time is within view
        if *self.current_time < *self.min_time || *self.current_time > *self.max_time {
            return;
        }

        let time_span = *self.max_time - *self.min_time;
        if time_span <= 0.0 {
            return;
        }

        let cursor_norm = (*self.current_time - *self.min_time) / time_span;
        let cursor_x = rect.min.x + cursor_norm * rect.width();

        if tile.show_hover_circles || tile.show_hover_tooltip {
            tile.update_tooltip_cache(*self.current_time, self.data_store, true);
        }

        if tile.show_hover_circles {
            let val_span = max_y - min_y;
            if val_span > 0.0 {
                for (i, trace) in tile.traces.iter().enumerate() {
                    if let Some(Some(value)) = tile.cached_tooltip_values.get(i) {
                        let y_norm = 1.0 - (value - min_y) / val_span;
                        let y_px = rect.min.y + y_norm * rect.height();

                        if y_px >= rect.min.y && y_px <= rect.max.y {
                            let point_pos = egui::pos2(cursor_x, y_px);
                            let trace_color = egui::Color32::from_rgb(
                                (trace.color[0] * 255.0) as u8,
                                (trace.color[1] * 255.0) as u8,
                                (trace.color[2] * 255.0) as u8,
                            );

                            ui.painter().circle_filled(point_pos, 3.0, trace_color);

                            ui.painter().circle_stroke(
                                point_pos,
                                3.0,
                                egui::Stroke::new(1.5, egui::Color32::WHITE),
                            );
                        }
                    }
                }
            }
        }

        // Show tooltip at playback cursor
        if tile.show_hover_tooltip {
            let cursor_pos = egui::pos2(cursor_x, rect.center().y);
            render_cursor_tooltip(ui, rect, cursor_pos, *self.current_time, tile);
        }
    }

    fn draw_legend(&self, ui: &mut egui::Ui, rect: egui::Rect, tile: &mut PlotTile) {
        if tile.traces.is_empty() {
            return;
        }

        let padding = 10.0;
        let button_size = 16.0;
        let button_spacing = 8.0;

        let clear_button_pos = egui::pos2(rect.max.x - padding - button_size, rect.min.y + padding);
        let toggle_button_pos = egui::pos2(
            clear_button_pos.x,
            clear_button_pos.y + button_size + button_spacing,
        );

        let clear_rect =
            egui::Rect::from_min_size(clear_button_pos, egui::vec2(button_size, button_size));

        let clear_response =
            ui.interact(clear_rect, ui.id().with("clear_plot"), egui::Sense::click());

        if clear_response.clicked() {
            tile.traces.clear();
            tile.cached_tooltip_values.clear();
            tile.cached_tooltip_time = f32::NEG_INFINITY;
        }

        let clear_bg_color = if clear_response.hovered() {
            egui::Color32::from_rgba_unmultiplied(255, 100, 100, 150)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 100, 100, 80)
        };

        ui.painter().rect_filled(clear_rect, 4.0, clear_bg_color);

        let icon_color = if clear_response.hovered() {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_gray(220)
        };

        ui.painter().text(
            clear_rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::TRASH,
            egui::FontId::proportional(button_size * 0.6),
            icon_color,
        );

        if clear_response.hovered() {
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                egui::LayerId::new(egui::Order::Middle, ui.id().with("clear_tooltip")),
                ui.id().with("clear_tooltip"),
                |ui| {
                    ui.label("Clear plot");
                },
            );
        }

        let toggle_rect =
            egui::Rect::from_min_size(toggle_button_pos, egui::vec2(button_size, button_size));

        let toggle_response = ui.interact(
            toggle_rect,
            ui.id().with("legend_toggle"),
            egui::Sense::click(),
        );

        if toggle_response.clicked() {
            tile.show_legend = !tile.show_legend;
        }

        let toggle_bg_color = if toggle_response.hovered() {
            egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150)
        } else {
            egui::Color32::from_rgba_unmultiplied(80, 80, 80, 80)
        };

        ui.painter().rect_filled(toggle_rect, 4.0, toggle_bg_color);

        let eye_icon = if tile.show_legend {
            icons::EYE
        } else {
            icons::EYE_SLASH
        };

        let eye_color = if toggle_response.hovered() {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_gray(220)
        };

        ui.painter().text(
            toggle_rect.center(),
            egui::Align2::CENTER_CENTER,
            eye_icon,
            egui::FontId::proportional(button_size * 0.6),
            eye_color,
        );

        if !tile.show_legend {
            return;
        }

        let legend_width = 200.0;
        let legend_x = clear_button_pos.x - legend_width - 5.0;
        let legend_y = rect.min.y + padding;

        let legend_start_pos = egui::pos2(legend_x, legend_y);

        let line_height = 18.0;
        let legend_padding = 8.0;
        let legend_height = (tile.traces.len() as f32 * line_height) + (legend_padding * 2.0);

        let legend_rect =
            egui::Rect::from_min_size(legend_start_pos, egui::vec2(legend_width, legend_height));

        ui.painter().rect_filled(
            legend_rect,
            8.0,
            egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200),
        );

        ui.painter().rect_stroke(
            legend_rect,
            8.0,
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150),
            ),
        );

        let mut y_offset = legend_start_pos.y + legend_padding;

        for trace in &tile.traces {
            let text_pos = egui::pos2(legend_start_pos.x + legend_padding + 15.0, y_offset);

            let swatch_center = egui::pos2(
                legend_start_pos.x + legend_padding + 5.0,
                y_offset + line_height / 2.0,
            );
            ui.painter().circle_filled(
                swatch_center,
                4.0,
                egui::Color32::from_rgb(
                    (trace.color[0] * 255.0) as u8,
                    (trace.color[1] * 255.0) as u8,
                    (trace.color[2] * 255.0) as u8,
                ),
            );

            let label_text = format!("{}/{}", trace.topic, trace.col);
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_TOP,
                label_text,
                egui::FontId::proportional(11.0),
                egui::Color32::from_gray(220),
            );

            y_offset += line_height;
        }
    }
}
