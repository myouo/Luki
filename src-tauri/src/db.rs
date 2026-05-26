use crate::models::{
    BulkImportResult, ContinueItem, DataLocations, ImportCandidate, LaunchProfile, LaunchReceipt,
    LibraryItem, LibraryPage, LibraryRoot, MonthStats, MutationResult, NoteItem, OperationItem,
    PathHealthIssue, PathHealthReport, PlaySession, SaveProfile, SaveSnapshot, ScanJob,
    SnapshotInfo, TimelineEvent, TodayDesk, UndoResult, WorkDetail,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const THIRTY_DAYS_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const CURRENT_SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug)]
pub struct LaunchTarget {
    pub work_id: String,
    pub title: String,
    pub profile_name: String,
    pub executable_path: String,
    pub working_dir: Option<String>,
    pub arguments: Option<String>,
}

pub struct Database {
    conn: Connection,
    app_dir: PathBuf,
}

struct SnapshotFile {
    id: String,
    snapshot_type: String,
    path: String,
    size_bytes: Option<i64>,
    created_at: i64,
}

#[derive(Clone, Debug)]
struct CoverAsset {
    id: String,
    original_path: String,
    thumbnail_256_path: String,
    thumbnail_512_path: String,
    mime_type: Option<String>,
    size_bytes: Option<i64>,
    hash: Option<String>,
}

impl SnapshotFile {
    fn to_info(&self, reason: &str) -> SnapshotInfo {
        SnapshotInfo {
            id: self.id.clone(),
            snapshot_type: self.snapshot_type.clone(),
            reason: empty_to_null(reason).map(str::to_string),
            path: self.path.clone(),
            created_at: self.created_at,
        }
    }
}

impl Database {
    pub fn open(app_dir: PathBuf) -> rusqlite::Result<Self> {
        fs::create_dir_all(&app_dir).map_err(io_to_sql)?;
        let db_path = app_dir.join("library.db");
        let db_existed = db_path.exists();
        let conn = Connection::open(&db_path)?;
        let mut db = Self { conn, app_dir };
        db.configure()?;
        let schema_version = db.schema_version()?;
        let migration_snapshot = if db_existed && schema_version < CURRENT_SCHEMA_VERSION {
            Some(db.create_snapshot_file("before_migration")?)
        } else {
            None
        };
        db.migrate()?;
        if schema_version < CURRENT_SCHEMA_VERSION {
            db.set_schema_version(CURRENT_SCHEMA_VERSION)?;
        }
        if let Some(snapshot) = migration_snapshot {
            db.insert_snapshot_record(&snapshot, "迁移前快照")?;
        }
        db.ensure_daily_snapshot()?;
        Ok(db)
    }

    fn configure(&self) -> rusqlite::Result<()> {
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        self.conn.pragma_update(None, "temp_store", "MEMORY")?;
        self.conn.pragma_update(None, "cache_size", -65536)?;
        self.conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(())
    }

    fn schema_version(&self) -> rusqlite::Result<i64> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
    }

    fn set_schema_version(&self, version: i64) -> rusqlite::Result<()> {
        self.conn.pragma_update(None, "user_version", version)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS works (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                original_title TEXT,
                sort_title TEXT,
                display_title TEXT,
                description TEXT,
                release_year INTEGER,
                developer TEXT,
                publisher TEXT,
                status TEXT NOT NULL DEFAULT 'unplayed',
                user_rating REAL,
                user_priority INTEGER NOT NULL DEFAULT 0,
                nsfw_level INTEGER NOT NULL DEFAULT 0,
                privacy_level INTEGER NOT NULL DEFAULT 0,
                total_playtime_seconds INTEGER NOT NULL DEFAULT 0,
                last_played_at INTEGER,
                first_played_at INTEGER,
                completed_at INTEGER,
                cover_asset_id TEXT,
                cover_path TEXT,
                cover_thumbnail_path TEXT,
                metadata_json TEXT,
                user_data_json TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER,
                revision INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS library_items (
                work_id TEXT PRIMARY KEY,
                display_title TEXT NOT NULL,
                original_title TEXT,
                sort_title TEXT,
                status TEXT NOT NULL,
                user_rating REAL,
                user_priority INTEGER NOT NULL DEFAULT 0,
                cover_asset_id TEXT,
                cover_path TEXT,
                cover_thumbnail_path TEXT,
                developer TEXT,
                release_year INTEGER,
                total_playtime_seconds INTEGER NOT NULL DEFAULT 0,
                last_played_at INTEGER,
                completed_at INTEGER,
                version_count INTEGER NOT NULL DEFAULT 0,
                installation_count INTEGER NOT NULL DEFAULT 0,
                available_installation_count INTEGER NOT NULL DEFAULT 0,
                tag_summary TEXT,
                staff_summary TEXT,
                nsfw_level INTEGER NOT NULL DEFAULT 0,
                privacy_level INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS installations (
                id TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                root_path TEXT NOT NULL,
                executable_path TEXT,
                storage_type TEXT NOT NULL DEFAULT 'local',
                is_available INTEGER NOT NULL DEFAULT 1,
                is_primary INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS launch_profiles (
                id TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                installation_id TEXT,
                name TEXT NOT NULL,
                executable_path TEXT NOT NULL,
                working_dir TEXT,
                arguments TEXT,
                locale TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS library_roots (
                id TEXT PRIMARY KEY,
                name TEXT,
                path TEXT NOT NULL UNIQUE,
                root_type TEXT NOT NULL DEFAULT 'games',
                recursive INTEGER NOT NULL DEFAULT 1,
                scan_rules_json TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                last_scanned_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER,
                revision INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS scan_jobs (
                id TEXT PRIMARY KEY,
                root_id TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                started_at INTEGER,
                finished_at INTEGER,
                total_count INTEGER DEFAULT 0,
                matched_count INTEGER DEFAULT 0,
                failed_count INTEGER DEFAULT 0,
                log_json TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS import_candidates (
                id TEXT PRIMARY KEY,
                scan_job_id TEXT,
                root_id TEXT,
                path TEXT NOT NULL UNIQUE,
                candidate_type TEXT NOT NULL DEFAULT 'folder',
                detected_title TEXT,
                detected_executable TEXT,
                size_bytes INTEGER,
                file_count INTEGER,
                confidence REAL NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                matched_work_id TEXT,
                matched_version_id TEXT,
                evidence_json TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS play_sessions (
                id TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL DEFAULT 'auto',
                confidence REAL NOT NULL DEFAULT 1.0,
                is_confirmed INTEGER NOT NULL DEFAULT 0,
                is_manual INTEGER NOT NULL DEFAULT 0,
                timer_status TEXT NOT NULL DEFAULT 'completed',
                evidence_json TEXT,
                correction_reason TEXT,
                note TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS playtime_daily (
                id TEXT PRIMARY KEY,
                date TEXT NOT NULL,
                work_id TEXT NOT NULL,
                duration_seconds INTEGER NOT NULL DEFAULT 0,
                session_count INTEGER NOT NULL DEFAULT 0,
                first_started_at INTEGER,
                last_ended_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(date, work_id)
            );

            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                work_id TEXT,
                title TEXT,
                content TEXT NOT NULL,
                note_type TEXT NOT NULL DEFAULT 'note',
                spoiler_level INTEGER NOT NULL DEFAULT 0,
                privacy_level INTEGER NOT NULL DEFAULT 0,
                route_name TEXT,
                progress_label TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS save_profiles (
                id TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                name TEXT NOT NULL,
                save_path TEXT NOT NULL,
                engine TEXT,
                strategy TEXT NOT NULL DEFAULT 'copy',
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS save_snapshots (
                id TEXT PRIMARY KEY,
                work_id TEXT NOT NULL,
                save_profile_id TEXT,
                snapshot_path TEXT NOT NULL,
                note TEXT,
                route_name TEXT,
                progress_label TEXT,
                is_locked INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS assets (
                id TEXT PRIMARY KEY,
                asset_type TEXT NOT NULL,
                local_path TEXT NOT NULL,
                thumbnail_256_path TEXT,
                thumbnail_512_path TEXT,
                original_url TEXT,
                source TEXT,
                mime_type TEXT,
                width INTEGER,
                height INTEGER,
                size_bytes INTEGER,
                hash TEXT,
                blurhash TEXT,
                dominant_color TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER,
                revision INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS asset_links (
                id TEXT PRIMARY KEY,
                asset_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                role TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                UNIQUE(asset_id, entity_type, entity_id, role)
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                id TEXT PRIMARY KEY,
                snapshot_type TEXT NOT NULL,
                path TEXT NOT NULL,
                reason TEXT,
                size_bytes INTEGER,
                checksum TEXT,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS operations (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                operation_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                inverse_payload_json TEXT,
                created_at INTEGER NOT NULL,
                synced_at INTEGER,
                checksum TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS work_search_fts USING fts5(
                work_id UNINDEXED,
                title,
                original_title,
                aliases,
                developer,
                publisher,
                tags,
                description,
                tokenize = 'unicode61'
            );

            CREATE INDEX IF NOT EXISTS idx_library_items_status ON library_items(status);
            CREATE INDEX IF NOT EXISTS idx_library_items_last_played ON library_items(last_played_at DESC);
            CREATE INDEX IF NOT EXISTS idx_library_roots_path ON library_roots(path);
            CREATE INDEX IF NOT EXISTS idx_library_roots_active ON library_roots(is_active);
            CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status);
            CREATE INDEX IF NOT EXISTS idx_scan_jobs_created ON scan_jobs(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_candidates_status ON import_candidates(status);
            CREATE INDEX IF NOT EXISTS idx_import_candidates_scan ON import_candidates(scan_job_id);
            CREATE INDEX IF NOT EXISTS idx_play_sessions_work_started ON play_sessions(work_id, started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_playtime_daily_date ON playtime_daily(date);
            CREATE INDEX IF NOT EXISTS idx_save_profiles_work ON save_profiles(work_id);
            CREATE INDEX IF NOT EXISTS idx_save_snapshots_work ON save_snapshots(work_id);
            CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
            CREATE INDEX IF NOT EXISTS idx_assets_hash ON assets(hash);
            CREATE INDEX IF NOT EXISTS idx_assets_source ON assets(source);
            CREATE INDEX IF NOT EXISTS idx_asset_links_entity ON asset_links(entity_type, entity_id);
            CREATE INDEX IF NOT EXISTS idx_asset_links_asset ON asset_links(asset_id);
            CREATE INDEX IF NOT EXISTS idx_snapshots_created ON snapshots(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_operations_created ON operations(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_operations_entity ON operations(entity_type, entity_id, created_at DESC);
            "#,
        )?;
        let _ = self.conn.execute(
            "ALTER TABLE save_snapshots ADD COLUMN save_profile_id TEXT",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE works ADD COLUMN cover_asset_id TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE works ADD COLUMN cover_thumbnail_path TEXT", []);
        let _ = self.conn.execute(
            "ALTER TABLE works ADD COLUMN nsfw_level INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE works ADD COLUMN privacy_level INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE library_items ADD COLUMN cover_asset_id TEXT",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE library_items ADD COLUMN cover_thumbnail_path TEXT",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE library_items ADD COLUMN nsfw_level INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE library_items ADD COLUMN privacy_level INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self
            .conn
            .execute("ALTER TABLE assets ADD COLUMN thumbnail_256_path TEXT", []);
        let _ = self
            .conn
            .execute("ALTER TABLE assets ADD COLUMN thumbnail_512_path TEXT", []);
        Ok(())
    }

    pub fn get_today_desk(&self) -> rusqlite::Result<TodayDesk> {
        let continue_items = self.query_continue_items(3)?;
        let pending_import_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM import_candidates WHERE status IN ('pending', 'needs_review')",
            [],
            |row| row.get(0),
        )?;
        let unconfirmed_session_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM play_sessions WHERE is_confirmed = 0 AND timer_status = 'needs_review' AND deleted_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        let recent_items = self.query_library_items("", "", 8, 0)?;
        let last_snapshot = self.query_last_snapshot()?;
        let month = self.query_month_stats()?;
        Ok(TodayDesk {
            continue_items,
            pending_import_count,
            unconfirmed_session_count,
            month,
            recent_items,
            last_snapshot,
        })
    }

    pub fn data_locations(&self) -> DataLocations {
        DataLocations {
            app_dir: self.app_dir.to_string_lossy().to_string(),
            database_path: self
                .app_dir
                .join("library.db")
                .to_string_lossy()
                .to_string(),
            assets_dir: self.app_dir.join("assets").to_string_lossy().to_string(),
            database_snapshots_dir: self
                .app_dir
                .join("backups")
                .join("snapshots")
                .to_string_lossy()
                .to_string(),
            save_snapshots_dir: self
                .app_dir
                .join("saves")
                .join("snapshots")
                .to_string_lossy()
                .to_string(),
            restore_backups_dir: self
                .app_dir
                .join("saves")
                .join("restore-backups")
                .to_string_lossy()
                .to_string(),
        }
    }

    pub fn list_library_items(
        &self,
        search: &str,
        status: &str,
        limit: i64,
        offset: i64,
    ) -> rusqlite::Result<Vec<LibraryItem>> {
        self.query_library_items(search, status, limit.clamp(1, 501), offset.max(0))
    }

    pub fn list_library_page(
        &self,
        search: &str,
        status: &str,
        limit: i64,
        offset: i64,
    ) -> rusqlite::Result<LibraryPage> {
        let limit = limit.clamp(1, 501);
        let offset = offset.max(0);
        let items = self.query_library_items(search, status, limit, offset)?;
        let total = self.count_library_items(search, status)?;
        Ok(LibraryPage {
            items,
            total,
            limit,
            offset,
        })
    }

    pub fn get_work_detail(&self, work_id: &str) -> rusqlite::Result<WorkDetail> {
        let item = self.query_library_item(work_id)?;
        let launch_profiles = self.query_launch_profiles(work_id)?;
        let recent_sessions = self.query_recent_sessions(work_id, 20)?;
        let save_profiles = self.query_save_profiles(work_id)?;
        let save_snapshots = self.query_save_snapshots(work_id, 30)?;
        let notes = self.query_notes(work_id, 50)?;
        let timeline_events = self.query_timeline_events(work_id, 80)?;
        let recent_operations = self.query_work_operations(work_id, 12)?;
        Ok(WorkDetail {
            item,
            launch_profiles,
            recent_sessions,
            save_profiles,
            save_snapshots,
            notes,
            timeline_events,
            recent_operations,
        })
    }

    pub fn list_operations(&self, limit: i64) -> rusqlite::Result<Vec<OperationItem>> {
        self.query_operations(limit.clamp(1, 100))
    }

    pub fn check_path_health(&mut self) -> rusqlite::Result<PathHealthReport> {
        let targets = self.query_installation_health_targets()?;
        let now = now_ms();
        let operation_id = Uuid::new_v4().to_string();
        let mut available_count = 0_i64;
        let mut missing_count = 0_i64;
        let mut changed_count = 0_i64;
        let mut issues = Vec::new();
        let mut affected_work_ids = Vec::new();

        let tx = self.conn.transaction()?;
        for target in &targets {
            let root_exists = PathBuf::from(&target.root_path).exists();
            let executable_exists = target
                .executable_path
                .as_ref()
                .map(|path| PathBuf::from(path).exists())
                .unwrap_or(true);
            let is_available = root_exists && executable_exists;
            if is_available {
                available_count += 1;
            } else {
                missing_count += 1;
                issues.push(PathHealthIssue {
                    work_id: target.work_id.clone(),
                    title: target.title.clone(),
                    installation_id: target.installation_id.clone(),
                    root_path: target.root_path.clone(),
                    executable_path: target.executable_path.clone(),
                    was_available: target.was_available,
                    is_available,
                });
            }

            if target.was_available != is_available {
                changed_count += 1;
                affected_work_ids.push(target.work_id.clone());
                tx.execute(
                    r#"
                    UPDATE installations
                    SET is_available = ?1,
                        updated_at = ?2
                    WHERE id = ?3
                    "#,
                    params![
                        if is_available { 1 } else { 0 },
                        now,
                        target.installation_id
                    ],
                )?;
            }
        }

        affected_work_ids.sort();
        affected_work_ids.dedup();
        for work_id in &affected_work_ids {
            refresh_installation_counts_tx(&tx, work_id, now)?;
        }

        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'library', 'path_health', 'path_health_check', ?2, ?3)
            "#,
            params![
                operation_id,
                json!({
                    "checked_count": targets.len(),
                    "available_count": available_count,
                    "missing_count": missing_count,
                    "changed_count": changed_count
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;

        Ok(PathHealthReport {
            checked_count: targets.len() as i64,
            available_count,
            missing_count,
            changed_count,
            issues,
            operation_id,
            checked_at: now,
        })
    }

    pub fn list_import_candidates(&self) -> rusqlite::Result<Vec<ImportCandidate>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, path, candidate_type, detected_title, detected_executable, size_bytes,
                   file_count, confidence, status, matched_work_id, evidence_json, created_at, updated_at
            FROM import_candidates
            ORDER BY
                CASE status
                    WHEN 'needs_review' THEN 0
                    WHEN 'pending' THEN 1
                    WHEN 'duplicate' THEN 2
                    ELSE 3
                END,
                confidence DESC,
                updated_at DESC
            LIMIT 500
            "#,
        )?;
        let rows = stmt.query_map([], import_candidate_from_row)?.collect();
        rows
    }

    pub fn list_library_roots(&self) -> rusqlite::Result<Vec<LibraryRoot>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, name, path, root_type, recursive, is_active, last_scanned_at, created_at, updated_at
            FROM library_roots
            WHERE deleted_at IS NULL
            ORDER BY COALESCE(last_scanned_at, updated_at) DESC, path ASC
            LIMIT 100
            "#,
        )?;
        let roots = stmt.query_map([], library_root_from_row)?.collect();
        roots
    }

    pub fn list_scan_jobs(&self, limit: i64) -> rusqlite::Result<Vec<ScanJob>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT sj.id, sj.root_id, lr.path, sj.status, sj.started_at, sj.finished_at,
                   sj.total_count, sj.matched_count, sj.failed_count, sj.log_json,
                   sj.created_at, sj.updated_at
            FROM scan_jobs sj
            LEFT JOIN library_roots lr ON lr.id = sj.root_id
            ORDER BY sj.created_at DESC
            LIMIT ?1
            "#,
        )?;
        let jobs = stmt
            .query_map(params![limit.clamp(1, 100)], scan_job_from_row)?
            .collect();
        jobs
    }

    pub fn scan_library_root(&mut self, path: &str) -> rusqlite::Result<Vec<ImportCandidate>> {
        let root = PathBuf::from(path);
        if !root.exists() {
            return Err(rusqlite::Error::InvalidPath(root));
        }

        let mut found = Vec::new();
        let started_at = now_ms();
        collect_candidates(&root, 0, &mut found);
        let finished_at = now_ms();
        let root_path = root.to_string_lossy().to_string();
        let root_id = Uuid::new_v4().to_string();
        let scan_job_id = Uuid::new_v4().to_string();
        let total_count = found.len() as i64;
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO library_roots (
                id, name, path, root_type, recursive, is_active, last_scanned_at, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, 'games', 1, 1, ?4, ?5, ?4)
            ON CONFLICT(path) DO UPDATE SET
                name = COALESCE(library_roots.name, excluded.name),
                is_active = 1,
                last_scanned_at = excluded.last_scanned_at,
                updated_at = excluded.updated_at,
                deleted_at = NULL,
                revision = revision + 1
            "#,
            params![
                root_id,
                root.file_name().and_then(|value| value.to_str()),
                root_path,
                finished_at,
                started_at
            ],
        )?;
        let root_id: String = tx.query_row(
            "SELECT id FROM library_roots WHERE path = ?1",
            params![root.to_string_lossy().to_string()],
            |row| row.get(0),
        )?;
        tx.execute(
            r#"
            INSERT INTO scan_jobs (
                id, root_id, status, started_at, finished_at, total_count, matched_count,
                failed_count, log_json, created_at, updated_at
            )
            VALUES (?1, ?2, 'completed', ?3, ?4, ?5, ?5, 0, ?6, ?3, ?4)
            "#,
            params![
                scan_job_id,
                root_id,
                started_at,
                finished_at,
                total_count,
                json!({
                    "root_path": root.to_string_lossy(),
                    "detector": "folder-executable-scan",
                    "candidate_count": total_count,
                    "depth_limited": true
                })
                .to_string()
            ],
        )?;
        for detected in found {
            let id = Uuid::new_v4().to_string();
            let evidence = json!({
                "detector": "folder-executable-scan",
                "depth_limited": true,
                "executable": detected.executable,
                "scan_job_id": scan_job_id
            })
            .to_string();
            tx.execute(
                r#"
                INSERT INTO import_candidates (
                    id, path, candidate_type, detected_title, detected_executable, size_bytes,
                    file_count, confidence, status, scan_job_id, root_id, evidence_json, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?10, ?11, ?12, ?12)
                ON CONFLICT(path) DO UPDATE SET
                    detected_title = excluded.detected_title,
                    detected_executable = excluded.detected_executable,
                    file_count = excluded.file_count,
                    confidence = excluded.confidence,
                    scan_job_id = excluded.scan_job_id,
                    root_id = excluded.root_id,
                    evidence_json = excluded.evidence_json,
                    updated_at = excluded.updated_at
                "#,
                params![
                    id,
                    detected.path,
                    detected.candidate_type,
                    detected.title,
                    detected.executable,
                    detected.size_bytes,
                    detected.file_count,
                    detected.confidence,
                    scan_job_id,
                    root_id,
                    evidence,
                    finished_at
                ],
            )?;
        }
        tx.commit()?;
        self.list_import_candidates()
    }

    pub fn accept_import_candidate(&mut self, candidate_id: &str) -> rusqlite::Result<LibraryItem> {
        let candidate = self.query_import_candidate(candidate_id)?;
        let title = candidate
            .detected_title
            .clone()
            .unwrap_or_else(|| title_from_path(Path::new(&candidate.path)));
        let now = now_ms();
        let work_id = Uuid::new_v4().to_string();
        let installation_id = Uuid::new_v4().to_string();
        let launch_profile_id = Uuid::new_v4().to_string();
        let op_id = Uuid::new_v4().to_string();

        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO works (
                id, title, original_title, sort_title, display_title, status,
                total_playtime_seconds, created_at, updated_at
            )
            VALUES (?1, ?2, NULL, ?2, ?2, 'unplayed', 0, ?3, ?3)
            "#,
            params![work_id, title, now],
        )?;
        tx.execute(
            r#"
            INSERT INTO library_items (
                work_id, display_title, original_title, sort_title, status,
                total_playtime_seconds, version_count, installation_count,
                available_installation_count, updated_at
            )
            VALUES (?1, ?2, NULL, ?2, 'unplayed', 0, 1, 1, 1, ?3)
            "#,
            params![work_id, title, now],
        )?;
        tx.execute(
            r#"
            INSERT INTO installations (
                id, work_id, root_path, executable_path, is_available, is_primary, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, 1, 1, ?5, ?5)
            "#,
            params![
                installation_id,
                work_id,
                candidate.path,
                candidate.detected_executable,
                now
            ],
        )?;
        if let Some(executable) = &candidate.detected_executable {
            tx.execute(
                r#"
                INSERT INTO launch_profiles (
                    id, work_id, installation_id, name, executable_path, working_dir, is_default, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, '默认启动', ?4, ?5, 1, ?6, ?6)
                "#,
                params![
                    launch_profile_id,
                    work_id,
                    installation_id,
                    executable,
                    candidate.path,
                    now
                ],
            )?;
        }
        tx.execute(
            "UPDATE import_candidates SET status = 'imported', matched_work_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![work_id, now, candidate_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO work_search_fts (work_id, title, original_title, aliases, developer, publisher, tags, description)
            VALUES (?1, ?2, '', '', '', '', '', '')
            "#,
            params![work_id, title],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'work', ?2, 'create', ?3, ?4)
            "#,
            params![
                op_id,
                work_id,
                json!({"source": "import_candidate", "candidate_id": candidate_id}).to_string(),
                now
            ],
        )?;
        tx.commit()?;

        self.query_library_item(&work_id)
    }

    pub fn accept_pending_import_candidates(&mut self) -> rusqlite::Result<BulkImportResult> {
        let pending: Vec<ImportCandidate> = self
            .list_import_candidates()?
            .into_iter()
            .filter(|candidate| matches!(candidate.status.as_str(), "pending" | "needs_review"))
            .collect();
        if pending.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "no pending import candidates".to_string(),
            ));
        }

        let snapshot = self.create_snapshot_with_type("before_bulk_import", "批量导入前快照")?;
        let mut imported_items = Vec::with_capacity(pending.len());
        let mut candidate_ids = Vec::with_capacity(pending.len());
        for candidate in pending {
            let item = self.accept_import_candidate(&candidate.id)?;
            candidate_ids.push(candidate.id);
            imported_items.push(item);
        }

        let now = now_ms();
        let operation_id = Uuid::new_v4().to_string();
        let work_ids: Vec<&str> = imported_items
            .iter()
            .map(|item| item.work_id.as_str())
            .collect();
        self.conn.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'library', 'bulk_import', 'bulk_import', ?2, ?3)
            "#,
            params![
                operation_id,
                json!({
                    "work_ids": work_ids,
                    "candidate_ids": candidate_ids,
                    "snapshot_id": snapshot.id
                })
                .to_string(),
                now
            ],
        )?;

        Ok(BulkImportResult {
            imported_count: imported_items.len(),
            imported_items,
            snapshot,
            operation_id,
        })
    }

    pub fn undo_latest_bulk_import(&mut self) -> rusqlite::Result<UndoResult> {
        let (operation_id, payload_json): (String, String) = self.conn.query_row(
            r#"
            SELECT id, payload_json
            FROM operations
            WHERE operation_type = 'bulk_import'
              AND id NOT IN (
                  SELECT json_extract(payload_json, '$.undone_operation_id')
                  FROM operations
                  WHERE operation_type = 'undo_bulk_import'
              )
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let payload: Value = serde_json::from_str(&payload_json)
            .map_err(|error| rusqlite::Error::InvalidParameterName(error.to_string()))?;
        let work_ids = json_string_array(&payload, "work_ids");
        let candidate_ids = json_string_array(&payload, "candidate_ids");
        if work_ids.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "bulk import operation has no work ids".to_string(),
            ));
        }

        let now = now_ms();
        let tx = self.conn.transaction()?;
        let affected_work_count = work_ids.len();
        let restored_candidate_count = candidate_ids.len();
        for work_id in &work_ids {
            soft_delete_work_tx(&tx, work_id, now)?;
        }
        for candidate_id in &candidate_ids {
            tx.execute(
                r#"
                UPDATE import_candidates
                SET status = 'pending',
                    matched_work_id = NULL,
                    matched_version_id = NULL,
                    updated_at = ?1
                WHERE id = ?2
                "#,
                params![now, candidate_id],
            )?;
        }
        let undo_operation_id = Uuid::new_v4().to_string();
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'library', 'bulk_import', 'undo_bulk_import', ?2, ?3)
            "#,
            params![
                undo_operation_id,
                json!({
                    "undone_operation_id": operation_id,
                    "work_ids": work_ids.clone(),
                    "candidate_ids": candidate_ids.clone()
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;

        Ok(UndoResult {
            operation_id: undo_operation_id,
            affected_work_count,
            restored_candidate_count,
        })
    }

    pub fn soft_delete_work(&mut self, work_id: &str) -> rusqlite::Result<MutationResult> {
        self.query_library_item(work_id)?;
        let now = now_ms();
        let operation_id = Uuid::new_v4().to_string();
        let tx = self.conn.transaction()?;
        soft_delete_work_tx(&tx, work_id, now)?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'work', ?2, 'delete', ?3, ?4)
            "#,
            params![
                operation_id,
                work_id,
                json!({"deleted_at": now}).to_string(),
                now
            ],
        )?;
        tx.commit()?;
        Ok(MutationResult {
            operation_id,
            affected_count: 1,
            message: "作品已移出书架".to_string(),
        })
    }

    pub fn record_manual_session(
        &mut self,
        work_id: &str,
        duration_seconds: i64,
        note: &str,
    ) -> rusqlite::Result<PlaySession> {
        let item = self.query_library_item(work_id)?;
        let now = now_ms();
        let duration_seconds = duration_seconds.max(60);
        let started_at = now - duration_seconds * 1000;
        let session_id = Uuid::new_v4().to_string();
        let op_id = Uuid::new_v4().to_string();
        let note_id = Uuid::new_v4().to_string();
        let day = date_key(now);

        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO play_sessions (
                id, work_id, started_at, ended_at, duration_seconds, source, confidence,
                is_confirmed, is_manual, timer_status, evidence_json, note, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'manual', 1.0, 1, 1, 'completed', ?6, ?7, ?4, ?4)
            "#,
            params![
                session_id,
                work_id,
                started_at,
                now,
                duration_seconds,
                json!({"detector": "manual-entry"}).to_string(),
                empty_to_null(note)
            ],
        )?;
        tx.execute(
            r#"
            UPDATE works
            SET total_playtime_seconds = total_playtime_seconds + ?1,
                last_played_at = ?2,
                first_played_at = COALESCE(first_played_at, ?3),
                updated_at = ?2
            WHERE id = ?4
            "#,
            params![duration_seconds, now, started_at, work_id],
        )?;
        tx.execute(
            r#"
            UPDATE library_items
            SET total_playtime_seconds = total_playtime_seconds + ?1,
                last_played_at = ?2,
                status = CASE WHEN status = 'unplayed' THEN 'playing' ELSE status END,
                updated_at = ?2
            WHERE work_id = ?3
            "#,
            params![duration_seconds, now, work_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO playtime_daily (
                id, date, work_id, duration_seconds, session_count,
                first_started_at, last_ended_at, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6, ?6)
            ON CONFLICT(date, work_id) DO UPDATE SET
                duration_seconds = duration_seconds + excluded.duration_seconds,
                session_count = session_count + 1,
                first_started_at = MIN(first_started_at, excluded.first_started_at),
                last_ended_at = MAX(last_ended_at, excluded.last_ended_at),
                updated_at = excluded.updated_at
            "#,
            params![
                Uuid::new_v4().to_string(),
                day,
                work_id,
                duration_seconds,
                started_at,
                now
            ],
        )?;
        if !note.trim().is_empty() {
            tx.execute(
                r#"
                INSERT INTO notes (
                    id, work_id, title, content, note_type, spoiler_level, privacy_level, created_at, updated_at
                )
                VALUES (?1, ?2, NULL, ?3, 'play_note', 0, 0, ?4, ?4)
                "#,
                params![note_id, work_id, note.trim(), now],
            )?;
        }
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'play_session', ?2, 'create', ?3, ?4)
            "#,
            params![
                op_id,
                session_id,
                json!({"work_id": work_id, "duration_seconds": duration_seconds}).to_string(),
                now
            ],
        )?;
        tx.commit()?;

        Ok(PlaySession {
            id: session_id,
            work_id: work_id.to_string(),
            title: item.display_title,
            started_at,
            ended_at: Some(now),
            duration_seconds,
            source: "manual".to_string(),
            confidence: 1.0,
            is_confirmed: true,
            timer_status: "completed".to_string(),
            note: empty_to_null(note).map(str::to_string),
        })
    }

    pub fn create_note(
        &mut self,
        work_id: &str,
        title: &str,
        content: &str,
        note_type: &str,
        spoiler_level: i64,
        privacy_level: i64,
    ) -> rusqlite::Result<NoteItem> {
        self.query_library_item(work_id)?;
        let content = content.trim();
        if content.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "note content cannot be empty".to_string(),
            ));
        }
        let now = now_ms();
        let note_id = Uuid::new_v4().to_string();
        let operation_id = Uuid::new_v4().to_string();
        let note_type = empty_to_null(note_type).unwrap_or("note");
        let spoiler_level = spoiler_level.clamp(0, 3);
        let privacy_level = privacy_level.clamp(0, 3);
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO notes (
                id, work_id, title, content, note_type, spoiler_level, privacy_level, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            "#,
            params![
                note_id,
                work_id,
                empty_to_null(title),
                content,
                note_type,
                spoiler_level,
                privacy_level,
                now
            ],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'note', ?2, 'create', ?3, ?4)
            "#,
            params![
                operation_id,
                note_id,
                json!({
                    "work_id": work_id,
                    "note_type": note_type,
                    "spoiler_level": spoiler_level,
                    "privacy_level": privacy_level
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.query_note(&note_id)
    }

    pub fn list_unconfirmed_sessions(&self) -> rusqlite::Result<Vec<PlaySession>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT ps.id, ps.work_id, li.display_title, ps.started_at, ps.ended_at,
                   ps.duration_seconds, ps.source, ps.confidence, ps.is_confirmed,
                   ps.timer_status, ps.note
            FROM play_sessions ps
            JOIN library_items li ON li.work_id = ps.work_id
            WHERE ps.is_confirmed = 0
              AND ps.timer_status = 'needs_review'
              AND ps.deleted_at IS NULL
            ORDER BY ps.ended_at DESC, ps.started_at DESC
            LIMIT 50
            "#,
        )?;
        let rows = stmt.query_map([], play_session_from_row)?.collect();
        rows
    }

    pub fn confirm_play_session(
        &mut self,
        session_id: &str,
        duration_seconds: i64,
        note: &str,
    ) -> rusqlite::Result<PlaySession> {
        let (work_id, started_at, ended_at, current_duration, is_confirmed, timer_status): (
            String,
            i64,
            Option<i64>,
            i64,
            i64,
            String,
        ) = self.conn.query_row(
            r#"
            SELECT work_id, started_at, ended_at, duration_seconds, is_confirmed, timer_status
            FROM play_sessions
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
            params![session_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;

        if is_confirmed == 1 {
            return self.query_session(session_id);
        }
        if timer_status == "discarded" {
            return Err(rusqlite::Error::InvalidParameterName(
                "discarded session cannot be confirmed".to_string(),
            ));
        }

        let now = now_ms();
        let ended_at = ended_at.unwrap_or(now);
        let duration_seconds = duration_seconds.max(1);
        let day = date_key(ended_at);
        let op_id = Uuid::new_v4().to_string();
        let note_id = Uuid::new_v4().to_string();
        let note = empty_to_null(note);
        let correction_reason = if duration_seconds != current_duration {
            Some("用户修正游玩回执")
        } else {
            None
        };

        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE play_sessions
            SET ended_at = ?1,
                duration_seconds = ?2,
                is_confirmed = 1,
                timer_status = 'completed',
                correction_reason = ?3,
                note = COALESCE(?4, note),
                updated_at = ?5
            WHERE id = ?6
            "#,
            params![
                ended_at,
                duration_seconds,
                correction_reason,
                note,
                now,
                session_id
            ],
        )?;
        apply_playtime_tx(&tx, &work_id, duration_seconds, started_at, ended_at, &day)?;
        if let Some(note) = note {
            tx.execute(
                r#"
                INSERT INTO notes (
                    id, work_id, title, content, note_type, spoiler_level, privacy_level, created_at, updated_at
                )
                VALUES (?1, ?2, NULL, ?3, 'play_receipt', 0, 0, ?4, ?4)
                "#,
                params![note_id, work_id, note, now],
            )?;
        }
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'play_session', ?2, 'confirm', ?3, ?4)
            "#,
            params![
                op_id,
                session_id,
                json!({
                    "duration_seconds": duration_seconds,
                    "previous_duration_seconds": current_duration,
                    "note_added": note.is_some()
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.query_session(session_id)
    }

    pub fn discard_play_session(
        &mut self,
        session_id: &str,
        reason: &str,
    ) -> rusqlite::Result<MutationResult> {
        let (_work_id, is_confirmed, timer_status): (String, i64, String) = self.conn.query_row(
            r#"
            SELECT work_id, is_confirmed, timer_status
            FROM play_sessions
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if is_confirmed == 1 && timer_status != "needs_review" {
            return Err(rusqlite::Error::InvalidParameterName(
                "only pending receipts can be discarded".to_string(),
            ));
        }

        let now = now_ms();
        let operation_id = Uuid::new_v4().to_string();
        let reason = empty_to_null(reason).unwrap_or("用户标记为误启动");
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE play_sessions
            SET duration_seconds = 0,
                is_confirmed = 1,
                timer_status = 'discarded',
                correction_reason = ?1,
                updated_at = ?2
            WHERE id = ?3
            "#,
            params![reason, now, session_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'play_session', ?2, 'discard', ?3, ?4)
            "#,
            params![
                operation_id,
                session_id,
                json!({"reason": reason}).to_string(),
                now
            ],
        )?;
        tx.commit()?;

        Ok(MutationResult {
            operation_id,
            affected_count: 1,
            message: "已标记为误启动，本次不会计入时长。".to_string(),
        })
    }

    pub fn create_snapshot(&mut self, reason: &str) -> rusqlite::Result<SnapshotInfo> {
        self.create_snapshot_with_type("manual", reason)
    }

    fn ensure_daily_snapshot(&mut self) -> rusqlite::Result<Option<SnapshotInfo>> {
        if !self.has_snapshot_worthy_data()? {
            return Ok(None);
        }

        let now = now_ms();
        if self.has_daily_snapshot_for_day(now)? {
            return Ok(None);
        }

        self.create_snapshot_with_type("daily", "每日自动快照")
            .map(Some)
    }

    fn has_snapshot_worthy_data(&self) -> rusqlite::Result<bool> {
        let has_data: i64 = self.conn.query_row(
            r#"
            SELECT CASE WHEN
                EXISTS(SELECT 1 FROM works WHERE deleted_at IS NULL)
                OR EXISTS(SELECT 1 FROM library_items)
                OR EXISTS(SELECT 1 FROM import_candidates)
                OR EXISTS(SELECT 1 FROM play_sessions WHERE deleted_at IS NULL)
                OR EXISTS(SELECT 1 FROM notes WHERE deleted_at IS NULL)
                OR EXISTS(SELECT 1 FROM save_profiles WHERE deleted_at IS NULL)
                OR EXISTS(SELECT 1 FROM save_snapshots WHERE deleted_at IS NULL)
            THEN 1 ELSE 0 END
            "#,
            [],
            |row| row.get(0),
        )?;
        Ok(has_data == 1)
    }

    fn has_daily_snapshot_for_day(&self, timestamp_ms: i64) -> rusqlite::Result<bool> {
        let day_start = (timestamp_ms / 86_400_000) * 86_400_000;
        let next_day_start = day_start + 86_400_000;
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM snapshots
            WHERE snapshot_type = 'daily'
              AND created_at >= ?1
              AND created_at < ?2
            "#,
            params![day_start, next_day_start],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn create_snapshot_with_type(
        &mut self,
        snapshot_type: &str,
        reason: &str,
    ) -> rusqlite::Result<SnapshotInfo> {
        let snapshot = self.create_snapshot_file(snapshot_type)?;
        self.insert_snapshot_record(&snapshot, reason)?;
        Ok(snapshot.to_info(reason))
    }

    fn create_snapshot_file(&self, snapshot_type: &str) -> rusqlite::Result<SnapshotFile> {
        let now = now_ms();
        let id = Uuid::new_v4().to_string();
        let snapshot_dir = self.app_dir.join("backups").join("snapshots");
        fs::create_dir_all(&snapshot_dir).map_err(io_to_sql)?;
        let file_name = format!("{}-{}-{}.db", now, snapshot_type, &id[..8]);
        let path = snapshot_dir.join(file_name);
        let path_string = path.to_string_lossy().to_string();
        self.conn
            .execute("VACUUM main INTO ?1", params![path_string.as_str()])?;
        let size_bytes = fs::metadata(&path)
            .map(|metadata| metadata.len() as i64)
            .ok();
        Ok(SnapshotFile {
            id,
            snapshot_type: snapshot_type.to_string(),
            path: path_string,
            size_bytes,
            created_at: now,
        })
    }

    fn insert_snapshot_record(
        &self,
        snapshot: &SnapshotFile,
        reason: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO snapshots (id, snapshot_type, path, reason, size_bytes, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                snapshot.id.as_str(),
                snapshot.snapshot_type.as_str(),
                snapshot.path.as_str(),
                empty_to_null(reason),
                snapshot.size_bytes,
                snapshot.created_at
            ],
        )?;
        Ok(())
    }

    fn import_cover_asset(&self, source_path: &str) -> rusqlite::Result<Option<CoverAsset>> {
        let source = PathBuf::from(source_path);
        if !source.is_file() {
            return Ok(None);
        }

        let id = Uuid::new_v4().to_string();
        let extension = asset_extension(&source);
        let file_name = format!("{id}.{extension}");
        let original_path = self
            .app_dir
            .join("assets")
            .join("originals")
            .join("covers")
            .join(&file_name);
        let thumbnail_256_path = self
            .app_dir
            .join("assets")
            .join("thumbnails")
            .join("256")
            .join("covers")
            .join(&file_name);
        let thumbnail_512_path = self
            .app_dir
            .join("assets")
            .join("thumbnails")
            .join("512")
            .join("covers")
            .join(&file_name);

        copy_path(&source, &original_path).map_err(io_to_sql)?;
        copy_path(&source, &thumbnail_256_path).map_err(io_to_sql)?;
        copy_path(&source, &thumbnail_512_path).map_err(io_to_sql)?;

        let size_bytes = fs::metadata(&original_path)
            .map(|metadata| metadata.len() as i64)
            .ok();
        let hash = file_checksum(&original_path).ok();

        Ok(Some(CoverAsset {
            id,
            original_path: original_path.to_string_lossy().to_string(),
            thumbnail_256_path: thumbnail_256_path.to_string_lossy().to_string(),
            thumbnail_512_path: thumbnail_512_path.to_string_lossy().to_string(),
            mime_type: mime_type_for_extension(&extension).map(str::to_string),
            size_bytes,
            hash,
        }))
    }

    pub fn app_dir(&self) -> PathBuf {
        self.app_dir.clone()
    }

    pub fn update_work_status(
        &mut self,
        work_id: &str,
        status: &str,
    ) -> rusqlite::Result<LibraryItem> {
        if !matches!(
            status,
            "unplayed" | "playing" | "completed" | "paused" | "dropped" | "wishlist" | "archived"
        ) {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "invalid status: {status}"
            )));
        }

        let now = now_ms();
        let completed_at: Option<i64> = if status == "completed" {
            Some(now)
        } else {
            None
        };
        let op_id = Uuid::new_v4().to_string();
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE works
            SET status = ?1,
                completed_at = CASE WHEN ?1 = 'completed' THEN ?2 ELSE completed_at END,
                updated_at = ?3,
                revision = revision + 1
            WHERE id = ?4
            "#,
            params![status, completed_at, now, work_id],
        )?;
        tx.execute(
            r#"
            UPDATE library_items
            SET status = ?1,
                completed_at = CASE WHEN ?1 = 'completed' THEN ?2 ELSE completed_at END,
                updated_at = ?3
            WHERE work_id = ?4
            "#,
            params![status, completed_at, now, work_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'work', ?2, 'update', ?3, ?4)
            "#,
            params![
                op_id,
                work_id,
                json!({"after": {"status": status, "completed_at": completed_at}}).to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.query_library_item(work_id)
    }

    pub fn update_work_metadata(
        &mut self,
        work_id: &str,
        title: &str,
        original_title: &str,
        user_rating: Option<f64>,
        tag_summary: &str,
        cover_path: &str,
        nsfw_level: i64,
        privacy_level: i64,
    ) -> rusqlite::Result<LibraryItem> {
        let title = title.trim();
        if title.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "title cannot be empty".to_string(),
            ));
        }
        let rating = user_rating.map(|value| value.clamp(0.0, 10.0));
        let now = now_ms();
        let original_title = empty_to_null(original_title);
        let tag_summary = empty_to_null(tag_summary);
        let nsfw_level = nsfw_level.clamp(0, 3);
        let privacy_level = privacy_level.clamp(0, 3);
        let requested_cover_path = empty_to_null(cover_path);
        let cover_asset = requested_cover_path
            .map(|path| self.import_cover_asset(path))
            .transpose()?
            .flatten();
        let stored_cover_path = cover_asset
            .as_ref()
            .map(|asset| asset.original_path.as_str())
            .or(requested_cover_path);
        let cover_thumbnail_path = cover_asset
            .as_ref()
            .map(|asset| asset.thumbnail_256_path.as_str());
        let cover_asset_id = cover_asset.as_ref().map(|asset| asset.id.as_str());
        let op_id = Uuid::new_v4().to_string();

        let tx = self.conn.transaction()?;
        if let Some(asset) = &cover_asset {
            insert_cover_asset_tx(&tx, asset, now)?;
            tx.execute(
                "DELETE FROM asset_links WHERE entity_type = 'work' AND entity_id = ?1 AND role = 'cover'",
                params![work_id],
            )?;
            tx.execute(
                r#"
                INSERT INTO asset_links (
                    id, asset_id, entity_type, entity_id, role, sort_order, created_at
                )
                VALUES (?1, ?2, 'work', ?3, 'cover', 0, ?4)
                "#,
                params![Uuid::new_v4().to_string(), asset.id.as_str(), work_id, now],
            )?;
        } else if requested_cover_path.is_none() {
            tx.execute(
                "DELETE FROM asset_links WHERE entity_type = 'work' AND entity_id = ?1 AND role = 'cover'",
                params![work_id],
            )?;
        }
        tx.execute(
            r#"
            UPDATE works
            SET title = ?1,
                display_title = ?1,
                sort_title = ?1,
                original_title = ?2,
                user_rating = ?3,
                cover_path = ?4,
                cover_thumbnail_path = ?5,
                cover_asset_id = ?6,
                nsfw_level = ?7,
                privacy_level = ?8,
                updated_at = ?9,
                revision = revision + 1
            WHERE id = ?10
            "#,
            params![
                title,
                original_title,
                rating,
                stored_cover_path,
                cover_thumbnail_path,
                cover_asset_id,
                nsfw_level,
                privacy_level,
                now,
                work_id
            ],
        )?;
        tx.execute(
            r#"
            UPDATE library_items
            SET display_title = ?1,
                sort_title = ?1,
                original_title = ?2,
                user_rating = ?3,
                tag_summary = ?4,
                cover_path = ?5,
                cover_thumbnail_path = ?6,
                cover_asset_id = ?7,
                nsfw_level = ?8,
                privacy_level = ?9,
                updated_at = ?10
            WHERE work_id = ?11
            "#,
            params![
                title,
                original_title,
                rating,
                tag_summary,
                stored_cover_path,
                cover_thumbnail_path,
                cover_asset_id,
                nsfw_level,
                privacy_level,
                now,
                work_id
            ],
        )?;
        tx.execute(
            "DELETE FROM work_search_fts WHERE work_id = ?1",
            params![work_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO work_search_fts (work_id, title, original_title, aliases, developer, publisher, tags, description)
            VALUES (?1, ?2, ?3, '', '', '', ?4, '')
            "#,
            params![work_id, title, original_title, tag_summary],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'work', ?2, 'update', ?3, ?4)
            "#,
            params![
                op_id,
                work_id,
                json!({
                    "after": {
                        "title": title,
                        "original_title": original_title,
                        "user_rating": rating,
                        "tag_summary": tag_summary,
                        "cover_path": stored_cover_path,
                        "cover_thumbnail_path": cover_thumbnail_path,
                        "cover_asset_id": cover_asset_id,
                        "nsfw_level": nsfw_level,
                        "privacy_level": privacy_level
                    }
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.query_library_item(work_id)
    }

    pub fn prepare_launch(&self, work_id: &str) -> rusqlite::Result<LaunchTarget> {
        self.conn.query_row(
            r#"
            SELECT li.work_id, li.display_title, lp.name, lp.executable_path, lp.working_dir, lp.arguments
            FROM library_items li
            JOIN launch_profiles lp ON lp.work_id = li.work_id
            WHERE li.work_id = ?1 AND lp.is_default = 1 AND lp.deleted_at IS NULL
            LIMIT 1
            "#,
            params![work_id],
            |row| {
                Ok(LaunchTarget {
                    work_id: row.get(0)?,
                    title: row.get(1)?,
                    profile_name: row.get(2)?,
                    executable_path: row.get(3)?,
                    working_dir: row.get(4)?,
                    arguments: row.get(5)?,
                })
            },
        )
    }

    pub fn upsert_default_launch_profile(
        &mut self,
        work_id: &str,
        name: &str,
        executable_path: &str,
        working_dir: &str,
        arguments: &str,
    ) -> rusqlite::Result<LaunchProfile> {
        self.query_library_item(work_id)?;
        let executable = PathBuf::from(executable_path);
        if !executable.exists() {
            return Err(rusqlite::Error::InvalidPath(executable));
        }
        if !executable.is_file() {
            return Err(rusqlite::Error::InvalidParameterName(
                "launch target must be a file".to_string(),
            ));
        }

        let working_dir_path = if let Some(working_dir) = empty_to_null(working_dir) {
            let path = PathBuf::from(working_dir);
            if !path.exists() || !path.is_dir() {
                return Err(rusqlite::Error::InvalidPath(path));
            }
            path
        } else {
            executable.parent().map(Path::to_path_buf).ok_or_else(|| {
                rusqlite::Error::InvalidParameterName(
                    "launch target has no parent directory".to_string(),
                )
            })?
        };

        let now = now_ms();
        let profile_name = empty_to_null(name).unwrap_or("默认启动");
        let executable_path = executable.to_string_lossy().to_string();
        let working_dir = working_dir_path.to_string_lossy().to_string();
        let arguments = empty_to_null(arguments).map(str::to_string);
        let existing: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                r#"
                SELECT id, installation_id
                FROM launch_profiles
                WHERE work_id = ?1 AND is_default = 1 AND deleted_at IS NULL
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                params![work_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let profile_id = existing
            .as_ref()
            .map(|(profile_id, _)| profile_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let installation_id = existing
            .and_then(|(_, installation_id)| installation_id)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let operation_id = Uuid::new_v4().to_string();
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE launch_profiles SET is_default = 0, updated_at = ?1 WHERE work_id = ?2 AND deleted_at IS NULL",
            params![now, work_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO installations (
                id, work_id, root_path, executable_path, storage_type, is_available, is_primary, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, 'local', 1, 1, ?5, ?5)
            ON CONFLICT(id) DO UPDATE SET
                root_path = excluded.root_path,
                executable_path = excluded.executable_path,
                is_available = 1,
                is_primary = 1,
                updated_at = excluded.updated_at,
                deleted_at = NULL
            "#,
            params![installation_id, work_id, working_dir, executable_path, now],
        )?;
        tx.execute(
            r#"
            INSERT INTO launch_profiles (
                id, work_id, installation_id, name, executable_path, working_dir, arguments,
                is_default, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)
            ON CONFLICT(id) DO UPDATE SET
                installation_id = excluded.installation_id,
                name = excluded.name,
                executable_path = excluded.executable_path,
                working_dir = excluded.working_dir,
                arguments = excluded.arguments,
                is_default = 1,
                updated_at = excluded.updated_at,
                deleted_at = NULL
            "#,
            params![
                profile_id,
                work_id,
                installation_id,
                profile_name,
                executable_path,
                working_dir,
                arguments,
                now
            ],
        )?;
        refresh_installation_counts_tx(&tx, work_id, now)?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'launch_profile', ?2, 'upsert_default', ?3, ?4)
            "#,
            params![
                operation_id,
                profile_id,
                json!({
                    "work_id": work_id,
                    "name": profile_name,
                    "executable_path": executable_path,
                    "working_dir": working_dir,
                    "arguments": arguments
                })
                .to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.query_launch_profile(&profile_id)
    }

    pub fn start_launch_session(
        &mut self,
        target: &LaunchTarget,
        process_id: Option<u32>,
    ) -> rusqlite::Result<LaunchReceipt> {
        let now = now_ms();
        let session_id = Uuid::new_v4().to_string();
        self.conn.execute(
            r#"
            INSERT INTO play_sessions (
                id, work_id, started_at, duration_seconds, source, confidence,
                is_confirmed, is_manual, timer_status, evidence_json, note, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, 0, 'auto', 0.75, 0, 0, 'running', ?4, NULL, ?3, ?3)
            "#,
            params![
                session_id,
                target.work_id,
                now,
                json!({
                    "launched_from_app": true,
                    "process_id": process_id,
                    "executable_path": target.executable_path,
                    "launch_profile": target.profile_name
                })
                .to_string()
            ],
        )?;
        Ok(LaunchReceipt {
            session_id,
            work_id: target.work_id.clone(),
            title: target.title.clone(),
            launch_profile_name: target.profile_name.clone(),
            executable_path: target.executable_path.clone(),
            process_id,
            started_at: now,
            status: "running".to_string(),
        })
    }

    pub fn finish_launch_session(
        &mut self,
        session_id: &str,
        exit_code: Option<i32>,
    ) -> rusqlite::Result<PlaySession> {
        let now = now_ms();
        let (_work_id, started_at, status): (String, i64, String) = self.conn.query_row(
            "SELECT work_id, started_at, timer_status FROM play_sessions WHERE id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        if status != "running" {
            return self.query_session(session_id);
        }

        let duration_seconds = ((now - started_at) / 1000).max(1);
        let op_id = Uuid::new_v4().to_string();
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE play_sessions
            SET ended_at = ?1,
                duration_seconds = ?2,
                confidence = 0.9,
                is_confirmed = 0,
                timer_status = 'needs_review',
                evidence_json = json_set(COALESCE(evidence_json, '{}'), '$.exit_code', json(?3), '$.exit_detected', true),
                updated_at = ?1
            WHERE id = ?4
            "#,
            params![
                now,
                duration_seconds,
                exit_code.map(|value| value.to_string()).unwrap_or_else(|| "null".to_string()),
                session_id
            ],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'play_session', ?2, 'update', ?3, ?4)
            "#,
            params![
                op_id,
                session_id,
                json!({"timer_status": "needs_review", "duration_seconds": duration_seconds})
                    .to_string(),
                now
            ],
        )?;
        tx.commit()?;
        self.query_session(session_id)
    }

    pub fn create_save_profile(
        &mut self,
        work_id: &str,
        name: &str,
        save_path: &str,
    ) -> rusqlite::Result<SaveProfile> {
        self.query_library_item(work_id)?;
        let source = PathBuf::from(save_path);
        if !source.exists() {
            return Err(rusqlite::Error::InvalidPath(source));
        }

        let now = now_ms();
        let id = Uuid::new_v4().to_string();
        let profile_name = if name.trim().is_empty() {
            "默认存档"
        } else {
            name.trim()
        };
        self.conn.execute(
            r#"
            INSERT INTO save_profiles (
                id, work_id, name, save_path, strategy, is_active, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, 'copy', 1, ?5, ?5)
            "#,
            params![id, work_id, profile_name, save_path, now],
        )?;
        self.query_save_profile(&id)
    }

    pub fn create_save_snapshot(
        &mut self,
        work_id: &str,
        save_profile_id: Option<&str>,
        source_path: &str,
        note: &str,
        route_name: &str,
        progress_label: &str,
    ) -> rusqlite::Result<SaveSnapshot> {
        self.query_library_item(work_id)?;
        let profile_path =
            if let Some(profile_id) = save_profile_id.filter(|value| !value.is_empty()) {
                let profile = self.query_save_profile(profile_id)?;
                if profile.work_id != work_id {
                    return Err(rusqlite::Error::InvalidParameterName(
                        "save profile belongs to another work".to_string(),
                    ));
                }
                profile.save_path
            } else {
                source_path.to_string()
            };

        let source = PathBuf::from(&profile_path);
        if !source.exists() {
            return Err(rusqlite::Error::InvalidPath(source));
        }

        let now = now_ms();
        let id = Uuid::new_v4().to_string();
        let snapshot_root = self
            .app_dir
            .join("saves")
            .join("snapshots")
            .join(work_id)
            .join(&id);
        let files_dir = snapshot_root.join("files");
        copy_path(&source, &files_dir).map_err(io_to_sql)?;
        fs::write(
            snapshot_root.join("manifest.json"),
            json!({
                "work_id": work_id,
                "source_path": profile_path,
                "created_at": now,
                "note": empty_to_null(note),
                "route_name": empty_to_null(route_name),
                "progress_label": empty_to_null(progress_label)
            })
            .to_string(),
        )
        .map_err(io_to_sql)?;

        self.conn.execute(
            r#"
            INSERT INTO save_snapshots (
                id, work_id, save_profile_id, snapshot_path, note, route_name,
                progress_label, is_locked, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8)
            "#,
            params![
                id,
                work_id,
                save_profile_id.filter(|value| !value.is_empty()),
                files_dir.to_string_lossy().to_string(),
                empty_to_null(note),
                empty_to_null(route_name),
                empty_to_null(progress_label),
                now
            ],
        )?;
        self.query_save_snapshot(&id)
    }

    pub fn restore_save_snapshot(
        &mut self,
        snapshot_id: &str,
        target_path: &str,
    ) -> rusqlite::Result<SaveSnapshot> {
        let snapshot = self.query_save_snapshot(snapshot_id)?;
        let source = PathBuf::from(&snapshot.snapshot_path);
        let target = PathBuf::from(target_path);
        if !source.exists() {
            return Err(rusqlite::Error::InvalidPath(source));
        }

        let now = now_ms();
        if target.exists() {
            let backup = self
                .app_dir
                .join("saves")
                .join("restore-backups")
                .join(&snapshot.work_id)
                .join(format!("{}-{}", snapshot.id, now));
            copy_path(&target, &backup).map_err(io_to_sql)?;
            remove_path(&target).map_err(io_to_sql)?;
        }
        copy_path(&source, &target).map_err(io_to_sql)?;

        self.conn.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'save_snapshot', ?2, 'restore', ?3, ?4)
            "#,
            params![
                Uuid::new_v4().to_string(),
                snapshot_id,
                json!({"target_path": target_path}).to_string(),
                now
            ],
        )?;
        Ok(snapshot)
    }

    pub fn set_save_snapshot_locked(
        &mut self,
        snapshot_id: &str,
        is_locked: bool,
    ) -> rusqlite::Result<SaveSnapshot> {
        let snapshot = self.query_save_snapshot(snapshot_id)?;
        let now = now_ms();
        let operation_id = Uuid::new_v4().to_string();
        self.conn.execute(
            r#"
            UPDATE save_snapshots
            SET is_locked = ?1,
                updated_at = ?2
            WHERE id = ?3 AND deleted_at IS NULL
            "#,
            params![if is_locked { 1 } else { 0 }, now, snapshot_id],
        )?;
        self.conn.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'save_snapshot', ?2, 'lock_update', ?3, ?4)
            "#,
            params![
                operation_id,
                snapshot_id,
                json!({"work_id": snapshot.work_id, "is_locked": is_locked}).to_string(),
                now
            ],
        )?;
        self.query_save_snapshot(snapshot_id)
    }

    pub fn delete_save_snapshot(&mut self, snapshot_id: &str) -> rusqlite::Result<MutationResult> {
        let snapshot = self.query_save_snapshot(snapshot_id)?;
        if snapshot.is_locked {
            return Err(rusqlite::Error::InvalidParameterName(
                "locked save snapshot cannot be deleted".to_string(),
            ));
        }

        let now = now_ms();
        let operation_id = Uuid::new_v4().to_string();
        let snapshot_root = PathBuf::from(&snapshot.snapshot_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(&snapshot.snapshot_path));
        let tx = self.conn.transaction()?;
        tx.execute(
            r#"
            UPDATE save_snapshots
            SET deleted_at = ?1,
                updated_at = ?1
            WHERE id = ?2 AND deleted_at IS NULL
            "#,
            params![now, snapshot_id],
        )?;
        tx.execute(
            r#"
            INSERT INTO operations (
                id, device_id, entity_type, entity_id, operation_type, payload_json, created_at
            )
            VALUES (?1, 'local', 'save_snapshot', ?2, 'delete', ?3, ?4)
            "#,
            params![
                operation_id,
                snapshot_id,
                json!({
                    "work_id": snapshot.work_id,
                    "snapshot_path": snapshot.snapshot_path,
                    "deleted_files": snapshot_root.exists()
                })
                .to_string(),
                now
            ],
        )?;
        if snapshot_root.exists() {
            remove_path(&snapshot_root).map_err(io_to_sql)?;
        }
        tx.commit()?;

        Ok(MutationResult {
            operation_id,
            affected_count: 1,
            message: "存档快照已删除。".to_string(),
        })
    }

    fn query_session(&self, session_id: &str) -> rusqlite::Result<PlaySession> {
        self.conn.query_row(
            r#"
            SELECT ps.id, ps.work_id, li.display_title, ps.started_at, ps.ended_at,
                   ps.duration_seconds, ps.source, ps.confidence, ps.is_confirmed,
                   ps.timer_status, ps.note
            FROM play_sessions ps
            JOIN library_items li ON li.work_id = ps.work_id
            WHERE ps.id = ?1
            "#,
            params![session_id],
            play_session_from_row,
        )
    }

    fn query_save_profiles(&self, work_id: &str) -> rusqlite::Result<Vec<SaveProfile>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, work_id, name, save_path, engine, strategy, is_active, created_at, updated_at
            FROM save_profiles
            WHERE work_id = ?1 AND deleted_at IS NULL
            ORDER BY is_active DESC, updated_at DESC
            "#,
        )?;
        let rows = stmt
            .query_map(params![work_id], save_profile_from_row)?
            .collect();
        rows
    }

    fn query_launch_profiles(&self, work_id: &str) -> rusqlite::Result<Vec<LaunchProfile>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, work_id, installation_id, name, executable_path, working_dir, arguments,
                   is_default, created_at, updated_at
            FROM launch_profiles
            WHERE work_id = ?1 AND deleted_at IS NULL
            ORDER BY is_default DESC, updated_at DESC
            "#,
        )?;
        let rows = stmt
            .query_map(params![work_id], launch_profile_from_row)?
            .collect();
        rows
    }

    fn query_launch_profile(&self, profile_id: &str) -> rusqlite::Result<LaunchProfile> {
        self.conn.query_row(
            r#"
            SELECT id, work_id, installation_id, name, executable_path, working_dir, arguments,
                   is_default, created_at, updated_at
            FROM launch_profiles
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
            params![profile_id],
            launch_profile_from_row,
        )
    }

    fn query_save_profile(&self, profile_id: &str) -> rusqlite::Result<SaveProfile> {
        self.conn.query_row(
            r#"
            SELECT id, work_id, name, save_path, engine, strategy, is_active, created_at, updated_at
            FROM save_profiles
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
            params![profile_id],
            save_profile_from_row,
        )
    }

    fn query_save_snapshots(
        &self,
        work_id: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<SaveSnapshot>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, work_id, save_profile_id, snapshot_path, note, route_name,
                   progress_label, is_locked, created_at, updated_at
            FROM save_snapshots
            WHERE work_id = ?1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt
            .query_map(params![work_id, limit], save_snapshot_from_row)?
            .collect();
        rows
    }

    fn query_save_snapshot(&self, snapshot_id: &str) -> rusqlite::Result<SaveSnapshot> {
        self.conn.query_row(
            r#"
            SELECT id, work_id, save_profile_id, snapshot_path, note, route_name,
                   progress_label, is_locked, created_at, updated_at
            FROM save_snapshots
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
            params![snapshot_id],
            save_snapshot_from_row,
        )
    }

    fn query_library_items(
        &self,
        search: &str,
        status: &str,
        limit: i64,
        offset: i64,
    ) -> rusqlite::Result<Vec<LibraryItem>> {
        if !search.trim().is_empty() {
            if let Ok(items) = self.query_library_items_fts(search, status, limit, offset) {
                return Ok(items);
            }
        }

        let search_like = format!("%{}%", search.trim());
        let status_filter = status.trim();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT work_id, display_title, original_title, status, user_rating, cover_path,
                   cover_thumbnail_path, nsfw_level, privacy_level, total_playtime_seconds, last_played_at, completed_at, version_count,
                   installation_count, available_installation_count, tag_summary, updated_at
            FROM library_items
            WHERE (?1 = '' OR display_title LIKE ?2 OR COALESCE(original_title, '') LIKE ?2 OR COALESCE(tag_summary, '') LIKE ?2)
              AND (?3 = '' OR status = ?3)
            ORDER BY COALESCE(last_played_at, updated_at) DESC, display_title ASC
            LIMIT ?4
            OFFSET ?5
            "#,
        )?;
        let rows = stmt
            .query_map(
                params![search.trim(), search_like, status_filter, limit, offset],
                library_item_from_row,
            )?
            .collect();
        rows
    }

    fn query_library_items_fts(
        &self,
        search: &str,
        status: &str,
        limit: i64,
        offset: i64,
    ) -> rusqlite::Result<Vec<LibraryItem>> {
        let query = fts_query(search);
        let status_filter = status.trim();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT li.work_id, li.display_title, li.original_title, li.status, li.user_rating, li.cover_path,
                   li.cover_thumbnail_path, li.nsfw_level, li.privacy_level, li.total_playtime_seconds, li.last_played_at, li.completed_at, li.version_count,
                   li.installation_count, li.available_installation_count, li.tag_summary, li.updated_at
            FROM work_search_fts f
            JOIN library_items li ON li.work_id = f.work_id
            WHERE work_search_fts MATCH ?1
              AND (?2 = '' OR li.status = ?2)
            ORDER BY rank, COALESCE(li.last_played_at, li.updated_at) DESC
            LIMIT ?3
            OFFSET ?4
            "#,
        )?;
        let rows = stmt
            .query_map(
                params![query, status_filter, limit, offset],
                library_item_from_row,
            )?
            .collect();
        rows
    }

    fn count_library_items(&self, search: &str, status: &str) -> rusqlite::Result<i64> {
        if !search.trim().is_empty() {
            if let Ok(count) = self.count_library_items_fts(search, status) {
                return Ok(count);
            }
        }

        let search_like = format!("%{}%", search.trim());
        let status_filter = status.trim();
        self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM library_items
            WHERE (?1 = '' OR display_title LIKE ?2 OR COALESCE(original_title, '') LIKE ?2 OR COALESCE(tag_summary, '') LIKE ?2)
              AND (?3 = '' OR status = ?3)
            "#,
            params![search.trim(), search_like, status_filter],
            |row| row.get(0),
        )
    }

    fn count_library_items_fts(&self, search: &str, status: &str) -> rusqlite::Result<i64> {
        let query = fts_query(search);
        let status_filter = status.trim();
        self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM work_search_fts f
            JOIN library_items li ON li.work_id = f.work_id
            WHERE work_search_fts MATCH ?1
              AND (?2 = '' OR li.status = ?2)
            "#,
            params![query, status_filter],
            |row| row.get(0),
        )
    }

    fn query_library_item(&self, work_id: &str) -> rusqlite::Result<LibraryItem> {
        self.conn.query_row(
            r#"
            SELECT work_id, display_title, original_title, status, user_rating, cover_path,
                   cover_thumbnail_path, nsfw_level, privacy_level, total_playtime_seconds, last_played_at, completed_at, version_count,
                   installation_count, available_installation_count, tag_summary, updated_at
            FROM library_items
            WHERE work_id = ?1
            "#,
            params![work_id],
            library_item_from_row,
        )
    }

    fn query_continue_items(&self, limit: i64) -> rusqlite::Result<Vec<ContinueItem>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT li.work_id, li.display_title, li.original_title, li.status, li.user_rating, li.cover_path,
                   li.cover_thumbnail_path, li.nsfw_level, li.privacy_level, li.total_playtime_seconds, li.last_played_at, li.completed_at, li.version_count,
                   li.installation_count, li.available_installation_count, li.tag_summary, li.updated_at,
                   lp.name,
                   ss.progress_label
            FROM library_items li
            LEFT JOIN launch_profiles lp ON lp.work_id = li.work_id AND lp.is_default = 1 AND lp.deleted_at IS NULL
            LEFT JOIN save_snapshots ss ON ss.id = (
                SELECT id FROM save_snapshots
                WHERE work_id = li.work_id AND deleted_at IS NULL
                ORDER BY created_at DESC
                LIMIT 1
            )
            WHERE li.status IN ('playing', 'paused', 'unplayed')
            ORDER BY COALESCE(li.last_played_at, li.updated_at) DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(ContinueItem {
                    item: LibraryItem {
                        work_id: row.get(0)?,
                        display_title: row.get(1)?,
                        original_title: row.get(2)?,
                        status: row.get(3)?,
                        user_rating: row.get(4)?,
                        cover_path: row.get(5)?,
                        cover_thumbnail_path: row.get(6)?,
                        nsfw_level: row.get(7)?,
                        privacy_level: row.get(8)?,
                        total_playtime_seconds: row.get(9)?,
                        last_played_at: row.get(10)?,
                        completed_at: row.get(11)?,
                        version_count: row.get(12)?,
                        installation_count: row.get(13)?,
                        available_installation_count: row.get(14)?,
                        tag_summary: row.get(15)?,
                        updated_at: row.get(16)?,
                    },
                    default_launch_name: row.get(17)?,
                    latest_save_label: row.get(18)?,
                })
            })?
            .collect();
        rows
    }

    fn query_recent_sessions(
        &self,
        work_id: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<PlaySession>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT ps.id, ps.work_id, li.display_title, ps.started_at, ps.ended_at,
                   ps.duration_seconds, ps.source, ps.confidence, ps.is_confirmed,
                   ps.timer_status, ps.note
            FROM play_sessions ps
            JOIN library_items li ON li.work_id = ps.work_id
            WHERE ps.work_id = ?1 AND ps.deleted_at IS NULL
            ORDER BY ps.started_at DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt
            .query_map(params![work_id, limit], play_session_from_row)?
            .collect();
        rows
    }

    fn query_notes(&self, work_id: &str, limit: i64) -> rusqlite::Result<Vec<NoteItem>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, content, note_type, spoiler_level, privacy_level, created_at, updated_at
            FROM notes
            WHERE work_id = ?1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt
            .query_map(params![work_id, limit], note_from_row)?
            .collect();
        rows
    }

    fn query_note(&self, note_id: &str) -> rusqlite::Result<NoteItem> {
        self.conn.query_row(
            r#"
            SELECT id, title, content, note_type, spoiler_level, privacy_level, created_at, updated_at
            FROM notes
            WHERE id = ?1 AND deleted_at IS NULL
            "#,
            params![note_id],
            note_from_row,
        )
    }

    fn query_timeline_events(
        &self,
        work_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<TimelineEvent>> {
        let mut events = Vec::new();

        for session in self.query_recent_sessions(work_id, limit as i64)? {
            let status_label = if session.is_confirmed {
                "已确认"
            } else {
                "待确认"
            };
            events.push(TimelineEvent {
                id: format!("play_session:{}", session.id),
                work_id: session.work_id.clone(),
                event_type: "play_session".to_string(),
                source_id: session.id,
                occurred_at: session.ended_at.unwrap_or(session.started_at),
                title: format!("游玩 {}", format_duration_cn(session.duration_seconds)),
                summary: session.note.clone(),
                detail: Some(format!(
                    "{} · {} · 可信度 {}%",
                    session.source,
                    status_label,
                    (session.confidence * 100.0).round() as i64
                )),
                duration_seconds: Some(session.duration_seconds),
                privacy_level: 0,
                spoiler_level: 0,
            });
        }

        for snapshot in self.query_save_snapshots(work_id, limit as i64)? {
            let title = snapshot
                .progress_label
                .clone()
                .or(snapshot.note.clone())
                .unwrap_or_else(|| "存档快照".to_string());
            events.push(TimelineEvent {
                id: format!("save_snapshot:{}", snapshot.id),
                work_id: snapshot.work_id.clone(),
                event_type: "save_snapshot".to_string(),
                source_id: snapshot.id,
                occurred_at: snapshot.created_at,
                title,
                summary: snapshot.route_name.clone(),
                detail: Some(snapshot.snapshot_path),
                duration_seconds: None,
                privacy_level: 0,
                spoiler_level: 0,
            });
        }

        for note in self.query_notes(work_id, limit as i64)? {
            events.push(TimelineEvent {
                id: format!("note:{}", note.id),
                work_id: work_id.to_string(),
                event_type: note.note_type.clone(),
                source_id: note.id,
                occurred_at: note.created_at,
                title: note.title.clone().unwrap_or_else(|| "笔记".to_string()),
                summary: Some(truncate_summary(&note.content, 120)),
                detail: Some(note.content),
                duration_seconds: None,
                privacy_level: note.privacy_level,
                spoiler_level: note.spoiler_level,
            });
        }

        events.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        events.truncate(limit);
        Ok(events)
    }

    fn query_operations(&self, limit: i64) -> rusqlite::Result<Vec<OperationItem>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, entity_type, entity_id, operation_type, payload_json, created_at, synced_at
            FROM operations
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt
            .query_map(params![limit], operation_from_row)?
            .collect();
        rows
    }

    fn query_work_operations(
        &self,
        work_id: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<OperationItem>> {
        let like_work_id = format!("%{}%", work_id);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, entity_type, entity_id, operation_type, payload_json, created_at, synced_at
            FROM operations
            WHERE entity_id = ?1
               OR json_extract(payload_json, '$.work_id') = ?1
               OR payload_json LIKE ?2
            ORDER BY created_at DESC
            LIMIT ?3
            "#,
        )?;
        let rows = stmt
            .query_map(params![work_id, like_work_id, limit], operation_from_row)?
            .collect();
        rows
    }

    fn query_installation_health_targets(&self) -> rusqlite::Result<Vec<InstallationHealthTarget>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT i.id, i.work_id, li.display_title, i.root_path, i.executable_path, i.is_available
            FROM installations i
            JOIN library_items li ON li.work_id = i.work_id
            WHERE i.deleted_at IS NULL
            ORDER BY li.display_title ASC
            "#,
        )?;
        let rows = stmt
            .query_map([], |row| {
                let available: i64 = row.get(5)?;
                Ok(InstallationHealthTarget {
                    installation_id: row.get(0)?,
                    work_id: row.get(1)?,
                    title: row.get(2)?,
                    root_path: row.get(3)?,
                    executable_path: row.get(4)?,
                    was_available: available == 1,
                })
            })?
            .collect();
        rows
    }

    fn query_last_snapshot(&self) -> rusqlite::Result<Option<SnapshotInfo>> {
        self.conn
            .query_row(
                r#"
                SELECT id, snapshot_type, reason, path, created_at
                FROM snapshots
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                [],
                snapshot_from_row,
            )
            .optional()
    }

    fn query_month_stats(&self) -> rusqlite::Result<MonthStats> {
        let since = now_ms() - THIRTY_DAYS_MS;
        let (duration_seconds, active_days): (i64, i64) = self.conn.query_row(
            r#"
            SELECT COALESCE(SUM(duration_seconds), 0),
                   COUNT(DISTINCT strftime('%Y-%m-%d', started_at / 1000, 'unixepoch'))
            FROM play_sessions
            WHERE started_at >= ?1 AND deleted_at IS NULL
            "#,
            params![since],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let completed_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM works WHERE completed_at >= ?1 AND deleted_at IS NULL",
            params![since],
            |row| row.get(0),
        )?;
        let most_played_title = self
            .conn
            .query_row(
                r#"
                SELECT li.display_title
                FROM play_sessions ps
                JOIN library_items li ON li.work_id = ps.work_id
                WHERE ps.started_at >= ?1 AND ps.deleted_at IS NULL
                GROUP BY ps.work_id
                ORDER BY SUM(ps.duration_seconds) DESC
                LIMIT 1
                "#,
                params![since],
                |row| row.get(0),
            )
            .optional()?;

        Ok(MonthStats {
            duration_seconds,
            active_days,
            completed_count,
            most_played_title,
        })
    }

    fn query_import_candidate(&self, candidate_id: &str) -> rusqlite::Result<ImportCandidate> {
        self.conn.query_row(
            r#"
            SELECT id, path, candidate_type, detected_title, detected_executable, size_bytes,
                   file_count, confidence, status, matched_work_id, evidence_json, created_at, updated_at
            FROM import_candidates
            WHERE id = ?1
            "#,
            params![candidate_id],
            import_candidate_from_row,
        )
    }
}

fn library_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryItem> {
    Ok(LibraryItem {
        work_id: row.get(0)?,
        display_title: row.get(1)?,
        original_title: row.get(2)?,
        status: row.get(3)?,
        user_rating: row.get(4)?,
        cover_path: row.get(5)?,
        cover_thumbnail_path: row.get(6)?,
        nsfw_level: row.get(7)?,
        privacy_level: row.get(8)?,
        total_playtime_seconds: row.get(9)?,
        last_played_at: row.get(10)?,
        completed_at: row.get(11)?,
        version_count: row.get(12)?,
        installation_count: row.get(13)?,
        available_installation_count: row.get(14)?,
        tag_summary: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn import_candidate_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportCandidate> {
    Ok(ImportCandidate {
        id: row.get(0)?,
        path: row.get(1)?,
        candidate_type: row.get(2)?,
        detected_title: row.get(3)?,
        detected_executable: row.get(4)?,
        size_bytes: row.get(5)?,
        file_count: row.get(6)?,
        confidence: row.get(7)?,
        status: row.get(8)?,
        matched_work_id: row.get(9)?,
        evidence_json: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn library_root_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryRoot> {
    let recursive: i64 = row.get(4)?;
    let is_active: i64 = row.get(5)?;
    Ok(LibraryRoot {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        root_type: row.get(3)?,
        recursive: recursive == 1,
        is_active: is_active == 1,
        last_scanned_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn scan_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanJob> {
    Ok(ScanJob {
        id: row.get(0)?,
        root_id: row.get(1)?,
        root_path: row.get(2)?,
        status: row.get(3)?,
        started_at: row.get(4)?,
        finished_at: row.get(5)?,
        total_count: row.get(6)?,
        matched_count: row.get(7)?,
        failed_count: row.get(8)?,
        log_json: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn play_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlaySession> {
    let confirmed: i64 = row.get(8)?;
    Ok(PlaySession {
        id: row.get(0)?,
        work_id: row.get(1)?,
        title: row.get(2)?,
        started_at: row.get(3)?,
        ended_at: row.get(4)?,
        duration_seconds: row.get(5)?,
        source: row.get(6)?,
        confidence: row.get(7)?,
        is_confirmed: confirmed == 1,
        timer_status: row.get(9)?,
        note: row.get(10)?,
    })
}

fn launch_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LaunchProfile> {
    let default_value: i64 = row.get(7)?;
    let executable_path: String = row.get(4)?;
    Ok(LaunchProfile {
        id: row.get(0)?,
        work_id: row.get(1)?,
        installation_id: row.get(2)?,
        name: row.get(3)?,
        is_available: PathBuf::from(&executable_path).exists(),
        executable_path,
        working_dir: row.get(5)?,
        arguments: row.get(6)?,
        is_default: default_value == 1,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn save_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveProfile> {
    let active: i64 = row.get(6)?;
    Ok(SaveProfile {
        id: row.get(0)?,
        work_id: row.get(1)?,
        name: row.get(2)?,
        save_path: row.get(3)?,
        engine: row.get(4)?,
        strategy: row.get(5)?,
        is_active: active == 1,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn save_snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveSnapshot> {
    let locked: i64 = row.get(7)?;
    Ok(SaveSnapshot {
        id: row.get(0)?,
        work_id: row.get(1)?,
        save_profile_id: row.get(2)?,
        snapshot_path: row.get(3)?,
        note: row.get(4)?,
        route_name: row.get(5)?,
        progress_label: row.get(6)?,
        is_locked: locked == 1,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NoteItem> {
    Ok(NoteItem {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        note_type: row.get(3)?,
        spoiler_level: row.get(4)?,
        privacy_level: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationItem> {
    let entity_type: String = row.get(1)?;
    let operation_type: String = row.get(3)?;
    let payload_json: String = row.get(4)?;
    let synced_at: Option<i64> = row.get(6)?;
    Ok(OperationItem {
        id: row.get(0)?,
        summary: operation_summary(&entity_type, &operation_type),
        entity_type,
        entity_id: row.get(2)?,
        operation_type,
        payload_json,
        created_at: row.get(5)?,
        synced_at,
        is_synced: synced_at.is_some(),
    })
}

fn snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SnapshotInfo> {
    Ok(SnapshotInfo {
        id: row.get(0)?,
        snapshot_type: row.get(1)?,
        reason: row.get(2)?,
        path: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn collect_candidates(root: &Path, depth: usize, found: &mut Vec<DetectedCandidate>) {
    if depth > 3 || found.len() >= 500 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    let mut executable: Option<String> = None;
    let mut file_count = 0_i64;
    let mut size_bytes = 0_i64;
    let mut child_dirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            child_dirs.push(path);
            continue;
        }
        file_count += 1;
        if let Ok(metadata) = entry.metadata() {
            size_bytes += metadata.len() as i64;
        }
        if executable.is_none() && looks_launchable(&path) {
            executable = Some(path.to_string_lossy().to_string());
        }
    }

    if let Some(executable) = executable {
        found.push(DetectedCandidate {
            path: root.to_string_lossy().to_string(),
            candidate_type: "folder".to_string(),
            title: title_from_path(root),
            executable: Some(executable),
            size_bytes: Some(size_bytes),
            file_count: Some(file_count),
            confidence: if depth <= 1 { 0.72 } else { 0.58 },
        });
        return;
    }

    for child in child_dirs {
        collect_candidates(&child, depth + 1, found);
    }
}

struct DetectedCandidate {
    path: String,
    candidate_type: String,
    title: String,
    executable: Option<String>,
    size_bytes: Option<i64>,
    file_count: Option<i64>,
    confidence: f64,
}

struct InstallationHealthTarget {
    installation_id: String,
    work_id: String,
    title: String,
    root_path: String,
    executable_path: Option<String>,
    was_available: bool,
}

fn looks_launchable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "exe" | "bat" | "cmd" | "sh"
            )
        })
        .unwrap_or(false)
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("未命名作品")
        .replace(['_', '.'], " ")
}

fn empty_to_null(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn date_key(timestamp_ms: i64) -> String {
    let seconds = timestamp_ms / 1000;
    let days = seconds / 86_400;
    format!("day-{}", days)
}

fn format_duration_cn(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours} 小时 {minutes} 分钟")
    } else {
        format!("{} 分钟", minutes.max(1))
    }
}

fn truncate_summary(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for character in value.chars().take(max_chars) {
        output.push(character);
    }
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    output
}

fn operation_summary(entity_type: &str, operation_type: &str) -> String {
    match (entity_type, operation_type) {
        ("work", "create") => "创建作品".to_string(),
        ("work", "update") => "更新作品资料".to_string(),
        ("work", "delete") => "移出书架".to_string(),
        ("library", "bulk_import") => "批量导入作品".to_string(),
        ("library", "undo_bulk_import") => "撤销批量导入".to_string(),
        ("library", "path_health_check") => "检查路径健康".to_string(),
        ("play_session", "create") => "记录游玩".to_string(),
        ("play_session", "update") => "生成游玩回执".to_string(),
        ("play_session", "confirm") => "确认游玩回执".to_string(),
        ("play_session", "discard") => "标记误启动".to_string(),
        ("launch_profile", "upsert_default") => "更新默认启动方式".to_string(),
        ("note", "create") => "创建笔记".to_string(),
        ("save_snapshot", "lock_update") => "更新存档快照锁定".to_string(),
        ("save_snapshot", "restore") => "恢复存档快照".to_string(),
        ("save_snapshot", "delete") => "删除存档快照".to_string(),
        (entity, operation) => format!("{entity} · {operation}"),
    }
}

fn fts_query(search: &str) -> String {
    let terms: Vec<String> = search
        .split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|ch| {
                    ch.is_alphanumeric() || *ch == '_' || ('\u{4e00}'..='\u{9fff}').contains(ch)
                })
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .collect();
    if terms.is_empty() {
        "\"\"".to_string()
    } else {
        terms.join(" ")
    }
}

fn json_string_array(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn soft_delete_work_tx(
    tx: &rusqlite::Transaction<'_>,
    work_id: &str,
    deleted_at: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE works SET deleted_at = ?1, updated_at = ?1, revision = revision + 1 WHERE id = ?2",
        params![deleted_at, work_id],
    )?;
    tx.execute(
        "UPDATE installations SET deleted_at = ?1, updated_at = ?1 WHERE work_id = ?2 AND deleted_at IS NULL",
        params![deleted_at, work_id],
    )?;
    tx.execute(
        "UPDATE launch_profiles SET deleted_at = ?1, updated_at = ?1 WHERE work_id = ?2 AND deleted_at IS NULL",
        params![deleted_at, work_id],
    )?;
    tx.execute(
        "DELETE FROM library_items WHERE work_id = ?1",
        params![work_id],
    )?;
    tx.execute(
        "DELETE FROM work_search_fts WHERE work_id = ?1",
        params![work_id],
    )?;
    Ok(())
}

fn apply_playtime_tx(
    tx: &rusqlite::Transaction<'_>,
    work_id: &str,
    duration_seconds: i64,
    started_at: i64,
    ended_at: i64,
    day: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        r#"
        UPDATE works
        SET total_playtime_seconds = total_playtime_seconds + ?1,
            last_played_at = ?2,
            first_played_at = COALESCE(first_played_at, ?3),
            updated_at = ?2
        WHERE id = ?4
        "#,
        params![duration_seconds, ended_at, started_at, work_id],
    )?;
    tx.execute(
        r#"
        UPDATE library_items
        SET total_playtime_seconds = total_playtime_seconds + ?1,
            last_played_at = ?2,
            status = CASE WHEN status = 'unplayed' THEN 'playing' ELSE status END,
            updated_at = ?2
        WHERE work_id = ?3
        "#,
        params![duration_seconds, ended_at, work_id],
    )?;
    tx.execute(
        r#"
        INSERT INTO playtime_daily (
            id, date, work_id, duration_seconds, session_count,
            first_started_at, last_ended_at, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6, ?6)
        ON CONFLICT(date, work_id) DO UPDATE SET
            duration_seconds = duration_seconds + excluded.duration_seconds,
            session_count = session_count + 1,
            first_started_at = MIN(first_started_at, excluded.first_started_at),
            last_ended_at = MAX(last_ended_at, excluded.last_ended_at),
            updated_at = excluded.updated_at
        "#,
        params![
            Uuid::new_v4().to_string(),
            day,
            work_id,
            duration_seconds,
            started_at,
            ended_at
        ],
    )?;
    Ok(())
}

fn refresh_installation_counts_tx(
    tx: &rusqlite::Transaction<'_>,
    work_id: &str,
    updated_at: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        r#"
        UPDATE library_items
        SET installation_count = (
                SELECT COUNT(*) FROM installations
                WHERE work_id = ?1 AND deleted_at IS NULL
            ),
            available_installation_count = (
                SELECT COUNT(*) FROM installations
                WHERE work_id = ?1 AND deleted_at IS NULL AND is_available = 1
            ),
            updated_at = ?2
        WHERE work_id = ?1
        "#,
        params![work_id, updated_at],
    )?;
    Ok(())
}

fn insert_cover_asset_tx(
    tx: &rusqlite::Transaction<'_>,
    asset: &CoverAsset,
    now: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        r#"
        INSERT INTO assets (
            id, asset_type, local_path, thumbnail_256_path, thumbnail_512_path,
            source, mime_type, size_bytes, hash, created_at, updated_at
        )
        VALUES (?1, 'cover', ?2, ?3, ?4, 'user', ?5, ?6, ?7, ?8, ?8)
        "#,
        params![
            asset.id.as_str(),
            asset.original_path.as_str(),
            asset.thumbnail_256_path.as_str(),
            asset.thumbnail_512_path.as_str(),
            asset.mime_type.as_deref(),
            asset.size_bytes,
            asset.hash.as_deref(),
            now
        ],
    )?;
    Ok(())
}

fn asset_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| {
            value
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "img".to_string())
}

fn mime_type_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

fn file_checksum(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0_u8; 8192];
    let mut hash = 0xcbf29ce484222325_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("{hash:016x}"))
}

fn copy_path(source: &Path, destination: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_destination = destination.join(entry.file_name());
            copy_path(&entry.path(), &child_destination)?;
        }
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn io_to_sql(error: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    fn test_dir(name: &str) -> PathBuf {
        let stamp = Uuid::new_v4();
        let path = std::env::temp_dir().join(format!("luki-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create temp test dir");
        path
    }

    fn seeded_library() -> (PathBuf, Database, LibraryItem) {
        let app_dir = test_dir("app");
        let mut db = Database::open(app_dir.clone()).expect("open database");
        let game_dir = app_dir.join("games").join("Example_VN");
        fs::create_dir_all(&game_dir).expect("create game dir");
        fs::write(game_dir.join("game.sh"), "#!/bin/sh\nexit 0\n").expect("write executable");

        let candidates = db
            .scan_library_root(game_dir.to_str().expect("game dir path"))
            .expect("scan candidates");
        assert_eq!(candidates.len(), 1);
        let item = db
            .accept_import_candidate(&candidates[0].id)
            .expect("accept candidate");
        (app_dir, db, item)
    }

    fn create_game_dir(root: &Path, name: &str) {
        let game_dir = root.join(name);
        fs::create_dir_all(&game_dir).expect("create game dir");
        fs::write(game_dir.join("game.sh"), "#!/bin/sh\nexit 0\n").expect("write executable");
    }

    fn snapshot_count(db: &Database, snapshot_type: &str) -> i64 {
        db.conn
            .query_row(
                "SELECT COUNT(*) FROM snapshots WHERE snapshot_type = ?1",
                params![snapshot_type],
                |row| row.get(0),
            )
            .expect("count snapshots")
    }

    #[test]
    fn scan_import_manual_session_and_status_update_form_mvp_loop() {
        let (_app_dir, mut db, item) = seeded_library();
        assert_eq!(item.display_title, "Example VN");
        assert_eq!(item.installation_count, 1);

        let session = db
            .record_manual_session(&item.work_id, 1800, "推进到共通线结束")
            .expect("record manual session");
        assert_eq!(session.duration_seconds, 1800);
        assert!(session.is_confirmed);

        let updated = db
            .query_library_item(&item.work_id)
            .expect("query library item");
        assert_eq!(updated.status, "playing");
        assert!(updated.total_playtime_seconds >= 1800);

        let completed = db
            .update_work_status(&item.work_id, "completed")
            .expect("update status");
        assert_eq!(completed.status, "completed");
        assert!(completed.completed_at.is_some());

        let edited = db
            .update_work_metadata(
                &item.work_id,
                "Example VN Edited",
                "原名 Example",
                Some(8.5),
                "测试, 共通线",
                "/tmp/example-cover.png",
                2,
                1,
            )
            .expect("update metadata");
        assert_eq!(edited.display_title, "Example VN Edited");
        assert_eq!(edited.original_title.as_deref(), Some("原名 Example"));
        assert_eq!(edited.user_rating, Some(8.5));
        assert_eq!(edited.cover_path.as_deref(), Some("/tmp/example-cover.png"));
        assert!(edited.cover_thumbnail_path.is_none());
        assert_eq!(edited.nsfw_level, 2);
        assert_eq!(edited.privacy_level, 1);

        let searched = db
            .list_library_items("共通线", "", 100, 0)
            .expect("search by tag summary");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].work_id, item.work_id);

        let roots = db.list_library_roots().expect("list library roots");
        assert_eq!(roots.len(), 1);
        assert!(roots[0].last_scanned_at.is_some());

        let scan_jobs = db.list_scan_jobs(10).expect("list scan jobs");
        assert_eq!(scan_jobs.len(), 1);
        assert_eq!(scan_jobs[0].status, "completed");
        assert_eq!(scan_jobs[0].matched_count, 1);
        assert_eq!(scan_jobs[0].failed_count, 0);
    }

    #[test]
    fn existing_cover_file_is_imported_into_managed_assets() {
        let (app_dir, mut db, item) = seeded_library();
        let source_cover = app_dir.join("source-cover.png");
        fs::write(&source_cover, b"not-a-real-png-but-managed-as-a-file").expect("write cover");

        let edited = db
            .update_work_metadata(
                &item.work_id,
                "Managed Cover VN",
                "",
                None,
                "",
                source_cover.to_str().expect("source cover path"),
                0,
                0,
            )
            .expect("update cover metadata");

        let cover_path = PathBuf::from(edited.cover_path.expect("managed cover path"));
        let thumbnail_path =
            PathBuf::from(edited.cover_thumbnail_path.expect("managed thumbnail path"));
        assert!(cover_path.exists());
        assert!(thumbnail_path.exists());
        assert!(cover_path
            .to_string_lossy()
            .contains("assets/originals/covers"));
        assert!(thumbnail_path
            .to_string_lossy()
            .contains("assets/thumbnails/256/covers"));

        let asset_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM assets WHERE asset_type = 'cover'",
                [],
                |row| row.get(0),
            )
            .expect("count assets");
        let link_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM asset_links WHERE entity_type = 'work' AND entity_id = ?1 AND role = 'cover'",
                params![item.work_id],
                |row| row.get(0),
            )
            .expect("count asset links");
        assert_eq!(asset_count, 1);
        assert_eq!(link_count, 1);

        let detail = db.get_work_detail(&item.work_id).expect("query detail");
        assert_eq!(
            detail.item.cover_thumbnail_path,
            Some(thumbnail_path.to_string_lossy().to_string())
        );
    }

    #[test]
    fn data_locations_expose_local_storage_paths() {
        let app_dir = test_dir("data-locations");
        let db = Database::open(app_dir.clone()).expect("open database");
        let locations = db.data_locations();

        assert_eq!(locations.app_dir, app_dir.to_string_lossy().to_string());
        assert_eq!(
            locations.database_path,
            app_dir.join("library.db").to_string_lossy().to_string()
        );
        assert_eq!(
            locations.assets_dir,
            app_dir.join("assets").to_string_lossy().to_string()
        );
        assert_eq!(
            locations.database_snapshots_dir,
            app_dir
                .join("backups")
                .join("snapshots")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(
            locations.save_snapshots_dir,
            app_dir
                .join("saves")
                .join("snapshots")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(
            locations.restore_backups_dir,
            app_dir
                .join("saves")
                .join("restore-backups")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn launch_session_can_be_started_and_finished_with_evidence() {
        let (_app_dir, mut db, item) = seeded_library();
        let target = db.prepare_launch(&item.work_id).expect("prepare launch");
        assert!(target.executable_path.ends_with("game.sh"));

        let receipt = db
            .start_launch_session(&target, Some(42))
            .expect("start launch session");
        assert_eq!(receipt.status, "running");

        let session = db
            .finish_launch_session(&receipt.session_id, Some(0))
            .expect("finish launch session");
        assert_eq!(session.timer_status, "needs_review");
        assert!(!session.is_confirmed);
        assert!(session.duration_seconds >= 1);

        let updated = db
            .query_library_item(&item.work_id)
            .expect("query library item");
        assert_eq!(updated.total_playtime_seconds, 0);

        let pending = db
            .list_unconfirmed_sessions()
            .expect("query unconfirmed sessions");
        assert_eq!(pending.len(), 1);

        let confirmed = db
            .confirm_play_session(&receipt.session_id, 90, "退出后确认")
            .expect("confirm play session");
        assert_eq!(confirmed.timer_status, "completed");
        assert!(confirmed.is_confirmed);
        assert_eq!(confirmed.duration_seconds, 90);

        let updated = db
            .query_library_item(&item.work_id)
            .expect("query library item");
        assert_eq!(updated.total_playtime_seconds, 90);
        assert!(db
            .list_unconfirmed_sessions()
            .expect("query unconfirmed sessions after confirm")
            .is_empty());
    }

    #[test]
    fn default_launch_profile_can_be_relinked() {
        let (app_dir, mut db, item) = seeded_library();
        let new_dir = app_dir.join("games").join("Example_VN_Relocated");
        fs::create_dir_all(&new_dir).expect("create relocated game dir");
        let new_executable = new_dir.join("start.sh");
        fs::write(&new_executable, "#!/bin/sh\nexit 0\n").expect("write relocated executable");

        let profile = db
            .upsert_default_launch_profile(
                &item.work_id,
                "修复后的启动",
                new_executable.to_str().expect("new executable path"),
                new_dir.to_str().expect("new working dir"),
                "--windowed",
            )
            .expect("update launch profile");
        assert_eq!(profile.name, "修复后的启动");
        assert!(profile.is_default);
        assert!(profile.is_available);
        assert_eq!(profile.arguments.as_deref(), Some("--windowed"));

        let target = db
            .prepare_launch(&item.work_id)
            .expect("prepare launch after relink");
        assert_eq!(
            target.executable_path,
            new_executable.to_string_lossy().to_string()
        );
        assert_eq!(target.arguments.as_deref(), Some("--windowed"));

        let detail = db.get_work_detail(&item.work_id).expect("query detail");
        assert_eq!(detail.launch_profiles.len(), 1);
        assert_eq!(detail.launch_profiles[0].name, "修复后的启动");
        assert_eq!(detail.item.available_installation_count, 1);
    }

    #[test]
    fn pending_launch_receipt_can_be_discarded_without_playtime() {
        let (_app_dir, mut db, item) = seeded_library();
        let target = db.prepare_launch(&item.work_id).expect("prepare launch");
        let receipt = db
            .start_launch_session(&target, Some(42))
            .expect("start launch session");
        db.finish_launch_session(&receipt.session_id, Some(0))
            .expect("finish launch session");

        let discarded = db
            .discard_play_session(&receipt.session_id, "误启动")
            .expect("discard play session");
        assert_eq!(discarded.affected_count, 1);

        let updated = db
            .query_library_item(&item.work_id)
            .expect("query library item");
        assert_eq!(updated.total_playtime_seconds, 0);
        assert!(db
            .list_unconfirmed_sessions()
            .expect("query unconfirmed sessions after discard")
            .is_empty());
    }

    #[test]
    fn save_profile_snapshot_restore_and_database_snapshot_work() {
        let (app_dir, mut db, item) = seeded_library();
        let save_dir = app_dir.join("saves-src");
        fs::create_dir_all(&save_dir).expect("create save dir");
        fs::write(save_dir.join("slot01.dat"), "before").expect("write save file");

        let profile = db
            .create_save_profile(
                &item.work_id,
                "默认存档",
                save_dir.to_str().expect("save dir path"),
            )
            .expect("create save profile");
        assert_eq!(profile.name, "默认存档");

        let save_snapshot = db
            .create_save_snapshot(
                &item.work_id,
                Some(&profile.id),
                "",
                "共通线结束",
                "共通线",
                "分歧前",
            )
            .expect("create save snapshot");
        assert!(PathBuf::from(&save_snapshot.snapshot_path)
            .join("slot01.dat")
            .exists());

        fs::write(save_dir.join("slot01.dat"), "after").expect("mutate save file");
        db.restore_save_snapshot(&save_snapshot.id, save_dir.to_str().expect("save dir path"))
            .expect("restore save snapshot");
        let restored = fs::read_to_string(save_dir.join("slot01.dat")).expect("read restored");
        assert_eq!(restored, "before");

        let db_snapshot = db.create_snapshot("测试快照").expect("create db snapshot");
        assert!(PathBuf::from(db_snapshot.path).exists());

        let detail = db.get_work_detail(&item.work_id).expect("query detail");
        assert_eq!(detail.save_profiles.len(), 1);
        assert_eq!(detail.save_snapshots.len(), 1);
    }

    #[test]
    fn open_creates_one_daily_snapshot_per_day_when_library_has_data() {
        let empty_app_dir = test_dir("empty-daily-snapshot");
        let empty_db = Database::open(empty_app_dir).expect("open empty database");
        assert_eq!(snapshot_count(&empty_db, "daily"), 0);
        drop(empty_db);

        let (app_dir, db, _item) = seeded_library();
        assert_eq!(snapshot_count(&db, "daily"), 0);
        drop(db);

        let reopened = Database::open(app_dir.clone()).expect("reopen populated database");
        assert_eq!(snapshot_count(&reopened, "daily"), 1);
        let daily = reopened
            .query_last_snapshot()
            .expect("query last snapshot")
            .expect("daily snapshot exists");
        assert_eq!(daily.snapshot_type, "daily");
        assert_eq!(daily.reason.as_deref(), Some("每日自动快照"));
        assert!(PathBuf::from(&daily.path).exists());
        drop(reopened);

        let mut reopened = Database::open(app_dir).expect("reopen same day");
        assert_eq!(snapshot_count(&reopened, "daily"), 1);

        let yesterday = now_ms() - 86_400_000;
        reopened
            .conn
            .execute(
                "UPDATE snapshots SET created_at = ?1 WHERE snapshot_type = 'daily'",
                params![yesterday],
            )
            .expect("move daily snapshot to previous day");
        let next_daily = reopened
            .ensure_daily_snapshot()
            .expect("ensure next daily snapshot")
            .expect("new daily snapshot");
        assert_eq!(next_daily.snapshot_type, "daily");
        assert_eq!(snapshot_count(&reopened, "daily"), 2);
    }

    #[test]
    fn open_creates_before_migration_snapshot_for_existing_unversioned_database() {
        let app_dir = test_dir("migration-snapshot");
        let db_path = app_dir.join("library.db");
        let legacy = Connection::open(&db_path).expect("open legacy database");
        legacy
            .execute_batch(
                r#"
                CREATE TABLE legacy_marker (
                    id TEXT PRIMARY KEY
                );
                INSERT INTO legacy_marker (id) VALUES ('legacy-data');
                PRAGMA user_version = 0;
                "#,
            )
            .expect("seed legacy database");
        drop(legacy);

        let db = Database::open(app_dir.clone()).expect("open migrated database");
        assert_eq!(
            db.schema_version().expect("query schema version"),
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(snapshot_count(&db, "before_migration"), 1);
        let snapshot = db
            .query_last_snapshot()
            .expect("query migration snapshot")
            .expect("migration snapshot exists");
        assert_eq!(snapshot.snapshot_type, "before_migration");
        assert_eq!(snapshot.reason.as_deref(), Some("迁移前快照"));
        assert!(PathBuf::from(&snapshot.path).exists());
        drop(db);

        let db = Database::open(app_dir).expect("reopen migrated database");
        assert_eq!(snapshot_count(&db, "before_migration"), 1);
    }

    #[test]
    fn save_snapshot_can_be_locked_unlocked_and_deleted() {
        let (app_dir, mut db, item) = seeded_library();
        let save_dir = app_dir.join("lock-save-src");
        fs::create_dir_all(&save_dir).expect("create save dir");
        fs::write(save_dir.join("slot01.dat"), "snapshot").expect("write save file");
        let profile = db
            .create_save_profile(
                &item.work_id,
                "默认存档",
                save_dir.to_str().expect("save dir path"),
            )
            .expect("create save profile");
        let snapshot = db
            .create_save_snapshot(
                &item.work_id,
                Some(&profile.id),
                "",
                "重要节点",
                "共通线",
                "分歧前",
            )
            .expect("create save snapshot");
        let snapshot_root = PathBuf::from(&snapshot.snapshot_path)
            .parent()
            .expect("snapshot root")
            .to_path_buf();
        assert!(snapshot_root.exists());

        let locked = db
            .set_save_snapshot_locked(&snapshot.id, true)
            .expect("lock save snapshot");
        assert!(locked.is_locked);
        assert!(db.delete_save_snapshot(&snapshot.id).is_err());
        assert!(snapshot_root.exists());

        let unlocked = db
            .set_save_snapshot_locked(&snapshot.id, false)
            .expect("unlock save snapshot");
        assert!(!unlocked.is_locked);
        let deleted = db
            .delete_save_snapshot(&snapshot.id)
            .expect("delete save snapshot");
        assert_eq!(deleted.affected_count, 1);
        assert!(!snapshot_root.exists());

        let detail = db.get_work_detail(&item.work_id).expect("query detail");
        assert!(detail.save_snapshots.is_empty());
        assert!(db
            .list_operations(20)
            .expect("list operations")
            .iter()
            .any(|operation| operation.summary == "删除存档快照"));
    }

    #[test]
    fn path_health_check_updates_installation_cache() {
        let (_app_dir, mut db, item) = seeded_library();
        let target = db.prepare_launch(&item.work_id).expect("prepare launch");
        fs::remove_file(&target.executable_path).expect("remove executable");

        let report = db.check_path_health().expect("check path health");
        assert_eq!(report.checked_count, 1);
        assert_eq!(report.available_count, 0);
        assert_eq!(report.missing_count, 1);
        assert_eq!(report.changed_count, 1);
        assert_eq!(report.issues.len(), 1);

        let unavailable = db
            .query_library_item(&item.work_id)
            .expect("query library item");
        assert_eq!(unavailable.available_installation_count, 0);
        assert!(db
            .list_operations(20)
            .expect("list operations")
            .iter()
            .any(|operation| operation.summary == "检查路径健康"));

        fs::write(&target.executable_path, "#!/bin/sh\nexit 0\n").expect("restore executable");
        let report = db.check_path_health().expect("check path health again");
        assert_eq!(report.available_count, 1);
        assert_eq!(report.missing_count, 0);
        assert_eq!(report.changed_count, 1);

        let available = db
            .query_library_item(&item.work_id)
            .expect("query library item");
        assert_eq!(available.available_installation_count, 1);
    }

    #[test]
    fn large_library_paginates_and_searches_from_cache() {
        let app_dir = test_dir("large-library");
        let mut db = Database::open(app_dir).expect("open database");
        let now = now_ms();
        let tx = db.conn.transaction().expect("start large seed transaction");
        for index in 0..1_250 {
            let work_id = format!("large-{index:04}");
            let title = format!("Large VN {index:04}");
            let status = if index % 2 == 0 {
                "playing"
            } else {
                "unplayed"
            };
            let tags = if index == 777 {
                "needle special route"
            } else {
                "bulk test"
            };
            tx.execute(
                r#"
                INSERT INTO library_items (
                    work_id, display_title, original_title, sort_title, status,
                    total_playtime_seconds, version_count, installation_count,
                    available_installation_count, tag_summary, updated_at
                )
                VALUES (?1, ?2, NULL, ?2, ?3, 0, 1, 1, 1, ?4, ?5)
                "#,
                params![work_id, title, status, tags, now + index],
            )
            .expect("insert library item");
            tx.execute(
                r#"
                INSERT INTO work_search_fts (
                    work_id, title, original_title, aliases, developer, publisher, tags, description
                )
                VALUES (?1, ?2, '', '', '', '', ?3, '')
                "#,
                params![work_id, title, tags],
            )
            .expect("insert search item");
        }
        tx.commit().expect("commit large seed");

        let first_page = db
            .list_library_items("", "", 120, 0)
            .expect("query first page");
        let second_page = db
            .list_library_items("", "", 120, 120)
            .expect("query second page");
        assert_eq!(first_page.len(), 120);
        assert_eq!(second_page.len(), 120);
        assert_ne!(first_page[0].work_id, second_page[0].work_id);

        let playing = db
            .list_library_items("", "playing", 130, 0)
            .expect("query status page");
        assert_eq!(playing.len(), 130);
        assert!(playing.iter().all(|item| item.status == "playing"));

        let searched = db
            .list_library_items("needle", "", 20, 0)
            .expect("search large library");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].work_id, "large-0777");

        let virtual_page = db
            .list_library_page("", "", 240, 480)
            .expect("query virtual shelf page");
        assert_eq!(virtual_page.items.len(), 240);
        assert_eq!(virtual_page.total, 1_250);
        assert_eq!(virtual_page.offset, 480);

        let searched_page = db
            .list_library_page("needle", "", 20, 0)
            .expect("query virtual search page");
        assert_eq!(searched_page.total, 1);
        assert_eq!(searched_page.items[0].work_id, "large-0777");
    }

    #[test]
    fn work_detail_mixes_notes_saves_and_sessions_into_timeline() {
        let (app_dir, mut db, item) = seeded_library();
        let session = db
            .record_manual_session(&item.work_id, 900, "短暂推进")
            .expect("record manual session");
        assert_eq!(session.timer_status, "completed");

        let note = db
            .create_note(&item.work_id, "分歧点", "下次从这里继续。", "note", 1, 0)
            .expect("create note");
        assert_eq!(note.title.as_deref(), Some("分歧点"));

        let save_dir = app_dir.join("timeline-save-src");
        fs::create_dir_all(&save_dir).expect("create save dir");
        fs::write(save_dir.join("slot01.dat"), "snapshot").expect("write save file");
        let profile = db
            .create_save_profile(
                &item.work_id,
                "默认存档",
                save_dir.to_str().expect("save dir path"),
            )
            .expect("create save profile");
        db.create_save_snapshot(
            &item.work_id,
            Some(&profile.id),
            "",
            "快照备注",
            "共通线",
            "分歧前",
        )
        .expect("create save snapshot");

        let detail = db.get_work_detail(&item.work_id).expect("query detail");
        assert!(detail
            .timeline_events
            .iter()
            .any(|event| event.event_type == "play_session"));
        assert!(detail
            .timeline_events
            .iter()
            .any(|event| event.event_type == "save_snapshot"));
        assert!(detail
            .timeline_events
            .iter()
            .any(|event| event.event_type == "note" && event.spoiler_level == 1));
        assert!(detail
            .timeline_events
            .windows(2)
            .all(|pair| pair[0].occurred_at >= pair[1].occurred_at));
    }

    #[test]
    fn operation_history_is_queryable_for_safety_center_and_work_detail() {
        let (_app_dir, mut db, item) = seeded_library();
        db.update_work_metadata(
            &item.work_id,
            "Operation History VN",
            "",
            Some(7.0),
            "历史",
            "",
            0,
            0,
        )
        .expect("update metadata");
        db.create_note(
            &item.work_id,
            "记录点",
            "需要能在操作历史看到。",
            "note",
            0,
            0,
        )
        .expect("create note");

        let operations = db.list_operations(20).expect("list operations");
        assert!(operations
            .iter()
            .any(|operation| operation.summary == "更新作品资料"));
        assert!(operations
            .iter()
            .any(|operation| operation.summary == "创建笔记"));

        let detail = db.get_work_detail(&item.work_id).expect("query detail");
        assert!(detail
            .recent_operations
            .iter()
            .any(|operation| operation.summary == "更新作品资料"));
        assert!(detail
            .recent_operations
            .iter()
            .any(|operation| operation.summary == "创建笔记"));
    }

    #[test]
    fn bulk_import_creates_snapshot_and_can_be_undone() {
        let app_dir = test_dir("bulk-app");
        let mut db = Database::open(app_dir.clone()).expect("open database");
        let games_root = app_dir.join("bulk-games");
        create_game_dir(&games_root, "First_VN");
        create_game_dir(&games_root, "Second_VN");

        let candidates = db
            .scan_library_root(games_root.to_str().expect("games root path"))
            .expect("scan candidates");
        assert_eq!(candidates.len(), 2);

        let result = db
            .accept_pending_import_candidates()
            .expect("bulk import candidates");
        assert_eq!(result.imported_count, 2);
        assert_eq!(result.snapshot.snapshot_type, "before_bulk_import");
        assert!(PathBuf::from(result.snapshot.path).exists());

        let library = db
            .list_library_items("", "", 100, 0)
            .expect("query library");
        assert_eq!(library.len(), 2);

        let undo = db
            .undo_latest_bulk_import()
            .expect("undo latest bulk import");
        assert_eq!(undo.affected_work_count, 2);
        assert_eq!(undo.restored_candidate_count, 2);

        let library = db
            .list_library_items("", "", 100, 0)
            .expect("query library after undo");
        assert!(library.is_empty());
        let candidates = db
            .list_import_candidates()
            .expect("query candidates after undo");
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.status == "pending")
                .count(),
            2
        );
    }

    #[test]
    fn soft_delete_removes_work_from_library_cache() {
        let (_app_dir, mut db, item) = seeded_library();
        let deleted = db
            .soft_delete_work(&item.work_id)
            .expect("soft delete work");
        assert_eq!(deleted.affected_count, 1);

        let library = db
            .list_library_items("", "", 100, 0)
            .expect("query library");
        assert!(library.is_empty());
        let detail = db.get_work_detail(&item.work_id);
        assert!(detail.is_err());
    }
}
