use std::fs;
use std::path::PathBuf;

use crate::error::AmuxError;

const PLUGIN_CONTENT: &str = include_str!("../../plugin/opencode/amux-status.js");

fn version_from_content(content: &str) -> Option<&str> {
    content.lines().next()?.strip_prefix("// amux-status ")
}

fn config_dir() -> Result<PathBuf, AmuxError> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(xdg))
    } else {
        Ok(dirs::home_dir()
            .ok_or_else(|| AmuxError::Setup("could not determine home directory".to_string()))?
            .join(".config"))
    }
}

fn plugin_path() -> Result<PathBuf, AmuxError> {
    Ok(config_dir()?
        .join("opencode")
        .join("plugins")
        .join("amux-status.js"))
}

fn legacy_plugin_path() -> Result<PathBuf, AmuxError> {
    Ok(config_dir()?
        .join("opencode")
        .join("plugin")
        .join("amux-status.js"))
}

pub fn run() -> Result<(), AmuxError> {
    let path = plugin_path()?;
    let legacy_path = legacy_plugin_path()?;
    let current_version = version_from_content(PLUGIN_CONTENT)
        .ok_or_else(|| AmuxError::Setup("could not parse version from plugin".to_string()))?;

    let current_installed = fs::read_to_string(&path)
        .ok()
        .is_some_and(|existing| version_from_content(&existing) == Some(current_version));

    if current_installed {
        if legacy_path.exists() {
            let _ = fs::remove_file(&legacy_path);
        }
        println!("Plugin already up to date.");
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AmuxError::Setup(format!("failed to create directory: {e}")))?;
    }

    fs::write(&path, PLUGIN_CONTENT)
        .map_err(|e| AmuxError::Setup(format!("failed to write plugin: {e}")))?;

    if legacy_path.exists() {
        let _ = fs::remove_file(&legacy_path);
    }

    println!("Plugin installed at {}", path.display());

    Ok(())
}
