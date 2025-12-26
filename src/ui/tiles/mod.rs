pub mod plot_tile;
pub mod tile_behavior;

pub use plot_tile::{InterpolationMode, PlotTile};
pub use tile_behavior::TiPlotBehavior;

use eframe::egui;

fn calculate_tooltip_layout(ui: &egui::Ui, num_traces: usize, max_height: f32) -> (usize, usize) {
    let font_id_item = egui::FontId::default();
    let sample_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            "sample: 0.0000".to_string(),
            font_id_item,
            egui::Color32::WHITE,
        )
    });
    let line_height = sample_galley.size().y + 6.0;

    let available_height = max_height - 40.0; // Header + padding
    let max_rows_per_column = (available_height / line_height).floor() as usize;
    let max_rows_per_column = max_rows_per_column.max(3);

    let num_columns = ((num_traces + max_rows_per_column - 1) / max_rows_per_column).max(1);
    let items_per_column = (num_traces + num_columns - 1) / num_columns;

    (num_columns, items_per_column)
}

fn render_tooltip_content(
    ui: &mut egui::Ui,
    tile: &PlotTile,
    num_columns: usize,
    items_per_column: usize,
) -> bool {
    const MAX_TOOLTIP_TRACES: usize = 50;
    let column_spacing = 6.0;

    let num_traces_to_show = tile.traces.len().min(MAX_TOOLTIP_TRACES);

    if tile.traces.len() > MAX_TOOLTIP_TRACES {
        ui.label(
            egui::RichText::new(format!(
                "Showing {} of {} traces",
                MAX_TOOLTIP_TRACES,
                tile.traces.len()
            ))
            .italics()
            .size(10.0)
            .color(egui::Color32::GRAY),
        );
        ui.separator();
    }

    let mut any_rendered = false;

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = column_spacing;

        for col_idx in 0..num_columns {
            let start_idx = col_idx * items_per_column;
            let end_idx = (start_idx + items_per_column).min(num_traces_to_show);

            if start_idx >= num_traces_to_show {
                break;
            }

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;

                for i in start_idx..end_idx {
                    let trace = &tile.traces[i];

                    if let Some(val) = tile.cached_tooltip_values.get(i).and_then(|&v| v) {
                        any_rendered = true;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;

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

                            ui.label(format!("{}: {:.4}", trace.col, val));
                        });
                    }
                }
            });
        }
    });

    any_rendered
}

pub fn render_cursor_tooltip(
    ui: &mut egui::Ui,
    plot_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    hover_time: f32,
    tile: &mut PlotTile,
) {
    let tooltip_padding = 6.0;
    let num_traces = tile.traces.len().min(50);
    let max_tooltip_height = plot_rect.height() - 40.0;

    let (num_columns, items_per_column) =
        calculate_tooltip_layout(ui, num_traces, max_tooltip_height);

    let tooltip_size_id = ui
        .id()
        .with("tooltip_size")
        .with((num_columns, items_per_column));
    let estimated_size: egui::Vec2 = ui.ctx().data(|d| {
        d.get_temp(tooltip_size_id).unwrap_or_else(|| {
            let width = match num_columns {
                1 => 220.0,
                2 => 440.0,
                _ => 660.0,
            };
            let height = (items_per_column as f32 * 18.0) + 50.0;
            egui::vec2(width, height)
        })
    });

    let right_edge_if_left = pointer_pos.x + 15.0 + estimated_size.x;
    let would_overflow_right = right_edge_if_left > plot_rect.max.x;

    let (pivot, tooltip_x) = if would_overflow_right {
        (egui::Align2::RIGHT_TOP, pointer_pos.x - 15.0)
    } else {
        (egui::Align2::LEFT_TOP, pointer_pos.x + 15.0)
    };

    let tooltip_y = (pointer_pos.y + 15.0).clamp(
        plot_rect.min.y,
        (plot_rect.max.y - estimated_size.y).max(plot_rect.min.y),
    );

    let tooltip_pos = egui::pos2(tooltip_x, tooltip_y);

    let response = egui::Area::new(
        ui.id()
            .with("cursor_tooltip")
            .with((num_columns, items_per_column)),
    )
    .fixed_pos(tooltip_pos)
    .pivot(pivot)
    .order(egui::Order::Middle)
    .show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style())
            .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 240))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(100)))
            .rounding(4.0)
            .inner_margin(tooltip_padding)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("Time: {:.3}s", hover_time))
                        .strong()
                        .size(12.0),
                );

                let has_values = tile.cached_tooltip_values.iter().any(|v| v.is_some());
                if has_values && items_per_column > 0 {
                    ui.separator();
                    render_tooltip_content(ui, tile, num_columns, items_per_column);
                }
            })
    });

    let actual_size = response.response.rect.size();
    ui.ctx()
        .data_mut(|d| d.insert_temp(tooltip_size_id, actual_size));
}
