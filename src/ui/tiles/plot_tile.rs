use crate::core::DataStore;

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum InterpolationMode {
    PreviousPoint,
    Linear,
    NextPoint,
}

impl Default for InterpolationMode {
    fn default() -> Self {
        Self::PreviousPoint
    }
}

#[derive(Clone, Debug)]
pub struct TraceConfig {
    pub topic: String,

    pub col: String,

    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct PlotTile {
    pub traces: Vec<TraceConfig>,

    pub show_legend: bool,
    pub show_hover_tooltip: bool,
    pub show_hover_circles: bool,
    pub scatter_mode: bool,

    pub cached_tooltip_time: f32,
    pub cached_tooltip_values: Vec<Option<f32>>,

    pub show_info_window: bool,
    pub cached_for_playback: bool,

    pub interpolation_mode: InterpolationMode,
}

impl PlotTile {
    pub fn new() -> Self {
        Self {
            traces: Vec::new(),
            show_legend: false,
            show_hover_tooltip: true,
            show_hover_circles: true,
            scatter_mode: false,
            cached_tooltip_time: f32::NEG_INFINITY,
            cached_tooltip_values: Vec::new(),
            show_info_window: false,
            cached_for_playback: false,
            interpolation_mode: InterpolationMode::default(),
        }
    }

    pub fn add_trace(&mut self, topic: String, col: String, color: [f32; 4]) {
        self.traces.push(TraceConfig { topic, col, color });
    }

    pub fn _is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    pub fn trace_count(&self) -> usize {
        self.traces.len()
    }

    pub fn update_tooltip_cache(
        &mut self,
        hover_time: f32,
        data_store: &DataStore,
        for_playback: bool,
    ) {
        const EPSILON: f32 = 0.001;

        if (hover_time - self.cached_tooltip_time).abs() < EPSILON
            && self.cached_for_playback == for_playback
        {
            return;
        }

        self.cached_tooltip_time = hover_time;
        self.cached_for_playback = for_playback;
        self.cached_tooltip_values.clear();

        for trace in &self.traces {
            let value = if let (Some(times), Some(values)) = (
                data_store.get_column(&trace.topic, "timestamp"),
                data_store.get_column(&trace.topic, &trace.col),
            ) {
                if times.is_empty() {
                    None
                } else {
                    self.interpolate_value(times, values, hover_time)
                }
            } else {
                None
            };

            self.cached_tooltip_values.push(value);
        }
    }

    fn interpolate_value(&self, times: &[f32], values: &[f32], hover_time: f32) -> Option<f32> {
        match self.interpolation_mode {
            InterpolationMode::PreviousPoint => {
                let idx = times.partition_point(|&t| t < hover_time);
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
                let idx = times.partition_point(|&t| t <= hover_time);
                if idx >= times.len() {
                    None
                } else if idx < values.len() {
                    Some(values[idx])
                } else {
                    None
                }
            }
            InterpolationMode::Linear => {
                let idx = times.partition_point(|&t| t < hover_time);

                if idx == 0 {
                    None
                } else if idx >= times.len() {
                    if !times.is_empty() && times.len() == values.len() {
                        Some(values[values.len() - 1])
                    } else {
                        None
                    }
                } else {
                    // Between two points - interpolate
                    let prev_idx = idx - 1;
                    if prev_idx < values.len() && idx < values.len() {
                        let t0 = times[prev_idx];
                        let t1 = times[idx];
                        let v0 = values[prev_idx];
                        let v1 = values[idx];

                        if (t1 - t0).abs() < 1e-6 {
                            Some(v0)
                        } else {
                            let t = (hover_time - t0) / (t1 - t0);
                            Some(v0 + t * (v1 - v0))
                        }
                    } else {
                        None
                    }
                }
            }
        }
    }
}

impl Default for PlotTile {
    fn default() -> Self {
        Self::new()
    }
}
