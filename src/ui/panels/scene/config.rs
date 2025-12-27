use crate::{core::DataStore, ui::tiles::InterpolationMode};
use eframe::egui;
use egui_phosphor::regular as icons;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const EARTH_RADIUS: f64 = 6378137.0;

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VehicleType {
    FixedWing,
    QuadCopter,
    DeltaWing,
}

impl VehicleType {
    pub fn default_scale(&self) -> f32 {
        match self {
            VehicleType::FixedWing => 1.0,
            VehicleType::QuadCopter => 1.0,
            VehicleType::DeltaWing => 1.0,
        }
    }

    pub fn model_path(&self) -> String {
        match self {
            VehicleType::FixedWing => "FixedWing".to_string(),
            VehicleType::QuadCopter => "QuadCopter".to_string(),
            VehicleType::DeltaWing => "DeltaWing".to_string(),
        }
    }

    pub fn orientation_offset(&self) -> glam::Vec3 {
        match self {
            VehicleType::FixedWing => glam::Vec3::new(0.0, 0.0, 0.0),
            VehicleType::QuadCopter => glam::Vec3::new(0.0, -std::f32::consts::FRAC_PI_2, 0.0),
            VehicleType::DeltaWing => glam::Vec3::new(0.0, -std::f32::consts::FRAC_PI_2, 0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AngleUnit {
    Radians,
    Degrees,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OrientationMode {
    Static,
    Quaternion {
        topic: String,
        qx: String,
        qy: String,
        qz: String,
        qw: String,
    },
    Euler {
        topic: String,
        roll: String,
        pitch: String,
        yaw: String,
        angle_unit: AngleUnit,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PositionMode {
    LocalNED {
        topic: String,
        north: String,
        east: String,
        down: String,
        lat_ref: String,
        lon_ref: String,
        alt_ref: String,
    },
    GlobalGPS {
        topic: String,
        lat: String,
        lon: String,
        alt: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VehicleConfig {
    pub id: Uuid,
    pub name: String,
    pub vehicle_type: VehicleType,
    pub color: [f32; 3],
    pub path_color: [f32; 3],
    pub scale: f32,
    pub orientation: OrientationMode,
    pub position: PositionMode,
    pub visible: bool,
}

impl Default for VehicleConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "New Vehicle".to_string(),
            vehicle_type: VehicleType::QuadCopter,
            color: [1.0, 0.0, 0.0],
            path_color: [0.0, 0.5, 1.0],
            scale: 1.0,
            orientation: OrientationMode::Quaternion {
                topic: "vehicle_attitude".to_string(),
                qx: "q[1]".to_string(),
                qy: "q[2]".to_string(),
                qz: "q[3]".to_string(),
                qw: "q[0]".to_string(),
            },
            position: PositionMode::LocalNED {
                topic: "vehicle_local_position".to_string(),
                north: "x".to_string(),
                east: "y".to_string(),
                down: "z".to_string(),
                lat_ref: "ref_lat".to_string(),
                lon_ref: "ref_lon".to_string(),
                alt_ref: "ref_alt".to_string(),
            },
            visible: true,
        }
    }
}

impl VehicleConfig {
    pub fn evaluate_at(
        &self,
        data_store: &DataStore,
        t: f32,
        interpolation_mode: InterpolationMode,
    ) -> (glam::Vec3, glam::Quat) {
        let pos = self.evaluate_position(data_store, t, interpolation_mode);
        let rot = self.evaluate_orientation(data_store, t, interpolation_mode);
        (pos, rot)
    }

    fn get_value_at(
        data_store: &DataStore,
        topic: &str,
        col: &str,
        t: f32,
        interpolation_mode: InterpolationMode,
    ) -> f32 {
        if let Some(timestamps) = data_store.get_column(topic, "timestamp") {
            if let Some(values) = data_store.get_column(topic, col) {
                if timestamps.is_empty() || values.is_empty() {
                    return 0.0;
                }

                return Self::interpolate_value(timestamps, values, t, interpolation_mode)
                    .unwrap_or(0.0);
            }
        }
        0.0
    }

    pub fn gps_to_ned(
        lat: f64,
        lon: f64,
        alt: f64,
        lat_ref: f64,
        lon_ref: f64,
        alt_ref: f64,
    ) -> glam::Vec3 {
        let lat_rad = lat.to_radians();
        let lon_rad = lon.to_radians();
        let lat_ref_rad = lat_ref.to_radians();
        let lon_ref_rad = lon_ref.to_radians();

        let d_lat = lat_rad - lat_ref_rad;
        let d_lon = lon_rad - lon_ref_rad;

        let north = (d_lat * EARTH_RADIUS) as f32;
        let east = (d_lon * EARTH_RADIUS * lat_ref_rad.cos()) as f32;
        let down = -(alt - alt_ref) as f32;

        glam::Vec3::new(north, east, down)
    }

    fn interpolate_value(
        times: &[f32],
        values: &[f32],
        t: f32,
        mode: InterpolationMode,
    ) -> Option<f32> {
        match mode {
            InterpolationMode::PreviousPoint => {
                let idx = times.partition_point(|&time| time < t);
                if idx == 0 {
                    None
                } else {
                    let prev_idx = idx - 1;
                    if prev_idx < values.len() {
                        Some(values[prev_idx])
                    } else {
                        None
                    }
                }
            }
            InterpolationMode::NextPoint => {
                let idx = times.partition_point(|&time| time <= t);
                if idx >= times.len() {
                    None
                } else if idx < values.len() {
                    Some(values[idx])
                } else {
                    None
                }
            }
            InterpolationMode::Linear => {
                let idx = times.partition_point(|&time| time < t);

                if idx == 0 {
                    None
                } else if idx >= times.len() {
                    if !times.is_empty() && times.len() == values.len() {
                        Some(values[values.len() - 1])
                    } else {
                        None
                    }
                } else {
                    let prev_idx = idx - 1;
                    if prev_idx < values.len() && idx < values.len() {
                        let t0 = times[prev_idx];
                        let t1 = times[idx];
                        let v0 = values[prev_idx];
                        let v1 = values[idx];

                        if (t1 - t0).abs() < 1e-6 {
                            Some(v0)
                        } else {
                            let alpha = (t - t0) / (t1 - t0);
                            Some(v0 + alpha * (v1 - v0))
                        }
                    } else {
                        None
                    }
                }
            }
        }
    }

    fn evaluate_position(
        &self,
        ds: &DataStore,
        t: f32,
        interpolation_mode: InterpolationMode,
    ) -> glam::Vec3 {
        match &self.position {
            PositionMode::LocalNED {
                topic,
                north,
                east,
                down,
                ..
            } => {
                let x = Self::get_value_at(ds, topic, north, t, interpolation_mode);
                let y = Self::get_value_at(ds, topic, east, t, interpolation_mode);
                let z = Self::get_value_at(ds, topic, down, t, interpolation_mode);
                glam::Vec3::new(x, y, z)
            }
            PositionMode::GlobalGPS {
                topic,
                lat,
                lon,
                alt,
            } => {
                let (lat_ref, lon_ref, alt_ref) =
                    if let Some(timestamps) = ds.get_column(topic, "timestamp") {
                        if !timestamps.is_empty() {
                            let lat_ref = Self::get_value_at(
                                ds,
                                topic,
                                lat,
                                timestamps[0],
                                InterpolationMode::PreviousPoint,
                            ) as f64;
                            let lon_ref = Self::get_value_at(
                                ds,
                                topic,
                                lon,
                                timestamps[0],
                                InterpolationMode::PreviousPoint,
                            ) as f64;
                            let alt_ref = Self::get_value_at(
                                ds,
                                topic,
                                alt,
                                timestamps[0],
                                InterpolationMode::PreviousPoint,
                            ) as f64;
                            (lat_ref, lon_ref, alt_ref)
                        } else {
                            (0.0, 0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0, 0.0)
                    };

                let lat_val = Self::get_value_at(ds, topic, lat, t, interpolation_mode) as f64;
                let lon_val = Self::get_value_at(ds, topic, lon, t, interpolation_mode) as f64;
                let alt_val = Self::get_value_at(ds, topic, alt, t, interpolation_mode) as f64;

                Self::gps_to_ned(lat_val, lon_val, alt_val, lat_ref, lon_ref, alt_ref)
            }
        }
    }

    fn evaluate_orientation(
        &self,
        ds: &DataStore,
        t: f32,
        interpolation_mode: InterpolationMode,
    ) -> glam::Quat {
        match &self.orientation {
            OrientationMode::Static => glam::Quat::IDENTITY,
            OrientationMode::Quaternion {
                topic,
                qx,
                qy,
                qz,
                qw,
            } => {
                let x = Self::get_value_at(ds, topic, qx, t, interpolation_mode);
                let y = Self::get_value_at(ds, topic, qy, t, interpolation_mode);
                let z = Self::get_value_at(ds, topic, qz, t, interpolation_mode);
                let w = Self::get_value_at(ds, topic, qw, t, interpolation_mode);

                let q = glam::Quat::from_xyzw(x, y, z, w);
                if q.length_squared() < 1e-6 {
                    glam::Quat::IDENTITY
                } else {
                    q.normalize()
                }
            }
            OrientationMode::Euler {
                topic,
                roll,
                pitch,
                yaw,
                angle_unit,
            } => {
                let mut r = Self::get_value_at(ds, topic, roll, t, interpolation_mode);
                let mut p = Self::get_value_at(ds, topic, pitch, t, interpolation_mode);
                let mut y = Self::get_value_at(ds, topic, yaw, t, interpolation_mode);

                if matches!(angle_unit, AngleUnit::Degrees) {
                    r = r.to_radians();
                    p = p.to_radians();
                    y = y.to_radians();
                }

                glam::Quat::from_euler(glam::EulerRot::XYZ, r, p, y)
            }
        }
    }
}

pub fn render_configuration_tab(
    ui: &mut egui::Ui,
    vehicles: &mut Vec<VehicleConfig>,
    data_store: &DataStore,
) {
    ui.add_space(10.0);
    if ui.button(format!("{} Add Vehicle", icons::PLUS)).clicked() {
        vehicles.push(VehicleConfig::default());
    }
    ui.separator();

    let mut remove_idx = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (idx, vehicle) in vehicles.iter_mut().enumerate() {
            let vehicle_id = vehicle.id;
            ui.push_id(vehicle_id, |ui| {
                let header_text = format!("Vehicle #{}", idx + 1);

                egui::CollapsingHeader::new(header_text)
                    .id_salt(vehicle_id)
                    .default_open(true)
                    .show(ui, |ui| {
                        render_vehicle_config(ui, vehicle, data_store);

                        ui.add_space(10.0);

                        if ui
                            .button(format!("{} Remove Vehicle", icons::TRASH))
                            .clicked()
                        {
                            remove_idx = Some(idx);
                        }
                    });
            });
            ui.separator();
        }
    });

    if let Some(idx) = remove_idx {
        vehicles.remove(idx);
    }
}

fn render_vehicle_config(ui: &mut egui::Ui, vehicle: &mut VehicleConfig, ds: &DataStore) {
    egui::Grid::new("vehicle_grid")
        .num_columns(2)
        .spacing([40.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut vehicle.name);
            ui.end_row();

            ui.label("Type");
            egui::ComboBox::from_id_salt("v_type")
                .selected_text(format!("{:?}", vehicle.vehicle_type))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut vehicle.vehicle_type,
                            VehicleType::FixedWing,
                            "FixedWing",
                        )
                        .clicked()
                    {
                        vehicle.scale = vehicle.vehicle_type.default_scale();
                    }
                    if ui
                        .selectable_value(
                            &mut vehicle.vehicle_type,
                            VehicleType::QuadCopter,
                            "QuadCopter",
                        )
                        .clicked()
                    {
                        vehicle.scale = vehicle.vehicle_type.default_scale();
                    }
                    if ui
                        .selectable_value(
                            &mut vehicle.vehicle_type,
                            VehicleType::DeltaWing,
                            "DeltaWing",
                        )
                        .clicked()
                    {
                        vehicle.scale = vehicle.vehicle_type.default_scale();
                    }
                });
            ui.end_row();

            ui.label("Vehicle Color");
            ui.color_edit_button_rgb(&mut vehicle.color);
            ui.end_row();

            ui.label("Path Color");
            ui.color_edit_button_rgb(&mut vehicle.path_color);
            ui.end_row();

            ui.label("Scale");
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add(
                        egui::DragValue::new(&mut vehicle.scale)
                            .speed(0.1)
                            .range(0.1..=100.0),
                    );
                    ui.add(egui::Slider::new(&mut vehicle.scale, 0.1..=100.0).show_value(false));
                },
            );
            ui.end_row();

            ui.label(egui::RichText::new("Orientation").strong());
            ui.horizontal(|ui| {
                ui.selectable_value(&mut vehicle.orientation, OrientationMode::Static, "Static");

                let is_quat = matches!(vehicle.orientation, OrientationMode::Quaternion { .. });
                if ui.selectable_label(is_quat, "Quaternion").clicked() {
                    vehicle.orientation = OrientationMode::Quaternion {
                        topic: "".to_string(),
                        qx: "qx".to_string(),
                        qy: "qy".to_string(),
                        qz: "qz".to_string(),
                        qw: "qw".to_string(),
                    };
                }

                let is_euler = matches!(vehicle.orientation, OrientationMode::Euler { .. });
                if ui.selectable_label(is_euler, "Euler").clicked() {
                    vehicle.orientation = OrientationMode::Euler {
                        topic: "".to_string(),
                        roll: "roll".to_string(),
                        pitch: "pitch".to_string(),
                        yaw: "yaw".to_string(),
                        angle_unit: AngleUnit::Radians,
                    };
                }
            });
            ui.end_row();

            match &mut vehicle.orientation {
                OrientationMode::Static => {
                    ui.label("Info");
                    ui.label("Uses Identity rotation");
                    ui.end_row();
                }
                OrientationMode::Quaternion {
                    topic,
                    qx,
                    qy,
                    qz,
                    qw,
                } => {
                    render_topic_selector(ui, ds, topic, "Orient. Topic");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, qx, "QX");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, qy, "QY");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, qz, "QZ");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, qw, "QW");
                    ui.end_row();
                }
                OrientationMode::Euler {
                    topic,
                    roll,
                    pitch,
                    yaw,
                    angle_unit,
                } => {
                    render_topic_selector(ui, ds, topic, "Orient. Topic");
                    ui.end_row();

                    ui.label("Angle Unit");
                    ui.horizontal(|ui| {
                        ui.selectable_value(angle_unit, AngleUnit::Radians, "Radians");
                        ui.selectable_value(angle_unit, AngleUnit::Degrees, "Degrees");
                    });
                    ui.end_row();

                    render_col_selector(ui, ds, topic, roll, "Roll");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, pitch, "Pitch");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, yaw, "Yaw");
                    ui.end_row();
                }
            }

            ui.label("");
            ui.end_row();

            ui.label(egui::RichText::new("Position").strong());
            ui.horizontal(|ui| {
                let is_ned = matches!(vehicle.position, PositionMode::LocalNED { .. });
                if ui.selectable_label(is_ned, "Local (NED)").clicked() {
                    vehicle.position = PositionMode::LocalNED {
                        topic: "".to_string(),
                        north: "x".to_string(),
                        east: "y".to_string(),
                        down: "z".to_string(),
                        lat_ref: "ref_lat".to_string(),
                        lon_ref: "ref_lon".to_string(),
                        alt_ref: "ref_alt".to_string(),
                    };
                }

                let is_gps = matches!(vehicle.position, PositionMode::GlobalGPS { .. });
                if ui.selectable_label(is_gps, "Global (GPS)").clicked() {
                    vehicle.position = PositionMode::GlobalGPS {
                        topic: "".to_string(),
                        lat: "lat".to_string(),
                        lon: "lon".to_string(),
                        alt: "alt".to_string(),
                    };
                }
            });
            ui.end_row();

            match &mut vehicle.position {
                PositionMode::LocalNED {
                    topic,
                    north,
                    east,
                    down,
                    lat_ref,
                    lon_ref,
                    alt_ref,
                } => {
                    render_topic_selector(ui, ds, topic, "Pos. Topic");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, north, "North (X)");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, east, "East (Y)");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, down, "Down (Z)");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, lat_ref, "Ref Latitude");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, lon_ref, "Ref Longitude");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, alt_ref, "Ref Altitude");
                    ui.end_row();
                }
                PositionMode::GlobalGPS {
                    topic,
                    lat,
                    lon,
                    alt,
                } => {
                    render_topic_selector(ui, ds, topic, "Pos. Topic");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, lat, "Latitude");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, lon, "Longitude");
                    ui.end_row();
                    render_col_selector(ui, ds, topic, alt, "Altitude");
                    ui.end_row();

                    ui.label("Info");
                    ui.label("Uses first position as origin");
                    ui.end_row();
                }
            }
        });
}

fn render_topic_selector(ui: &mut egui::Ui, ds: &DataStore, selected: &mut String, label: &str) {
    ui.label(label);

    let popup_id = ui.make_persistent_id(format!("topic_popup_{}", label));
    let filter_id = ui.make_persistent_id(format!("topic_filter_{}", label));

    let button_text = if selected.is_empty() {
        "Select Topic...".to_string()
    } else {
        selected.clone()
    };

    let response = ui.button(&button_text);

    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }

    egui::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(200.0);
            ui.set_max_height(300.0);

            let mut filter =
                ui.memory_mut(|mem| mem.data.get_temp::<String>(filter_id).unwrap_or_default());

            ui.horizontal(|ui| {
                ui.label("🔍");
                let filter_response = ui.text_edit_singleline(&mut filter);
                if ui.button("✖").clicked() {
                    filter.clear();
                }

                if response.clicked() {
                    filter_response.request_focus();
                }
            });

            ui.memory_mut(|mem| {
                mem.data.insert_temp(filter_id, filter.clone());
            });

            ui.separator();

            let filter_lower = filter.to_lowercase();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let topics = ds.get_topics();
                let mut found_any = false;

                for topic in topics {
                    if fuzzy_match(&topic.to_lowercase(), &filter_lower) {
                        found_any = true;
                        if ui.selectable_label(*selected == *topic, &*topic).clicked() {
                            *selected = topic.clone();
                            ui.memory_mut(|mem| {
                                mem.close_popup();
                                mem.data.remove::<String>(filter_id);
                            });
                        }
                    }
                }

                if !found_any && !filter.is_empty() {
                    ui.label(egui::RichText::new("No matches").italics().weak());
                }
            });
        },
    );
}

fn render_col_selector(
    ui: &mut egui::Ui,
    ds: &DataStore,
    topic: &str,
    selected: &mut String,
    label: &str,
) {
    ui.label(label);

    if topic.is_empty() {
        ui.label(egui::RichText::new("Select topic first").italics().weak());
        return;
    }

    let popup_id = ui.make_persistent_id(format!("col_popup_{}_{}", topic, label));
    let filter_id = ui.make_persistent_id(format!("col_filter_{}_{}", topic, label));

    let button_text = if selected.is_empty() {
        "Select Column...".to_string()
    } else {
        selected.clone()
    };

    let response = ui.button(&button_text);

    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }

    egui::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(200.0);
            ui.set_max_height(300.0);

            let mut filter =
                ui.memory_mut(|mem| mem.data.get_temp::<String>(filter_id).unwrap_or_default());

            ui.horizontal(|ui| {
                ui.label("🔍");
                let filter_response = ui.text_edit_singleline(&mut filter);
                if ui.button("✖").clicked() {
                    filter.clear();
                }

                if response.clicked() {
                    filter_response.request_focus();
                }
            });

            ui.memory_mut(|mem| {
                mem.data.insert_temp(filter_id, filter.clone());
            });

            ui.separator();

            let filter_lower = filter.to_lowercase();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let cols = ds.get_columns(topic);
                let mut found_any = false;

                for col in cols {
                    if fuzzy_match(&col.to_lowercase(), &filter_lower) {
                        found_any = true;
                        if ui.selectable_label(*selected == *col, &*col).clicked() {
                            *selected = col.clone();
                            ui.memory_mut(|mem| {
                                mem.close_popup();
                                mem.data.remove::<String>(filter_id);
                            });
                        }
                    }
                }

                if !found_any && !filter.is_empty() {
                    ui.label(egui::RichText::new("No matches").italics().weak());
                }
            });
        },
    );
}
