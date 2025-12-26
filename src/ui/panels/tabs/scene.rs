use crate::core::DataStore;
use crate::ui::panels::tabs::config::VehicleConfig;
use crate::ui::panels::tabs::gltf_loader::ModelCache;
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use egui_phosphor::regular as icons;
use glam::{Mat4, Quat, Vec3, Vec4};

#[derive(Clone)]
pub struct SceneState {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub follow_index: usize,
    pub lock_camera: bool,
    pub fixed_vehicle_scale: bool,
}

impl Default for SceneState {
    fn default() -> Self {
        Self {
            yaw: 45.0f32.to_radians(),
            pitch: 30.0f32.to_radians(),
            distance: 500.0,
            target: Vec3::ZERO,
            follow_index: 0,
            lock_camera: false,
            fixed_vehicle_scale: false,
        }
    }
}

pub fn render_scene_tab(
    ui: &mut egui::Ui,
    _frame: &eframe::Frame,
    vehicles: &mut [VehicleConfig],
    data_store: &DataStore,
    current_time: f32,
    state: &mut SceneState,
    model_cache: &ModelCache,
) {
    ui.horizontal(|ui| {
        if !vehicles.is_empty() {
            egui::ComboBox::from_id_salt("cam_follow_selector")
                .selected_text(
                    vehicles
                        .get(state.follow_index)
                        .map(|v| v.name.as_str())
                        .unwrap_or("None"),
                )
                .show_ui(ui, |ui| {
                    for (i, v) in vehicles.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let eye_icon = if v.visible {
                                icons::EYE
                            } else {
                                icons::EYE_SLASH
                            };

                            let eye_color = if v.visible {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::GRAY
                            };

                            if ui
                                .button(egui::RichText::new(eye_icon).color(eye_color))
                                .on_hover_text(if v.visible {
                                    "Hide vehicle"
                                } else {
                                    "Show vehicle"
                                })
                                .clicked()
                            {
                                v.visible = !v.visible;
                            }

                            ui.selectable_value(&mut state.follow_index, i, &v.name);
                        });
                    }
                });

            ui.separator();
            ui.checkbox(&mut state.lock_camera, "🔒 Lock Camera")
                .on_hover_text(
                    "Lock camera position to vehicle body, but keep horizon level (Chase View)",
                );

            ui.checkbox(&mut state.fixed_vehicle_scale, "📏 Fixed Vehicle Scale")
                .on_hover_text("Keep vehicle size constant regardless of zoom level");
        }
    });
    ui.separator();

    let mut vehicle_rotation = Quat::IDENTITY;

    if !vehicles.is_empty() {
        if state.follow_index >= vehicles.len() {
            state.follow_index = 0;
        }

        let vehicle = &vehicles[state.follow_index];
        let (pos, rot) = vehicle.evaluate_at(data_store, current_time);

        state.target = pos;
        vehicle_rotation = rot;
    }

    let available_size = ui.available_size();

    ui.allocate_ui_with_layout(
        available_size,
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            let rect = ui.max_rect();
            let response = ui.allocate_rect(rect, egui::Sense::drag());

            let painter = ui.painter_at(rect);

            painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 20));

            if response.dragged_by(egui::PointerButton::Primary) {
                state.yaw += response.drag_delta().x * 0.01;
                state.pitch += response.drag_delta().y * 0.01;
                state.pitch = state.pitch.clamp(0.01, 1.55);
            }

            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                state.distance -= scroll * (state.distance * 0.01);
                state.distance = state.distance.clamp(1.0, 5000.0);
            }

            let height = state.distance * state.pitch.sin();
            let ground_dist = state.distance * state.pitch.cos();

            let local_offset_x = -ground_dist * state.yaw.cos();
            let local_offset_y = -ground_dist * state.yaw.sin();
            let local_offset_z = -height;

            let raw_offset = Vec3::new(local_offset_x, local_offset_y, local_offset_z);

            let (eye, up) = if state.lock_camera {
                let rotated_offset = vehicle_rotation * raw_offset;
                (state.target + rotated_offset, -Vec3::Z)
            } else {
                (state.target + raw_offset, -Vec3::Z)
            };

            let view = Mat4::look_at_rh(eye, state.target, up);

            let aspect = rect.width() / rect.height();
            let proj = Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 10000.0);
            let view_proj = proj * view;

            let project = |pos: Vec3| -> Option<(Pos2, f32, f32)> {
                let clip = view_proj * Vec4::from((pos, 1.0));
                let w = clip.w;

                if w.abs() < 0.0001 {
                    return None;
                }

                let ndc = clip.truncate() / w;
                let x = rect.min.x + (1.0 + ndc.x) * 0.5 * rect.width();
                let y = rect.min.y + (1.0 - ndc.y) * 0.5 * rect.height();

                Some((Pos2::new(x, y), clip.z, w))
            };

            let mut draw_clipped_line = |p1: Vec3, p2: Vec3, stroke: Stroke| {
                let clip1 = view_proj * Vec4::from((p1, 1.0));
                let clip2 = view_proj * Vec4::from((p2, 1.0));

                let w1 = clip1.w;
                let w2 = clip2.w;

                let near_threshold = 0.1;

                if w1 < near_threshold && w2 < near_threshold {
                    return;
                }

                let (p1_use, _w1_use, p2_use, _w2_use) =
                    if w1 < near_threshold && w2 >= near_threshold {
                        let t = (near_threshold - w1) / (w2 - w1);
                        let clipped_p1 = p1.lerp(p2, t);
                        (clipped_p1, near_threshold, p2, w2)
                    } else if w2 < near_threshold && w1 >= near_threshold {
                        let t = (near_threshold - w2) / (w1 - w2);
                        let clipped_p2 = p2.lerp(p1, t);
                        (p1, w1, clipped_p2, near_threshold)
                    } else {
                        (p1, w1, p2, w2)
                    };

                if let (Some((s1, _, _)), Some((s2, _, _))) = (project(p1_use), project(p2_use)) {
                    painter.line_segment([s1, s2], stroke);
                }
            };

            let grid_extent = (state.distance * 3.0).max(100.0);

            draw_grid_and_axes(
                &painter,
                &mut draw_clipped_line,
                &project,
                grid_extent,
                state.target,
            );

            if vehicles.is_empty() {
                return;
            }

            let mut model_draw_list: Vec<(f32, Shape)> = Vec::new();

            for vehicle in vehicles.iter() {
                if !vehicle.visible {
                    continue;
                }

                let (pos, rot) = vehicle.evaluate_at(data_store, current_time);

                match &vehicle.position {
                    crate::ui::panels::tabs::config::PositionMode::LocalNED {
                        topic,
                        north,
                        east,
                        down,
                        ..
                    } => {
                        if let (Some(x), Some(y), Some(z), Some(t)) = (
                            data_store.get_column(topic, north),
                            data_store.get_column(topic, east),
                            data_store.get_column(topic, down),
                            data_store.get_column(topic, "timestamp"),
                        ) {
                            let end_idx = t.partition_point(|&val| val <= current_time);
                            let trail_color = Color32::from_rgb(
                                (vehicle.path_color[0] * 255.0) as u8,
                                (vehicle.path_color[1] * 255.0) as u8,
                                (vehicle.path_color[2] * 255.0) as u8,
                            );
                            let stroke = Stroke::new(1.5, trail_color);

                            let step = if end_idx > 2000 { end_idx / 2000 } else { 1 };

                            for i in (0..end_idx.saturating_sub(step)).step_by(step) {
                                let p1 = Vec3::new(x[i], y[i], z[i]);
                                let p2 = Vec3::new(x[i + step], y[i + step], z[i + step]);
                                draw_clipped_line(p1, p2, stroke);
                            }

                            if end_idx > 0 {
                                let last_idx = end_idx - 1;
                                let p_last = Vec3::new(x[last_idx], y[last_idx], z[last_idx]);
                                draw_clipped_line(p_last, pos, stroke);
                            }
                        }
                    }
                    crate::ui::panels::tabs::config::PositionMode::GlobalGPS {
                        topic,
                        lat,
                        lon,
                        alt,
                    } => {
                        if let (Some(lat_vals), Some(lon_vals), Some(alt_vals), Some(t)) = (
                            data_store.get_column(topic, lat),
                            data_store.get_column(topic, lon),
                            data_store.get_column(topic, alt),
                            data_store.get_column(topic, "timestamp"),
                        ) {
                            if !t.is_empty() {
                                let lat_ref = lat_vals[0] as f64;
                                let lon_ref = lon_vals[0] as f64;
                                let alt_ref = alt_vals[0] as f64;

                                let end_idx = t.partition_point(|&val| val <= current_time);
                                let trail_color = Color32::from_rgb(
                                    (vehicle.path_color[0] * 255.0) as u8,
                                    (vehicle.path_color[1] * 255.0) as u8,
                                    (vehicle.path_color[2] * 255.0) as u8,
                                );
                                let stroke = Stroke::new(1.5, trail_color);

                                let step = if end_idx > 2000 { end_idx / 2000 } else { 1 };

                                for i in (0..end_idx.saturating_sub(step)).step_by(step) {
                                    let pos1 = VehicleConfig::gps_to_ned(
                                        lat_vals[i] as f64,
                                        lon_vals[i] as f64,
                                        alt_vals[i] as f64,
                                        lat_ref,
                                        lon_ref,
                                        alt_ref,
                                    );
                                    let pos2 = VehicleConfig::gps_to_ned(
                                        lat_vals[i + step] as f64,
                                        lon_vals[i + step] as f64,
                                        alt_vals[i + step] as f64,
                                        lat_ref,
                                        lon_ref,
                                        alt_ref,
                                    );
                                    draw_clipped_line(pos1, pos2, stroke);
                                }

                                if end_idx > 0 {
                                    let last_idx = end_idx - 1;
                                    let p_last = VehicleConfig::gps_to_ned(
                                        lat_vals[last_idx] as f64,
                                        lon_vals[last_idx] as f64,
                                        alt_vals[last_idx] as f64,
                                        lat_ref,
                                        lon_ref,
                                        alt_ref,
                                    );
                                    draw_clipped_line(p_last, pos, stroke);
                                }
                            }
                        }
                    }
                }

                let offset = vehicle.vehicle_type.orientation_offset();
                let specific_correction =
                    Mat4::from_euler(glam::EulerRot::XYZ, offset.x, offset.y, offset.z);

                let base_correction = Mat4::from_rotation_x(-90.0f32.to_radians());
                let final_correction = base_correction * specific_correction;

                let effective_scale = if state.fixed_vehicle_scale {
                    vehicle.scale * (state.distance / 500.0)
                } else {
                    vehicle.scale
                };

                let model_mat =
                    Mat4::from_scale_rotation_translation(Vec3::splat(effective_scale), rot, pos)
                        * final_correction;

                let vehicle_color = Color32::from_rgb(
                    (vehicle.color[0] * 255.0) as u8,
                    (vehicle.color[1] * 255.0) as u8,
                    (vehicle.color[2] * 255.0) as u8,
                );
                let stroke = Stroke::new(1.5, vehicle_color);

                if let Some(model) =
                    model_cache.get_model(vehicle.vehicle_type.model_path().as_str())
                {
                    let transformed_verts: Vec<Vec3> = model
                        .vertices
                        .iter()
                        .map(|&v| model_mat.transform_point3(v))
                        .collect();

                    let projected_verts: Vec<Option<(Pos2, f32, f32)>> =
                        transformed_verts.iter().map(|&v| project(v)).collect();

                    for line_indices in &model.lines {
                        let idx1 = line_indices[0] as usize;
                        let idx2 = line_indices[1] as usize;

                        let p1_world = transformed_verts[idx1];
                        let p2_world = transformed_verts[idx2];

                        let proj1 = projected_verts[idx1];
                        let proj2 = projected_verts[idx2];

                        let should_draw = match (proj1, proj2) {
                            (Some((s1, d1, w1)), Some((s2, d2, w2))) => {
                                if w1 > 0.0 && w2 > 0.0 {
                                    let avg_depth = (d1 + d2) * 0.5;
                                    let visible = rect.expand(200.0).contains(s1)
                                        || rect.expand(200.0).contains(s2);

                                    if visible {
                                        model_draw_list.push((
                                            avg_depth,
                                            Shape::line_segment([s1, s2], stroke),
                                        ));
                                    }
                                    false
                                } else if w1 > 0.0 || w2 > 0.0 {
                                    true
                                } else {
                                    false
                                }
                            }
                            (Some(_), None) | (None, Some(_)) => true,
                            (None, None) => false,
                        };

                        if should_draw {
                            let clip1 = view_proj * Vec4::from((p1_world, 1.0));
                            let clip2 = view_proj * Vec4::from((p2_world, 1.0));

                            let w1 = clip1.w;
                            let w2 = clip2.w;

                            let near_clip = 0.12;

                            if w1 < near_clip && w2 < near_clip {
                                continue;
                            }

                            let (clipped_p1, clipped_p2) = if w1 < near_clip && w2 >= near_clip {
                                let t = (near_clip - w1) / (w2 - w1);
                                let new_p1 = p1_world + t * (p2_world - p1_world);
                                (new_p1, p2_world)
                            } else if w2 < near_clip && w1 >= near_clip {
                                let t = (near_clip - w2) / (w1 - w2);
                                let new_p2 = p2_world + t * (p1_world - p2_world);
                                (p1_world, new_p2)
                            } else {
                                (p1_world, p2_world)
                            };

                            if let (Some((s1, d1, _)), Some((s2, d2, _))) =
                                (project(clipped_p1), project(clipped_p2))
                            {
                                let avg_depth = (d1 + d2) * 0.5;
                                let visible = rect.expand(200.0).contains(s1)
                                    || rect.expand(200.0).contains(s2);

                                if visible {
                                    model_draw_list
                                        .push((avg_depth, Shape::line_segment([s1, s2], stroke)));
                                }
                            }
                        }
                    }
                }
            }

            model_draw_list
                .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            for (_, shape) in model_draw_list {
                painter.add(shape);
            }
        },
    );
}

fn draw_grid_and_axes(
    painter: &egui::Painter,
    draw_line: &mut impl FnMut(Vec3, Vec3, Stroke),
    project: &impl Fn(Vec3) -> Option<(Pos2, f32, f32)>,
    extent: f32,
    center: Vec3,
) {
    let grid_color = Color32::from_gray(50);
    let grid_stroke = Stroke::new(1.0, grid_color);

    let raw_step = extent / 10.0;
    let magnitude = 10.0f32.powf(raw_step.log10().floor());
    let normalized = raw_step / magnitude;
    let step = if normalized < 2.0 {
        1.0
    } else if normalized < 5.0 {
        2.0
    } else {
        5.0
    } * magnitude;

    let grid_range = extent * 1.5;
    let start = (grid_range / step).ceil() * step;

    let grid_center_x = (center.x / step).round() * step;
    let grid_center_y = (center.y / step).round() * step;
    let grid_z = 0.0;

    let segments = 20;

    let mut x = grid_center_x - start;
    while x <= grid_center_x + start {
        for seg in 0..segments {
            let t1 = seg as f32 / segments as f32;
            let t2 = (seg + 1) as f32 / segments as f32;
            let y1 = grid_center_y - start + t1 * (2.0 * start);
            let y2 = grid_center_y - start + t2 * (2.0 * start);
            draw_line(
                Vec3::new(x, y1, grid_z),
                Vec3::new(x, y2, grid_z),
                grid_stroke,
            );
        }
        x += step;
    }

    let mut y = grid_center_y - start;
    while y <= grid_center_y + start {
        for seg in 0..segments {
            let t1 = seg as f32 / segments as f32;
            let t2 = (seg + 1) as f32 / segments as f32;
            let x1 = grid_center_x - start + t1 * (2.0 * start);
            let x2 = grid_center_x - start + t2 * (2.0 * start);
            draw_line(
                Vec3::new(x1, y, grid_z),
                Vec3::new(x2, y, grid_z),
                grid_stroke,
            );
        }
        y += step;
    }

    let axis_len = step;
    let origin = Vec3::ZERO;

    draw_line(
        origin,
        Vec3::new(axis_len, 0.0, 0.0),
        Stroke::new(2.0, Color32::RED),
    );
    if let Some((pos, _, w)) = project(Vec3::new(axis_len * 1.1, 0.0, 0.0)) {
        if w > 0.0 {
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                "N",
                egui::FontId::proportional(12.0),
                Color32::RED,
            );
        }
    }

    draw_line(
        origin,
        Vec3::new(0.0, axis_len, 0.0),
        Stroke::new(2.0, Color32::GREEN),
    );
    if let Some((pos, _, w)) = project(Vec3::new(0.0, axis_len * 1.1, 0.0)) {
        if w > 0.0 {
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                "E",
                egui::FontId::proportional(12.0),
                Color32::GREEN,
            );
        }
    }

    draw_line(
        origin,
        Vec3::new(0.0, 0.0, axis_len),
        Stroke::new(2.0, Color32::BLUE),
    );
    if let Some((pos, _, w)) = project(Vec3::new(0.0, 0.0, axis_len * 1.1)) {
        if w > 0.0 {
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                "D",
                egui::FontId::proportional(12.0),
                Color32::BLUE,
            );
        }
    }
}
