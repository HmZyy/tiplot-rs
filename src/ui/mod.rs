pub mod app;
pub mod layout;
pub mod menu;
pub mod panels;
pub mod renderer;
pub mod tiles;

const COLOR_PALETTE: [[f32; 4]; 10] = [
    [0.12, 0.47, 0.71, 1.0], // Blue
    [1.00, 0.50, 0.05, 1.0], // Orange
    [0.17, 0.63, 0.17, 1.0], // Green
    [0.84, 0.15, 0.16, 1.0], // Red
    [0.58, 0.40, 0.74, 1.0], // Purple
    [0.55, 0.34, 0.29, 1.0], // Brown
    [0.89, 0.47, 0.76, 1.0], // Pink
    [0.50, 0.50, 0.50, 1.0], // Gray
    [0.74, 0.74, 0.13, 1.0], // Yellow
    [0.09, 0.75, 0.81, 1.0], // Cyan
];

pub fn get_trace_color(index: usize) -> [f32; 4] {
    COLOR_PALETTE[index % COLOR_PALETTE.len()]
}

pub fn calculate_grid_step(range: f32, target_steps: usize) -> f32 {
    if range == 0.0 {
        return 1.0;
    }

    let raw_step = range / target_steps as f32;
    let mag = 10.0_f32.powf(raw_step.log10().floor());
    let normalized_step = raw_step / mag;

    let nice_step = if normalized_step < 2.0 {
        1.0
    } else if normalized_step < 5.0 {
        2.0
    } else {
        5.0
    };

    nice_step * mag
}
