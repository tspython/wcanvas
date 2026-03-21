use super::FileDialogResult;
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel, NSSavePanel};
use objc2_foundation::{MainThreadMarker, NSString};

pub fn open_file_dialog() -> FileDialogResult {
    unsafe {
        let mtm = MainThreadMarker::new().expect("file dialog must run on main thread");
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(false);
        panel.setTitle(Some(&NSString::from_str("Open Drawing")));

        let response = panel.runModal();
        if response == NSModalResponseOK {
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
        let mtm = MainThreadMarker::new().expect("file dialog must run on main thread");
        let panel = NSSavePanel::savePanel(mtm);
        panel.setTitle(Some(&NSString::from_str("Save Drawing")));
        panel.setNameFieldStringValue(&NSString::from_str(default_name));
        panel.setCanCreateDirectories(true);

        let response = panel.runModal();
        if response == NSModalResponseOK {
            if let Some(url) = panel.URL() {
                if let Some(path) = url.path() {
                    return FileDialogResult::Selected(path.to_string());
                }
            }
        }
        FileDialogResult::Cancelled
    }
}
