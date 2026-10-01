mod app_info;
mod commands;

use mochi_persistence::SqliteStore;
use tauri::Manager;

pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("mochi.sqlite3");
            let store = SqliteStore::open_system(database_path)?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .run(tauri::generate_context!())
}
