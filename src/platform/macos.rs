use super::FileDialogResult;
use objc2_app_kit::{NSOpenPanel, NSSavePanel};
use objc2_foundation::NSString;

pub fn open_file_dialog() -> FileDialogResult {
    unsafe {
        let panel = NSOpenPanel::openPanel();
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(false);
        panel.setTitle(Some(&NSString::from_str("Open Drawing")));

        let response = panel.runModal();
        // NSModalResponseOK = 1
        if response.0 == 1 {
            if let Some(url) = panel.URL() {
                if let Some(path) = url.path() {
                    return FileDialogResult::Selected(path.to_string());
                }
            }
        }
        FileDialogResult::Cancelled
    }
}

pub fn save_file_dialog(default_name: &str) -> FileDialogResult {
    unsafe {
        let panel = NSSavePanel::savePanel();
        panel.setTitle(Some(&NSString::from_str("Save Drawing")));
        panel.setNameFieldStringValue(&NSString::from_str(default_name));
        panel.setCanCreateDirectories(true);

        let response = panel.runModal();
        if response.0 == 1 {
            if let Some(url) = panel.URL() {
                if let Some(path) = url.path() {
                    return FileDialogResult::Selected(path.to_string());
                }
            }
        }
        FileDialogResult::Cancelled
    }
}
