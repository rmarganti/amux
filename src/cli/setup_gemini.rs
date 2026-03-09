use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::error::AmuxError;

const EXTENSION_MANIFEST: &str = include_str!("../../plugin/gemini/gemini-extension.json");
const HOOKS_JSON: &str = include_str!("../../plugin/gemini/hooks/hooks.json");
const HOOK_SCRIPT: &str = include_str!("../../plugin/gemini/hooks/amux-status.sh");

const VERSION_LINE: &str = "# amux-status v1.0";

fn extension_dir() -> Result<PathBuf, AmuxError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AmuxError::Setup("could not determine home directory".to_string()))?;
    Ok(home.join(".gemini").join("extensions").join("amux-status"))
}

pub fn run() -> Result<(), AmuxError> {
    let dir = extension_dir()?;
    let hook_script_path = dir.join("hooks").join("amux-status.sh");

    if let Ok(existing) = fs::read_to_string(&hook_script_path)
        && existing.lines().any(|line| line == VERSION_LINE)
    {
        println!("Gemini CLI extension already up to date.");
        return Ok(());
    }

    let hooks_dir = dir.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .map_err(|e| AmuxError::Setup(format!("failed to create directory: {e}")))?;

    fs::write(dir.join("gemini-extension.json"), EXTENSION_MANIFEST)
        .map_err(|e| AmuxError::Setup(format!("failed to write extension manifest: {e}")))?;

    fs::write(hooks_dir.join("hooks.json"), HOOKS_JSON)
        .map_err(|e| AmuxError::Setup(format!("failed to write hooks.json: {e}")))?;

    fs::write(&hook_script_path, HOOK_SCRIPT)
        .map_err(|e| AmuxError::Setup(format!("failed to write hook script: {e}")))?;

    fs::set_permissions(&hook_script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| AmuxError::Setup(format!("failed to set hook script permissions: {e}")))?;

    println!("Gemini CLI extension installed at {}", dir.display());

    Ok(())
}
