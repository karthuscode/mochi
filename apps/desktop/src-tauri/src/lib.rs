mod analysis;
mod app_info;
mod commands;
mod companion;
mod credentials;
mod local_capture;
mod provider;

use tauri::Manager;

pub fn run() -> Result<(), tauri::Error> {
    let companion_preview = companion::preview_requested();
    let mut context = tauri::generate_context!();
    if companion_preview {
        if let Some(main) = context
            .config_mut()
            .app
            .windows
            .iter_mut()
            .find(|window| window.label == "main")
        {
            main.url = tauri::WebviewUrl::App("companion-preview.html".into());
            main.title = "mochi — companion preview".into();
        }
    }
    tauri::Builder::default()
        .setup(move |app| {
            companion::setup(app)?;
            if companion_preview {
                companion::show(app.handle())?;
                return Ok(());
            }
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
        .on_window_event(move |window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if companion_preview {
                        api.prevent_close();
                        let _ = window.hide();
                    } else if let Some(companion) =
                        window.app_handle().get_webview_window("companion")
                    {
                        let _ = companion.destroy();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            companion::get_companion_trial,
            companion::set_companion_visible,
            companion::open_mochi,
            companion::hide_companion,
            companion::start_companion_drag,
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
        .build(context)?
        .run(move |app, event| {
            #[cfg(target_os = "macos")]
            if companion_preview && matches!(event, tauri::RunEvent::Reopen { .. }) {
                let _ = companion::show_main(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
    Ok(())
}
