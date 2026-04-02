use std::fs;
use std::path::PathBuf;

use crate::error::AmuxError;

const PLUGIN_CONTENT: &str = include_str!("../../plugin/pi/amux-status.ts");

fn version_from_content(content: &str) -> Option<&str> {
    content.lines().next()?.strip_prefix("// amux-status ")
}

fn plugin_dir() -> Result<PathBuf, AmuxError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AmuxError::Setup("could not determine home directory".to_string()))?;
    Ok(home
        .join(".pi")
        .join("agent")
        .join("extensions")
        .join("amux-status"))
}

pub fn run() -> Result<(), AmuxError> {
    let dir = plugin_dir()?;
    let path = dir.join("index.ts");
    let current_version = version_from_content(PLUGIN_CONTENT)
        .ok_or_else(|| AmuxError::Setup("could not parse version from plugin".to_string()))?;

    if let Ok(existing) = fs::read_to_string(&path)
        && version_from_content(&existing) == Some(current_version)
    {
        println!("Plugin already up to date.");
        return Ok(());
    }

    fs::create_dir_all(&dir)
        .map_err(|e| AmuxError::Setup(format!("failed to create directory: {e}")))?;

    fs::write(&path, PLUGIN_CONTENT)
        .map_err(|e| AmuxError::Setup(format!("failed to write plugin: {e}")))?;

    println!("Plugin installed at {}", path.display());

    Ok(())
}
