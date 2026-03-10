use std::fs;
use std::path::PathBuf;

use crate::error::AmuxError;

const PLUGIN_CONTENT: &str = include_str!("../../plugin/opencode/amux-status.js");

fn version_from_content(content: &str) -> Option<&str> {
    content.lines().next()?.strip_prefix("// amux-status ")
}

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
    let current_version = version_from_content(PLUGIN_CONTENT)
        .ok_or_else(|| AmuxError::Setup("could not parse version from plugin".to_string()))?;

    if let Ok(existing) = fs::read_to_string(&path)
        && version_from_content(&existing) == Some(current_version)
    {
        println!("Plugin already up to date.");
        return Ok(());
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
