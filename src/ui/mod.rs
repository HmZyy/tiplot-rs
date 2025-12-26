pub mod app;
pub mod layout;
pub mod menu;
pub mod panels;
pub mod renderer;
pub mod tiles;

use std::process::Command;

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

fn is_loader_available() -> bool {
    if std::env::var("TIPLOT_LOADER_COMMAND").is_ok() {
        return true;
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            #[cfg(unix)]
            let loader_path = exe_dir.join("tiplot-loader");

            #[cfg(windows)]
            let loader_path = exe_dir.join("tiplot-loader.exe");

            return loader_path.exists();
        }
    }

    false
}

fn launch_loader() -> Result<(), String> {
    if let Ok(cmd) = std::env::var("TIPLOT_LOADER_COMMAND") {
        return launch_command(&cmd);
    }

    launch_loader_executable()
}

fn launch_command(cmd: &str) -> Result<(), String> {
    #[cfg(unix)]
    let result = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .spawn()
        .map_err(|e| e.to_string());

    #[cfg(windows)]
    let result = Command::new("cmd")
        .arg("/C")
        .arg(cmd)
        .spawn()
        .map_err(|e| e.to_string());

    match result {
        Ok(_) => {
            eprintln!("✓ Launched loader: {}", cmd);
            Ok(())
        }
        Err(e) => {
            let msg = format!("Failed to launch command '{}': {}", cmd, e);
            eprintln!("✗ {}", msg);
            Err(msg)
        }
    }
}

fn launch_loader_executable() -> Result<(), String> {
    let exe_path =
        std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

    let exe_dir = exe_path
        .parent()
        .ok_or("Failed to get executable directory")?;

    #[cfg(unix)]
    let loader_path = exe_dir.join("tiplot-loader");

    #[cfg(windows)]
    let loader_path = exe_dir.join("tiplot-loader.exe");

    if !loader_path.exists() {
        let msg = format!(
            "No loader found. Set TIPLOT_LOADER_COMMAND or place 'tiplot-loader' executable in: {}",
            exe_dir.display()
        );
        eprintln!("✗ {}", msg);
        return Err(msg);
    }

    match Command::new(&loader_path).spawn() {
        Ok(_) => {
            eprintln!("✓ Launched loader: {}", loader_path.display());
            Ok(())
        }
        Err(e) => {
            let msg = format!("Failed to launch '{}': {}", loader_path.display(), e);
            eprintln!("✗ {}", msg);
            Err(msg)
        }
    }
}
