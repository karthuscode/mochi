use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

const SIZE: f64 = 160.0;
const MARGIN: f64 = 24.0;

#[derive(Default)]
pub struct CompanionTrial {
    positioned: AtomicBool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialView {
    schema_version: u8,
    visible: bool,
}

pub fn preview_requested() -> bool {
    std::env::var_os("MOCHI_COMPANION_PREVIEW").as_deref() == Some(std::ffi::OsStr::new("1"))
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(CompanionTrial::default());
    let window = companion(app.handle())?;
    #[cfg(target_os = "macos")]
    {
        let _main_thread = objc2::MainThreadMarker::new()
            .ok_or("Companion must be configured on the native main thread.")?;
        // SAFETY: setup is on the AppKit main thread, Tauri owns this NSWindow,
        // and this borrow does not escape the lifetime of the window handle.
        let native = unsafe { &*window.ns_window()?.cast::<objc2_app_kit::NSWindow>() };
        use objc2_app_kit::NSWindowCollectionBehavior as Behavior;
        native.setCollectionBehavior(Behavior::CanJoinAllSpaces | Behavior::FullScreenNone);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    Ok(())
}

fn companion(app: &AppHandle) -> Result<WebviewWindow, &'static str> {
    app.get_webview_window("companion")
        .ok_or("Desktop mochi is unavailable. Restart mochi and try again.")
}

fn require_label(window: &WebviewWindow, expected: &str) -> Result<(), &'static str> {
    if window.label() != expected {
        return Err("This window cannot perform that companion action.");
    }
    Ok(())
}

fn first_position(x: i32, y: i32, width: u32, height: u32, scale: f64) -> PhysicalPosition<i32> {
    let inset = (MARGIN * scale).round() as i64;
    let size = (SIZE * scale).round() as i64;
    let horizontal = (i64::from(width) - size - inset).max(0);
    let vertical = (i64::from(height) - size - inset).max(0);
    PhysicalPosition::new(
        (i64::from(x) + horizontal).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        (i64::from(y) + vertical).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

pub fn show(app: &AppHandle) -> Result<(), &'static str> {
    let window = companion(app)?;
    let trial = app.state::<CompanionTrial>();
    if !trial.positioned.load(Ordering::Relaxed) {
        let main = app
            .get_webview_window("main")
            .ok_or("mochi window is unavailable.")?;
        let monitor = main
            .current_monitor()
            .map_err(|_| "Display is unavailable.")?
            .or(main
                .primary_monitor()
                .map_err(|_| "Display is unavailable.")?)
            .ok_or("Display is unavailable.")?;
        let area = monitor.work_area();
        window
            .set_position(first_position(
                area.position.x,
                area.position.y,
                area.size.width,
                area.size.height,
                monitor.scale_factor(),
            ))
            .map_err(|_| "Desktop mochi could not be placed on the display.")?;
        trial.positioned.store(true, Ordering::Relaxed);
    }
    window
        .show()
        .map_err(|_| "Desktop mochi could not be shown.")
}

#[tauri::command]
pub fn get_companion_trial(
    window: WebviewWindow,
    app: AppHandle,
) -> Result<TrialView, &'static str> {
    require_label(&window, "main")?;
    Ok(TrialView {
        schema_version: 1,
        visible: companion(&app)?
            .is_visible()
            .map_err(|_| "Desktop mochi state is unavailable.")?,
    })
}

#[tauri::command]
pub fn set_companion_visible(
    window: WebviewWindow,
    app: AppHandle,
    visible: bool,
) -> Result<TrialView, &'static str> {
    require_label(&window, "main")?;
    if visible {
        show(&app)?;
    } else {
        companion(&app)?
            .hide()
            .map_err(|_| "Desktop mochi could not be hidden.")?;
    }
    get_companion_trial(window, app)
}

#[tauri::command]
pub fn open_mochi(window: WebviewWindow, app: AppHandle) -> Result<(), &'static str> {
    require_label(&window, "companion")?;
    show_main(&app)
}

pub fn show_main(app: &AppHandle) -> Result<(), &'static str> {
    let main = app
        .get_webview_window("main")
        .ok_or("mochi window is unavailable.")?;
    main.show()
        .and_then(|()| main.unminimize())
        .and_then(|()| main.set_focus())
        .map_err(|_| "mochi could not be opened. Use its Dock icon to reopen it.")
}

#[tauri::command]
pub fn hide_companion(window: WebviewWindow) -> Result<(), &'static str> {
    require_label(&window, "companion")?;
    window
        .hide()
        .map_err(|_| "Desktop mochi could not be hidden.")
}

#[tauri::command]
pub fn start_companion_drag(window: WebviewWindow) -> Result<(), &'static str> {
    require_label(&window, "companion")?;
    window
        .start_dragging()
        .map_err(|_| "Desktop mochi could not be moved.")
}

#[cfg(test)]
mod tests {
    use super::first_position;
    use tauri::PhysicalPosition;

    #[test]
    fn placement_respects_retina_work_area_and_negative_monitor_origin() {
        assert_eq!(
            first_position(0, 50, 2880, 1750, 2.0),
            PhysicalPosition::new(2512, 1432)
        );
        assert_eq!(
            first_position(-1920, 25, 1920, 1055, 1.0),
            PhysicalPosition::new(-184, 896)
        );
    }

    #[test]
    fn tiny_work_area_does_not_place_the_window_before_its_origin() {
        assert_eq!(
            first_position(-300, 50, 100, 90, 1.0),
            PhysicalPosition::new(-300, 50)
        );
    }
}
