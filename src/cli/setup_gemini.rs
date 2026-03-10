use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::error::AmuxError;

const EXTENSION_MANIFEST: &str = include_str!("../../plugin/gemini/gemini-extension.json");
const HOOKS_JSON: &str = include_str!("../../plugin/gemini/hooks/hooks.json");
const HOOK_SCRIPT: &str = include_str!("../../plugin/gemini/hooks/amux-status.sh");

fn version_from_content(content: &str) -> Option<&str> {
    content.lines().next()?.strip_prefix("# amux-status ")
}

fn extension_dir() -> Result<PathBuf, AmuxError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AmuxError::Setup("could not determine home directory".to_string()))?;
    Ok(home.join(".gemini").join("extensions").join("amux-status"))
}

pub fn run() -> Result<(), AmuxError> {
    let dir = extension_dir()?;
    let hook_script_path = dir.join("hooks").join("amux-status.sh");
    let current_version = version_from_content(HOOK_SCRIPT)
        .ok_or_else(|| AmuxError::Setup("could not parse version from hook script".to_string()))?;

    if let Ok(existing) = fs::read_to_string(&hook_script_path)
        && version_from_content(&existing) == Some(current_version)
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
