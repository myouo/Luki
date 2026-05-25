export type WorkStatus =
  | 'unplayed'
  | 'playing'
  | 'completed'
  | 'paused'
  | 'dropped'
  | 'wishlist'
  | 'archived';

export interface LibraryItem {
  work_id: string;
  display_title: string;
  original_title: string | null;
  status: WorkStatus;
  user_rating: number | null;
  cover_path: string | null;
  cover_thumbnail_path: string | null;
  nsfw_level: number;
  privacy_level: number;
  total_playtime_seconds: number;
  last_played_at: number | null;
  completed_at: number | null;
  version_count: number;
  installation_count: number;
  available_installation_count: number;
  tag_summary: string | null;
  updated_at: number;
}

export interface LibraryPage {
  items: LibraryItem[];
  total: number;
  limit: number;
  offset: number;
}

export interface ContinueItem extends LibraryItem {
  default_launch_name: string | null;
  latest_save_label: string | null;
}

export interface MonthStats {
  duration_seconds: number;
  active_days: number;
  completed_count: number;
  most_played_title: string | null;
}

export interface SnapshotInfo {
  id: string;
  snapshot_type: string;
  reason: string | null;
  path: string;
  created_at: number;
}

export interface TodayDesk {
  continue_items: ContinueItem[];
  pending_import_count: number;
  unconfirmed_session_count: number;
  month: MonthStats;
  recent_items: LibraryItem[];
  last_snapshot: SnapshotInfo | null;
}

export interface ImportCandidate {
  id: string;
  path: string;
  candidate_type: string;
  detected_title: string | null;
  detected_executable: string | null;
  size_bytes: number | null;
  file_count: number | null;
  confidence: number;
  status: string;
  matched_work_id: string | null;
  evidence_json: string | null;
  created_at: number;
  updated_at: number;
}

export interface LibraryRoot {
  id: string;
  name: string | null;
  path: string;
  root_type: string;
  recursive: boolean;
  is_active: boolean;
  last_scanned_at: number | null;
  created_at: number;
  updated_at: number;
}

export interface ScanJob {
  id: string;
  root_id: string | null;
  root_path: string | null;
  status: string;
  started_at: number | null;
  finished_at: number | null;
  total_count: number;
  matched_count: number;
  failed_count: number;
  log_json: string | null;
  created_at: number;
  updated_at: number;
}

export interface BulkImportResult {
  imported_items: LibraryItem[];
  imported_count: number;
  snapshot: SnapshotInfo;
  operation_id: string;
}

export interface UndoResult {
  operation_id: string;
  restored_candidate_count: number;
  affected_work_count: number;
}

export interface MutationResult {
  operation_id: string;
  affected_count: number;
  message: string;
}

export interface PlaySession {
  id: string;
  work_id: string;
  title: string;
  started_at: number;
  ended_at: number | null;
  duration_seconds: number;
  source: string;
  confidence: number;
  is_confirmed: boolean;
  timer_status: string;
  note: string | null;
}

export interface LaunchReceipt {
  session_id: string;
  work_id: string;
  title: string;
  launch_profile_name: string;
  executable_path: string;
  process_id: number | null;
  started_at: number;
  status: string;
}

export interface LaunchProfile {
  id: string;
  work_id: string;
  installation_id: string | null;
  name: string;
  executable_path: string;
  working_dir: string | null;
  arguments: string | null;
  is_default: boolean;
  is_available: boolean;
  created_at: number;
  updated_at: number;
}

export interface SaveProfile {
  id: string;
  work_id: string;
  name: string;
  save_path: string;
  engine: string | null;
  strategy: string;
  is_active: boolean;
  created_at: number;
  updated_at: number;
}

export interface SaveSnapshot {
  id: string;
  work_id: string;
  save_profile_id: string | null;
  snapshot_path: string;
  note: string | null;
  route_name: string | null;
  progress_label: string | null;
  is_locked: boolean;
  created_at: number;
  updated_at: number;
}

export interface WorkDetail {
  item: LibraryItem;
  launch_profiles: LaunchProfile[];
  recent_sessions: PlaySession[];
  save_profiles: SaveProfile[];
  save_snapshots: SaveSnapshot[];
  notes: NoteItem[];
  timeline_events: TimelineEvent[];
  recent_operations: OperationItem[];
}

export interface NoteItem {
  id: string;
  title: string | null;
  content: string;
  note_type: string;
  spoiler_level: number;
  privacy_level: number;
  created_at: number;
  updated_at: number;
}

export interface TimelineEvent {
  id: string;
  work_id: string;
  event_type: string;
  source_id: string;
  occurred_at: number;
  title: string;
  summary: string | null;
  detail: string | null;
  duration_seconds: number | null;
  privacy_level: number;
  spoiler_level: number;
}

export interface OperationItem {
  id: string;
  entity_type: string;
  entity_id: string;
  operation_type: string;
  summary: string;
  payload_json: string;
  created_at: number;
  synced_at: number | null;
  is_synced: boolean;
}

export interface PathHealthIssue {
  work_id: string;
  title: string;
  installation_id: string;
  root_path: string;
  executable_path: string | null;
  was_available: boolean;
  is_available: boolean;
}

export interface PathHealthReport {
  checked_count: number;
  available_count: number;
  missing_count: number;
  changed_count: number;
  issues: PathHealthIssue[];
  operation_id: string;
  checked_at: number;
}
