mod analysis;
mod app_info;
mod commands;
mod credentials;
mod local_capture;
mod provider;

use tauri::Manager;

pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let helper = if cfg!(debug_assertions) {
                std::env::current_exe()?
                    .parent()
                    .ok_or("Native executable location unavailable")?
                    .join("mochi-hook")
            } else {
                app.path().resource_dir()?.join("mochi-hook")
            };
            let core =
                local_capture::LocalCapture::open(data, helper).map_err(std::io::Error::other)?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::local_status,
            commands::list_projects,
            commands::approve_project,
            commands::preview_connection,
            commands::apply_connection,
            commands::set_tracking,
            commands::pause_all_capture,
            commands::list_sessions,
            commands::session_detail,
            commands::finish_session,
            commands::delete_session,
            commands::delete_project,
            commands::analysis_status,
            commands::analysis_permission,
            commands::store_api_key,
            commands::delete_api_key,
            commands::preview_analysis,
            commands::approve_analysis,
            commands::cancel_analysis,
            commands::learning_document,
            commands::submit_selfcheck,
            commands::reveal_selfcheck,
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
