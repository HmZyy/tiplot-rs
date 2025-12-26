use std::path::PathBuf;

pub fn get_assets_dir() -> PathBuf {
    if let Ok(assets_dir) = std::env::var("TIPLOT_ASSETS_DIR") {
        let path = PathBuf::from(assets_dir);
        if path.exists() {
            return path;
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let source_assets = exe_dir
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("assets"));

            if let Some(path) = source_assets {
                if path.exists() {
                    return path;
                }
            }

            let adjacent_assets = exe_dir.join("assets");
            if adjacent_assets.exists() {
                return adjacent_assets;
            }

            let system_assets = PathBuf::from("/usr/share/tiplot/assets");
            if system_assets.exists() {
                return system_assets;
            }
        }
    }

    PathBuf::from("assets")
}

pub fn get_model_path(filename: &str) -> PathBuf {
    get_assets_dir().join("models").join(filename)
}
