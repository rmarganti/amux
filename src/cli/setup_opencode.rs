use std::fs;
use std::path::PathBuf;

use crate::error::AmuxError;

const PLUGIN_CONTENT: &str = include_str!("../../plugin/amux-status.js");
const VERSION_LINE: &str = "// amux-status v1.1";

fn plugin_path() -> Result<PathBuf, AmuxError> {
    let config_dir = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        dirs::home_dir()
            .ok_or_else(|| AmuxError::Setup("could not determine home directory".to_string()))?
            .join(".config")
    };
    Ok(config_dir
        .join("opencode")
        .join("plugin")
        .join("amux-status.js"))
}

pub fn run() -> Result<(), AmuxError> {
    let path = plugin_path()?;

    if let Ok(existing) = fs::read_to_string(&path) {
        if let Some(first_line) = existing.lines().next() {
            if first_line == VERSION_LINE {
                println!("Plugin already up to date.");
                return Ok(());
            }
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AmuxError::Setup(format!("failed to create directory: {e}")))?;
    }

    fs::write(&path, PLUGIN_CONTENT)
        .map_err(|e| AmuxError::Setup(format!("failed to write plugin: {e}")))?;

    println!("Plugin installed at {}", path.display());

    Ok(())
}
