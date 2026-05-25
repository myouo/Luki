use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LibraryItem {
    pub work_id: String,
    pub display_title: String,
    pub original_title: Option<String>,
    pub status: String,
    pub user_rating: Option<f64>,
    pub cover_path: Option<String>,
    pub cover_thumbnail_path: Option<String>,
    pub nsfw_level: i64,
    pub privacy_level: i64,
    pub total_playtime_seconds: i64,
    pub last_played_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub version_count: i64,
    pub installation_count: i64,
    pub available_installation_count: i64,
    pub tag_summary: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct ContinueItem {
    #[serde(flatten)]
    pub item: LibraryItem,
    pub default_launch_name: Option<String>,
    pub latest_save_label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MonthStats {
    pub duration_seconds: i64,
    pub active_days: i64,
    pub completed_count: i64,
    pub most_played_title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotInfo {
    pub id: String,
    pub snapshot_type: String,
    pub reason: Option<String>,
    pub path: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct TodayDesk {
    pub continue_items: Vec<ContinueItem>,
    pub pending_import_count: i64,
    pub unconfirmed_session_count: i64,
    pub month: MonthStats,
    pub recent_items: Vec<LibraryItem>,
    pub last_snapshot: Option<SnapshotInfo>,
}

#[derive(Debug, Serialize)]
pub struct ImportCandidate {
    pub id: String,
    pub path: String,
    pub candidate_type: String,
    pub detected_title: Option<String>,
    pub detected_executable: Option<String>,
    pub size_bytes: Option<i64>,
    pub file_count: Option<i64>,
    pub confidence: f64,
    pub status: String,
    pub matched_work_id: Option<String>,
    pub evidence_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct LibraryRoot {
    pub id: String,
    pub name: Option<String>,
    pub path: String,
    pub root_type: String,
    pub recursive: bool,
    pub is_active: bool,
    pub last_scanned_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct ScanJob {
    pub id: String,
    pub root_id: Option<String>,
    pub root_path: Option<String>,
    pub status: String,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub total_count: i64,
    pub matched_count: i64,
    pub failed_count: i64,
    pub log_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct BulkImportResult {
    pub imported_items: Vec<LibraryItem>,
    pub imported_count: usize,
    pub snapshot: SnapshotInfo,
    pub operation_id: String,
}

#[derive(Debug, Serialize)]
pub struct UndoResult {
    pub operation_id: String,
    pub restored_candidate_count: usize,
    pub affected_work_count: usize,
}

#[derive(Debug, Serialize)]
pub struct MutationResult {
    pub operation_id: String,
    pub affected_count: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PlaySession {
    pub id: String,
    pub work_id: String,
    pub title: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_seconds: i64,
    pub source: String,
    pub confidence: f64,
    pub is_confirmed: bool,
    pub timer_status: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LaunchReceipt {
    pub session_id: String,
    pub work_id: String,
    pub title: String,
    pub launch_profile_name: String,
    pub executable_path: String,
    pub process_id: Option<u32>,
    pub started_at: i64,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct LaunchProfile {
    pub id: String,
    pub work_id: String,
    pub installation_id: Option<String>,
    pub name: String,
    pub executable_path: String,
    pub working_dir: Option<String>,
    pub arguments: Option<String>,
    pub is_default: bool,
    pub is_available: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct SaveProfile {
    pub id: String,
    pub work_id: String,
    pub name: String,
    pub save_path: String,
    pub engine: Option<String>,
    pub strategy: String,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct SaveSnapshot {
    pub id: String,
    pub work_id: String,
    pub save_profile_id: Option<String>,
    pub snapshot_path: String,
    pub note: Option<String>,
    pub route_name: Option<String>,
    pub progress_label: Option<String>,
    pub is_locked: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct NoteItem {
    pub id: String,
    pub title: Option<String>,
    pub content: String,
    pub note_type: String,
    pub spoiler_level: i64,
    pub privacy_level: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct TimelineEvent {
    pub id: String,
    pub work_id: String,
    pub event_type: String,
    pub source_id: String,
    pub occurred_at: i64,
    pub title: String,
    pub summary: Option<String>,
    pub detail: Option<String>,
    pub duration_seconds: Option<i64>,
    pub privacy_level: i64,
    pub spoiler_level: i64,
}

#[derive(Debug, Serialize)]
pub struct OperationItem {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation_type: String,
    pub summary: String,
    pub payload_json: String,
    pub created_at: i64,
    pub synced_at: Option<i64>,
    pub is_synced: bool,
}

#[derive(Debug, Serialize)]
pub struct PathHealthIssue {
    pub work_id: String,
    pub title: String,
    pub installation_id: String,
    pub root_path: String,
    pub executable_path: Option<String>,
    pub was_available: bool,
    pub is_available: bool,
}

#[derive(Debug, Serialize)]
pub struct PathHealthReport {
    pub checked_count: i64,
    pub available_count: i64,
    pub missing_count: i64,
    pub changed_count: i64,
    pub issues: Vec<PathHealthIssue>,
    pub operation_id: String,
    pub checked_at: i64,
}

#[derive(Debug, Serialize)]
pub struct WorkDetail {
    pub item: LibraryItem,
    pub launch_profiles: Vec<LaunchProfile>,
    pub recent_sessions: Vec<PlaySession>,
    pub save_profiles: Vec<SaveProfile>,
    pub save_snapshots: Vec<SaveSnapshot>,
    pub notes: Vec<NoteItem>,
    pub timeline_events: Vec<TimelineEvent>,
    pub recent_operations: Vec<OperationItem>,
}
