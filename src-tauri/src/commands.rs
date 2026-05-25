use crate::{
    db::Database,
    models::{
        BulkImportResult, ImportCandidate, LaunchReceipt, LibraryItem, LibraryPage, LibraryRoot,
        MutationResult, OperationItem, PathHealthReport, PlaySession, SaveProfile, SaveSnapshot,
        ScanJob, SnapshotInfo, TodayDesk, UndoResult, WorkDetail,
    },
};
use std::{path::PathBuf, process::Command, sync::Mutex, thread};
use tauri::State;

pub struct AppState {
    pub db: Mutex<Database>,
}

#[tauri::command]
pub fn get_today_desk(state: State<'_, AppState>) -> Result<TodayDesk, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.get_today_desk().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_library_items(
    state: State<'_, AppState>,
    search: String,
    status: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<LibraryItem>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_library_items(&search, &status, limit, offset)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_library_page(
    state: State<'_, AppState>,
    search: String,
    status: String,
    limit: i64,
    offset: i64,
) -> Result<LibraryPage, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_library_page(&search, &status, limit, offset)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_work_detail(state: State<'_, AppState>, work_id: String) -> Result<WorkDetail, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.get_work_detail(&work_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_operations(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<OperationItem>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_operations(limit).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn check_path_health(state: State<'_, AppState>) -> Result<PathHealthReport, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.check_path_health().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_import_candidates(state: State<'_, AppState>) -> Result<Vec<ImportCandidate>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_import_candidates()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_library_roots(state: State<'_, AppState>) -> Result<Vec<LibraryRoot>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_library_roots().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_scan_jobs(state: State<'_, AppState>, limit: i64) -> Result<Vec<ScanJob>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_scan_jobs(limit).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn scan_library_root(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<ImportCandidate>, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.scan_library_root(&path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn accept_import_candidate(
    state: State<'_, AppState>,
    candidate_id: String,
) -> Result<LibraryItem, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.accept_import_candidate(&candidate_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn accept_pending_import_candidates(
    state: State<'_, AppState>,
) -> Result<BulkImportResult, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.accept_pending_import_candidates()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn undo_latest_bulk_import(state: State<'_, AppState>) -> Result<UndoResult, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.undo_latest_bulk_import()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn soft_delete_work(
    state: State<'_, AppState>,
    work_id: String,
) -> Result<MutationResult, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.soft_delete_work(&work_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn record_manual_session(
    state: State<'_, AppState>,
    work_id: String,
    duration_seconds: i64,
    note: String,
) -> Result<PlaySession, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.record_manual_session(&work_id, duration_seconds, &note)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    work_id: String,
    title: String,
    content: String,
    note_type: String,
    spoiler_level: i64,
    privacy_level: i64,
) -> Result<crate::models::NoteItem, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.create_note(
        &work_id,
        &title,
        &content,
        &note_type,
        spoiler_level,
        privacy_level,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_unconfirmed_sessions(state: State<'_, AppState>) -> Result<Vec<PlaySession>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.list_unconfirmed_sessions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn confirm_play_session(
    state: State<'_, AppState>,
    session_id: String,
    duration_seconds: i64,
    note: String,
) -> Result<PlaySession, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.confirm_play_session(&session_id, duration_seconds, &note)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn discard_play_session(
    state: State<'_, AppState>,
    session_id: String,
    reason: String,
) -> Result<MutationResult, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.discard_play_session(&session_id, &reason)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_snapshot(state: State<'_, AppState>, reason: String) -> Result<SnapshotInfo, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.create_snapshot(&reason)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_work_status(
    state: State<'_, AppState>,
    work_id: String,
    status: String,
) -> Result<LibraryItem, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.update_work_status(&work_id, &status)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_work_metadata(
    state: State<'_, AppState>,
    work_id: String,
    title: String,
    original_title: String,
    user_rating: Option<f64>,
    tag_summary: String,
    cover_path: String,
    nsfw_level: i64,
    privacy_level: i64,
) -> Result<LibraryItem, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.update_work_metadata(
        &work_id,
        &title,
        &original_title,
        user_rating,
        &tag_summary,
        &cover_path,
        nsfw_level,
        privacy_level,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn upsert_default_launch_profile(
    state: State<'_, AppState>,
    work_id: String,
    name: String,
    executable_path: String,
    working_dir: String,
    arguments: String,
) -> Result<crate::models::LaunchProfile, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.upsert_default_launch_profile(&work_id, &name, &executable_path, &working_dir, &arguments)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn launch_work(state: State<'_, AppState>, work_id: String) -> Result<LaunchReceipt, String> {
    let (target, app_dir) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        (
            db.prepare_launch(&work_id)
                .map_err(|error| error.to_string())?,
            db.app_dir(),
        )
    };

    let executable = PathBuf::from(&target.executable_path);
    if !executable.exists() {
        return Err(format!("启动文件不存在：{}", target.executable_path));
    }

    let mut command = Command::new(&target.executable_path);
    if let Some(working_dir) = &target.working_dir {
        if !working_dir.trim().is_empty() {
            command.current_dir(working_dir);
        }
    }
    if let Some(arguments) = &target.arguments {
        command.args(arguments.split_whitespace());
    }

    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let process_id = child.id();
    let receipt = {
        let mut db = state.db.lock().map_err(|error| error.to_string())?;
        db.start_launch_session(&target, Some(process_id))
            .map_err(|error| error.to_string())?
    };

    let session_id = receipt.session_id.clone();
    thread::spawn(move || {
        let exit_code = child.wait().ok().and_then(|status| status.code());
        if let Ok(mut db) = Database::open(app_dir) {
            let _ = db.finish_launch_session(&session_id, exit_code);
        }
    });

    Ok(receipt)
}

#[tauri::command]
pub fn create_save_profile(
    state: State<'_, AppState>,
    work_id: String,
    name: String,
    save_path: String,
) -> Result<SaveProfile, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.create_save_profile(&work_id, &name, &save_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_save_snapshot(
    state: State<'_, AppState>,
    work_id: String,
    save_profile_id: Option<String>,
    source_path: String,
    note: String,
    route_name: String,
    progress_label: String,
) -> Result<SaveSnapshot, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.create_save_snapshot(
        &work_id,
        save_profile_id.as_deref(),
        &source_path,
        &note,
        &route_name,
        &progress_label,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn restore_save_snapshot(
    state: State<'_, AppState>,
    snapshot_id: String,
    target_path: String,
) -> Result<SaveSnapshot, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.restore_save_snapshot(&snapshot_id, &target_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_save_snapshot_locked(
    state: State<'_, AppState>,
    snapshot_id: String,
    is_locked: bool,
) -> Result<SaveSnapshot, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.set_save_snapshot_locked(&snapshot_id, is_locked)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_save_snapshot(
    state: State<'_, AppState>,
    snapshot_id: String,
) -> Result<MutationResult, String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    db.delete_save_snapshot(&snapshot_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn pick_folder() -> Result<Option<String>, String> {
    Ok(rfd::FileDialog::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn pick_file() -> Result<Option<String>, String> {
    Ok(rfd::FileDialog::new()
        .pick_file()
        .map(|path| path.to_string_lossy().to_string()))
}
