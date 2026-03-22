use super::FileDialogResult;
use std::process::Command;

pub fn open_file_dialog() -> FileDialogResult {
    // Try zenity first (GNOME), then kdialog (KDE)
    if let Some(path) = try_zenity_open() {
        return FileDialogResult::Selected(path);
    }
    if let Some(path) = try_kdialog_open() {
        return FileDialogResult::Selected(path);
    }
    log::warn!("No file dialog available (install zenity or kdialog)");
    FileDialogResult::Cancelled
}

pub fn save_file_dialog(default_name: &str) -> FileDialogResult {
    if let Some(path) = try_zenity_save(default_name) {
        return FileDialogResult::Selected(path);
    }
    if let Some(path) = try_kdialog_save(default_name) {
        return FileDialogResult::Selected(path);
    }
    log::warn!("No file dialog available (install zenity or kdialog)");
    FileDialogResult::Cancelled
}

fn try_zenity_open() -> Option<String> {
    let output = Command::new("zenity")
        .args([
            "--file-selection",
            "--title=Open Drawing",
            "--file-filter=*.wcanvas *.json",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

fn try_zenity_save(default_name: &str) -> Option<String> {
    let output = Command::new("zenity")
        .args([
            "--file-selection",
            "--save",
            "--confirm-overwrite",
            "--title=Save Drawing",
            &format!("--filename={}", default_name),
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

fn try_kdialog_open() -> Option<String> {
    let output = Command::new("kdialog")
        .args(["--getopenfilename", ".", "*.wcanvas *.json|Drawing files"])
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

fn try_kdialog_save(default_name: &str) -> Option<String> {
    let output = Command::new("kdialog")
        .args([
            "--getsavefilename",
            default_name,
            "*.wcanvas *.json|Drawing files",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}
