mod commands;
mod db;
mod models;

use commands::{
    accept_import_candidate, accept_pending_import_candidates, check_path_health,
    confirm_play_session, create_note, create_save_profile, create_save_snapshot, create_snapshot,
    delete_save_snapshot, discard_play_session, get_today_desk, get_work_detail, launch_work,
    list_import_candidates, list_library_items, list_library_page, list_library_roots,
    list_operations, list_scan_jobs, list_unconfirmed_sessions, pick_file, pick_folder,
    record_manual_session, restore_save_snapshot, scan_library_root, set_save_snapshot_locked,
    soft_delete_work, undo_latest_bulk_import, update_work_metadata, update_work_status,
    upsert_default_launch_profile, AppState,
};
use db::Database;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            let db = Database::open(app_dir)?;
            app.manage(AppState { db: Mutex::new(db) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_today_desk,
            list_library_items,
            list_library_page,
            get_work_detail,
            list_operations,
            check_path_health,
            list_import_candidates,
            list_library_roots,
            list_scan_jobs,
            scan_library_root,
            accept_import_candidate,
            accept_pending_import_candidates,
            undo_latest_bulk_import,
            soft_delete_work,
            record_manual_session,
            create_note,
            list_unconfirmed_sessions,
            confirm_play_session,
            discard_play_session,
            create_snapshot,
            update_work_status,
            update_work_metadata,
            upsert_default_launch_profile,
            launch_work,
            create_save_profile,
            create_save_snapshot,
            restore_save_snapshot,
            set_save_snapshot_locked,
            delete_save_snapshot,
            pick_folder,
            pick_file
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Luki");
}
