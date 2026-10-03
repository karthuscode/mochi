use crate::app_info::{app_info, AppInfo};

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    app_info()
}

use crate::local_capture::{
    ListCursor, LocalCapture, LocalStatus, ProjectView, SessionDetail, SessionList,
};
use mochi_domain::{ProjectId, SessionId};
use mochi_integration::InstallPreview;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;
async fn call<T: Send + 'static>(
    core: Arc<LocalCapture>,
    f: impl FnOnce(&LocalCapture) -> Result<T, &'static str> + Send + 'static,
) -> Result<T, &'static str> {
    tauri::async_runtime::spawn_blocking(move || f(&core))
        .await
        .map_err(|_| "Native operation unavailable.")?
}
#[tauri::command]
pub async fn local_status(core: State<'_, Arc<LocalCapture>>) -> Result<LocalStatus, &'static str> {
    call(core.inner().clone(), LocalCapture::status).await
}
#[tauri::command]
pub async fn list_projects(
    core: State<'_, Arc<LocalCapture>>,
    after: Option<ProjectId>,
) -> Result<Vec<ProjectView>, &'static str> {
    call(core.inner().clone(), move |c| c.projects(after)).await
}
#[tauri::command]
pub async fn approve_project(
    core: State<'_, Arc<LocalCapture>>,
    path: String,
    name: String,
) -> Result<ProjectView, &'static str> {
    call(core.inner().clone(), move |c| c.approve_project(path, name)).await
}
#[tauri::command]
pub async fn preview_connection(
    core: State<'_, Arc<LocalCapture>>,
    project_id: ProjectId,
    disconnect: bool,
) -> Result<InstallPreview, &'static str> {
    call(core.inner().clone(), move |c| {
        c.preview_connection(project_id, disconnect)
    })
    .await
}
#[tauri::command]
pub async fn apply_connection(
    core: State<'_, Arc<LocalCapture>>,
    plan_id: Uuid,
    enable_local_capture: bool,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| {
        c.apply_connection(plan_id, enable_local_capture)
    })
    .await
}
#[tauri::command]
pub async fn set_tracking(
    core: State<'_, Arc<LocalCapture>>,
    project_id: ProjectId,
    enabled: bool,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| {
        c.tracking(project_id, enabled)
    })
    .await
}
#[tauri::command]
pub async fn pause_all_capture(core: State<'_, Arc<LocalCapture>>) -> Result<(), &'static str> {
    call(core.inner().clone(), LocalCapture::pause_all).await
}
#[tauri::command]
pub async fn list_sessions(
    core: State<'_, Arc<LocalCapture>>,
    project_id: ProjectId,
    before: Option<ListCursor>,
) -> Result<SessionList, &'static str> {
    call(core.inner().clone(), move |c| {
        c.sessions(project_id, before)
    })
    .await
}
#[tauri::command]
pub async fn session_detail(
    core: State<'_, Arc<LocalCapture>>,
    session_id: SessionId,
    after_sequence: Option<u64>,
) -> Result<SessionDetail, &'static str> {
    call(core.inner().clone(), move |c| {
        c.detail(session_id, after_sequence)
    })
    .await
}
#[tauri::command]
pub async fn finish_session(
    core: State<'_, Arc<LocalCapture>>,
    session_id: SessionId,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| c.finish(session_id)).await
}
#[tauri::command]
pub async fn delete_session(
    core: State<'_, Arc<LocalCapture>>,
    session_id: SessionId,
    confirmed: bool,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| {
        c.delete_session(session_id, confirmed)
    })
    .await
}
#[tauri::command]
pub async fn delete_project(
    core: State<'_, Arc<LocalCapture>>,
    project_id: ProjectId,
    confirmed: bool,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| {
        c.delete_project(project_id, confirmed)
    })
    .await
}

use crate::analysis::{AnalysisService, AnalysisStatus, LessonView, SendPreview};
use mochi_persistence::LearningAttempt;
#[tauri::command]
pub async fn analysis_status(
    core: State<'_, Arc<LocalCapture>>,
) -> Result<AnalysisStatus, &'static str> {
    call(core.inner().clone(), |c| c.analysis.status()).await
}
#[tauri::command]
pub async fn analysis_permission(
    core: State<'_, Arc<LocalCapture>>,
    enabled: bool,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| {
        c.analysis.permission(enabled)
    })
    .await
}
#[tauri::command]
pub async fn store_api_key(
    core: State<'_, Arc<LocalCapture>>,
    key: String,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| c.analysis.set_key(key)).await
}
#[tauri::command]
pub async fn delete_api_key(core: State<'_, Arc<LocalCapture>>) -> Result<(), &'static str> {
    call(core.inner().clone(), |c| c.analysis.delete_key()).await
}
#[tauri::command]
pub async fn preview_analysis(
    core: State<'_, Arc<LocalCapture>>,
    session_id: SessionId,
    attempt_id: Option<Uuid>,
) -> Result<SendPreview, &'static str> {
    call(core.inner().clone(), move |c| {
        c.analysis.prepare(c, session_id, attempt_id)
    })
    .await
}
#[tauri::command]
pub async fn approve_analysis(
    core: State<'_, Arc<LocalCapture>>,
    token: Uuid,
    confirmed: bool,
) -> Result<(), &'static str> {
    let c = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || AnalysisService::approve(c, token, confirmed))
        .await
        .map_err(|_| "Analysis unavailable.")?
}
#[tauri::command]
pub async fn cancel_analysis(core: State<'_, Arc<LocalCapture>>) -> Result<(), &'static str> {
    call(core.inner().clone(), |c| c.analysis.cancel()).await
}
#[tauri::command]
pub async fn learning_document(
    core: State<'_, Arc<LocalCapture>>,
    session_id: SessionId,
) -> Result<Option<LessonView>, &'static str> {
    call(core.inner().clone(), move |c| {
        c.analysis.lesson(c, session_id)
    })
    .await
}
#[tauri::command]
pub async fn submit_selfcheck(
    core: State<'_, Arc<LocalCapture>>,
    attempt_id: Uuid,
    question_id: Uuid,
    answer: String,
    assistance: String,
) -> Result<LearningAttempt, &'static str> {
    call(core.inner().clone(), move |c| {
        c.store
            .submit_selfcheck(attempt_id, question_id, &answer, &assistance)
            .map_err(|_| "Answer could not be saved safely. Check the question and retry.")
    })
    .await
}
#[tauri::command]
pub async fn reveal_selfcheck(
    core: State<'_, Arc<LocalCapture>>,
    question_id: Uuid,
) -> Result<String, &'static str> {
    call(core.inner().clone(), move |c| {
        c.store
            .reveal_selfcheck(question_id)
            .map_err(|_| "Answer explanation unavailable.")
    })
    .await
}

use crate::home_preferences::HomePreferences;
use mochi_persistence::ProjectRepository;
#[tauri::command]
pub async fn get_home_preferences(
    core: State<'_, Arc<LocalCapture>>,
) -> Result<HomePreferences, &'static str> {
    call(core.inner().clone(), |c| {
        c.home_preferences.read().map_err(|e| e.message())
    })
    .await
}
#[tauri::command]
pub async fn save_home_preferences(
    core: State<'_, Arc<LocalCapture>>,
    preferences: HomePreferences,
) -> Result<(), &'static str> {
    call(core.inner().clone(), move |c| {
        let _guard = c
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        if let Some(id) = preferences.project_id {
            if c.store
                .get_project(id)
                .map_err(|_| "Projects unavailable.")?
                .is_none_or(|p| p.deleted_at.is_some())
            {
                return Err("Approved project unavailable.");
            }
        }
        c.home_preferences
            .save(&preferences)
            .map_err(|e| e.message())
    })
    .await
}
