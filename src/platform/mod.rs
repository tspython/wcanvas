#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_arch = "wasm32")]
mod wasm;

/// Result of a file dialog operation.
pub enum FileDialogResult {
    /// User selected a file path.
    Selected(String),
    /// User cancelled the dialog.
    Cancelled,
}

/// Show a native "Open File" dialog. Returns the selected file path or Cancelled.
pub fn open_file_dialog() -> FileDialogResult {
    #[cfg(target_arch = "wasm32")]
    {
        return FileDialogResult::Cancelled;
    }
    #[cfg(target_os = "macos")]
    {
        macos::open_file_dialog()
    }
    #[cfg(target_os = "linux")]
    {
        linux::open_file_dialog()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_arch = "wasm32")))]
    {
        log::warn!("File dialogs not supported on this platform");
        FileDialogResult::Cancelled
    }
}

/// Show a native "Save File" dialog. Returns the selected file path or Cancelled.
pub fn save_file_dialog(default_name: &str) -> FileDialogResult {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = default_name;
        return FileDialogResult::Cancelled;
    }
    #[cfg(target_os = "macos")]
    {
        macos::save_file_dialog(default_name)
    }
    #[cfg(target_os = "linux")]
    {
        linux::save_file_dialog(default_name)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_arch = "wasm32")))]
    {
        log::warn!("File dialogs not supported on this platform");
        FileDialogResult::Cancelled
    }
}

/// Save document JSON to the filesystem (native only).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_to_file(path: &str, content: &str) -> Result<(), std::io::Error> {
    use std::io::Write;
    // Atomic write: write to temp file then rename
    let tmp_path = format!("{}.tmp", path);
    let mut file = std::fs::File::create(&tmp_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Load document JSON from the filesystem (native only).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

/// Get the auto-save directory path, creating it if needed.
#[cfg(not(target_arch = "wasm32"))]
pub fn autosave_dir() -> Result<std::path::PathBuf, std::io::Error> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::PathBuf::from(home).join(".wcanvas");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Get the auto-save file path.
#[cfg(not(target_arch = "wasm32"))]
pub fn autosave_path() -> Result<std::path::PathBuf, std::io::Error> {
    Ok(autosave_dir()?.join("autosave.wcanvas"))
}

// WASM persistence functions
#[cfg(target_arch = "wasm32")]
pub fn save_to_local_storage(key: &str, json: &str) {
    wasm::save_to_local_storage(key, json);
}

#[cfg(target_arch = "wasm32")]
pub fn load_from_local_storage(key: &str) -> Option<String> {
    wasm::load_from_local_storage(key)
}

#[cfg(target_arch = "wasm32")]
pub fn trigger_download(filename: &str, content: &str) {
    wasm::trigger_download(filename, content);
}

#[cfg(target_arch = "wasm32")]
pub fn trigger_file_open(callback: impl FnOnce(String) + 'static) {
    wasm::trigger_file_open(callback);
}
