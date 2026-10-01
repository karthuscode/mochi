use crate::app_info::{app_info, AppInfo};

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    app_info()
}
