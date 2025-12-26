use crate::ui::calculate_grid_step;
use eframe::egui;

pub fn render_timeline(
    ui: &mut egui::Ui,
    global_min: f32,
    global_max: f32,
    min_time: &mut f32,
    max_time: &mut f32,
    current_time: &mut f32,
    is_playing: &mut bool,
    playback_speed: &mut f32,
    lock_to_last: &mut bool,
    lock_viewport: &mut bool,
    always_show_playback_tooltip: &mut bool,
) {
    let available_rect = ui.available_rect_before_wrap();
    let timeline_height = 40.0;
    let play_button_width = 40.0;
    let speed_control_width = 60.0;
    let menu_button_width = 30.0;
    let controls_padding = 8.0;
    let controls_width =
        play_button_width + speed_control_width + menu_button_width + controls_padding * 4.0;

    let (full_rect, _) = ui.allocate_exact_size(
        egui::vec2(available_rect.width(), timeline_height),
        egui::Sense::hover(),
    );

    let controls_rect =
        egui::Rect::from_min_size(full_rect.min, egui::vec2(controls_width, timeline_height));

    let timeline_rect = egui::Rect::from_min_size(
        egui::pos2(full_rect.min.x + controls_width, full_rect.min.y),
        egui::vec2(full_rect.width() - controls_width, timeline_height),
    );

    ui.painter()
        .rect_filled(full_rect, 0.0, egui::Color32::from_rgb(30, 30, 30));

    let control_height = timeline_height - controls_padding * 2.0;
    let control_y = controls_rect.min.y + controls_padding;

    let button_rect = egui::Rect::from_min_size(
        egui::pos2(controls_rect.min.x + controls_padding, control_y),
        egui::vec2(play_button_width, control_height),
    );

    let button_response = ui.interact(
        button_rect,
        ui.id().with("play_pause_button"),
        egui::Sense::click(),
    );

    if button_response.clicked() {
        *is_playing = !*is_playing;
    }

    let button_color = if button_response.hovered() {
        egui::Color32::from_rgb(70, 70, 70)
    } else {
        egui::Color32::from_rgb(50, 50, 50)
    };

    ui.painter().rect_filled(button_rect, 4.0, button_color);
    ui.painter().rect_stroke(
        button_rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
    );

    let button_text = if *is_playing { "⏸" } else { "▶" };
    ui.painter().text(
        button_rect.center(),
        egui::Align2::CENTER_CENTER,
        button_text,
        egui::FontId::proportional(14.0),
        egui::Color32::WHITE,
    );

    let speed_control_rect = egui::Rect::from_min_size(
        egui::pos2(button_rect.max.x + controls_padding, control_y),
        egui::vec2(speed_control_width, control_height),
    );

    let speed_bg_color = egui::Color32::from_rgb(50, 50, 50);
    ui.painter()
        .rect_filled(speed_control_rect, 4.0, speed_bg_color);
    ui.painter().rect_stroke(
        speed_control_rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
    );

    ui.allocate_new_ui(
        egui::UiBuilder::new().max_rect(speed_control_rect).layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        ),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add(
                egui::DragValue::new(playback_speed)
                    .speed(0.1)
                    .range(0.01..=1000.0)
                    .suffix("x"),
            );
        },
    );

    let menu_button_rect = egui::Rect::from_min_size(
        egui::pos2(speed_control_rect.max.x + controls_padding, control_y),
        egui::vec2(menu_button_width, control_height),
    );

    let menu_button_response = ui.interact(
        menu_button_rect,
        ui.id().with("timeline_menu_button"),
        egui::Sense::click(),
    );

    let menu_bg_color = if menu_button_response.hovered() {
        egui::Color32::from_rgb(70, 70, 70)
    } else {
        egui::Color32::from_rgb(50, 50, 50)
    };

    ui.painter()
        .rect_filled(menu_button_rect, 4.0, menu_bg_color);
    ui.painter().rect_stroke(
        menu_button_rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
    );

    ui.painter().text(
        menu_button_rect.center(),
        egui::Align2::CENTER_CENTER,
        "⚙",
        egui::FontId::proportional(14.0),
        egui::Color32::WHITE,
    );

    if menu_button_response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(ui.id().with("timeline_menu_popup")));
    }

    egui::popup_above_or_below_widget(
        ui,
        ui.id().with("timeline_menu_popup"),
        &menu_button_response,
        egui::AboveOrBelow::Above,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(150.0);
            if ui.checkbox(lock_to_last, "Lock to Last").clicked() {
                ui.memory_mut(|mem| mem.close_popup());
            }
            if ui.checkbox(lock_viewport, "Lock Viewport").clicked() {
                ui.memory_mut(|mem| mem.close_popup());
            }
            if ui
                .checkbox(always_show_playback_tooltip, "Always Show Playback Tooltip")
                .clicked()
            {
                ui.memory_mut(|mem| mem.close_popup());
            }
        },
    );

    let bar_padding = 10.0;
    let bar_rect = timeline_rect.shrink2(egui::vec2(bar_padding, 5.0));

    ui.painter()
        .rect_filled(bar_rect, 2.0, egui::Color32::from_rgb(50, 50, 50));

    let time_span = global_max - global_min;
    if time_span > 0.0 {
        let t_step = calculate_grid_step(time_span, 10);
        let first_t = (global_min / t_step).ceil() * t_step;

        let num_ticks = ((global_max - first_t) / t_step).ceil() as i32 + 1;

        let pixels_per_tick = if num_ticks > 1 {
            bar_rect.width() / (num_ticks as f32)
        } else {
            bar_rect.width()
        };

        let min_label_spacing = 50.0;
        let label_frequency = ((min_label_spacing / pixels_per_tick).ceil() as i32).max(1);

        let mut t = first_t;
        let mut tick_index = 0;
        while t <= global_max {
            let x_norm = (t - global_min) / time_span;
            let x_px = bar_rect.min.x + x_norm * bar_rect.width();

            ui.painter().line_segment(
                [
                    egui::pos2(x_px, bar_rect.min.y),
                    egui::pos2(x_px, bar_rect.min.y + 4.0),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(150)),
            );

            if tick_index % label_frequency == 0 {
                ui.painter().text(
                    egui::pos2(x_px, bar_rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.1}s", t),
                    egui::FontId::proportional(9.0),
                    egui::Color32::from_gray(180),
                );
            }

            t += t_step;
            tick_index += 1;
        }
    }

    if time_span > 0.0 {
        let view_start_norm = (*min_time - global_min) / time_span;
        let view_end_norm = (*max_time - global_min) / time_span;

        let view_start_x = bar_rect.min.x + view_start_norm * bar_rect.width();
        let view_end_x = bar_rect.min.x + view_end_norm * bar_rect.width();

        let view_rect =
            egui::Rect::from_x_y_ranges(view_start_x..=view_end_x, bar_rect.min.y..=bar_rect.max.y);

        ui.painter().rect_filled(
            view_rect,
            2.0,
            egui::Color32::from_rgba_premultiplied(100, 150, 255, 60),
        );
    }

    if time_span > 0.0 {
        let cursor_norm = (*current_time - global_min) / time_span;
        let cursor_x = bar_rect.min.x + cursor_norm * bar_rect.width();

        ui.painter().line_segment(
            [
                egui::pos2(cursor_x, bar_rect.min.y),
                egui::pos2(cursor_x, bar_rect.max.y),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 165, 0)),
        );

        let handle_size = 5.0;
        let handle_points = vec![
            egui::pos2(cursor_x, bar_rect.min.y - handle_size),
            egui::pos2(cursor_x - handle_size, bar_rect.min.y),
            egui::pos2(cursor_x + handle_size, bar_rect.min.y),
        ];
        ui.painter().add(egui::Shape::convex_polygon(
            handle_points,
            egui::Color32::from_rgb(255, 165, 0),
            egui::Stroke::NONE,
        ));
    }

    let response = ui.interact(
        timeline_rect,
        ui.id().with("timeline_interaction"),
        egui::Sense::click_and_drag(),
    );

    if (response.clicked() || response.dragged()) && ui.input(|i| i.pointer.primary_down()) {
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            if bar_rect.contains(pointer_pos) {
                let x_norm = ((pointer_pos.x - bar_rect.min.x) / bar_rect.width()).clamp(0.0, 1.0);
                *current_time = global_min + x_norm * time_span;
                *is_playing = false;
            }
        }
    }

    if (response.clicked() || response.dragged()) && ui.input(|i| i.pointer.secondary_down()) {
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            if bar_rect.contains(pointer_pos) {
                let x_norm = ((pointer_pos.x - bar_rect.min.x) / bar_rect.width()).clamp(0.0, 1.0);
                let current_t = global_min + x_norm * time_span;
                let dist_to_min = current_t - *min_time;
                let dist_to_max = current_t - *max_time;

                if dist_to_max.abs() > dist_to_min.abs() {
                    *min_time = current_t;
                } else {
                    *max_time = current_t;
                }
            }
        }
    }

    if response.dragged() && ui.input(|i| i.pointer.middle_down()) {
        let delta = response.drag_delta();
        let width = bar_rect.width();

        if width > 0.0 && time_span > 0.0 {
            let dt = delta.x * (time_span / width);

            let view_width = *max_time - *min_time;
            let mut new_min = *min_time + dt;
            let mut new_max = *max_time + dt;

            if new_min < global_min {
                let offset = global_min - new_min;
                new_min = global_min;
                new_max += offset;
            }
            if new_max > global_max {
                let offset = new_max - global_max;
                new_max = global_max;
                new_min -= offset;
            }

            new_min = new_min.max(global_min);
            new_max = new_max.min(global_max);

            if (new_max - new_min - view_width).abs() < 0.001 {
                *min_time = new_min;
                *max_time = new_max;
            }
        }

        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
    }
}
