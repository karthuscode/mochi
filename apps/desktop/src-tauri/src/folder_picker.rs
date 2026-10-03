//! User-initiated directory selection only. This does not authorize or scan it.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct FolderPicker(pub Arc<AtomicBool>);
struct PickerLease(Arc<AtomicBool>);
impl Drop for PickerLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
#[tauri::command]
pub async fn pick_project_folder(
    window: tauri::WebviewWindow,
    picker: tauri::State<'_, FolderPicker>,
) -> Result<Option<String>, &'static str> {
    if window.label() != "main" {
        return Err("Folder picker unavailable.");
    }
    if picker
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Folder picker already open.");
    }
    let lease = PickerLease(picker.0.clone());
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        let picked = app
            .dialog()
            .file()
            .set_title("Choose your project folder")
            .blocking_pick_folder();
        picked
            .map(|value| {
                let path = value
                    .into_path()
                    .map_err(|_| "Folder selection unavailable.")?;
                let path = path.to_str().ok_or("Folder selection unavailable.")?;
                if !path.starts_with('/') || path.len() > 4096 || path.chars().any(char::is_control)
                {
                    return Err("Folder selection unavailable.");
                }
                Ok(path.to_owned())
            })
            .transpose()
    })
    .await
    .map_err(|_| "Folder picker unavailable.")?
}
