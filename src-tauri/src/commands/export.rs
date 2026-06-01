//! File export command
//!
//! The frontend cannot reliably save files via the browser `<a download>` /
//! `a.click()` pattern: Tauri's macOS WKWebView ignores the anchor `download`
//! attribute, so the click is silently dropped and no file is written. This
//! command performs the save from the Rust backend instead, which can write to
//! disk directly.

use std::fs;
use tauri::Manager;

/// Write `contents` to a file named `filename` in the user's Downloads
/// directory (falling back to the home directory) and return the absolute path
/// that was written.
#[tauri::command]
pub fn save_export_file(
    app: tauri::AppHandle,
    filename: String,
    contents: String,
) -> Result<String, String> {
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|e| format!("Could not resolve a directory to save to: {e}"))?;

    let path = dir.join(&filename);
    fs::write(&path, contents.as_bytes())
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;

    Ok(path.to_string_lossy().into_owned())
}
