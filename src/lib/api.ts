import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import type {
  BulkImportResult,
  DataLocations,
  ImportCandidate,
  LaunchProfile,
  LibraryPage,
  LibraryItem,
  LibraryRoot,
  LaunchReceipt,
  MutationResult,
  NoteItem,
  OperationItem,
  PathHealthReport,
  PlaySession,
  SaveProfile,
  SaveSnapshot,
  ScanJob,
  SnapshotInfo,
  TodayDesk,
  UndoResult,
  WorkDetail
} from './types';

const isTauri =
  typeof window !== 'undefined' &&
  '__TAURI_INTERNALS__' in (window as unknown as Record<string, unknown>);

const demoItems: LibraryItem[] = [
  {
    work_id: 'demo-1',
    display_title: 'Summer Pockets',
    original_title: 'サマーポケッツ',
    status: 'playing',
    user_rating: 9,
    cover_path: null,
    cover_thumbnail_path: null,
    nsfw_level: 0,
    privacy_level: 0,
    total_playtime_seconds: 18420,
    last_played_at: Date.now() - 86400000,
    completed_at: null,
    version_count: 1,
    installation_count: 1,
    available_installation_count: 1,
    tag_summary: 'Key, 夏日, 共通线',
    updated_at: Date.now()
  },
  {
    work_id: 'demo-2',
    display_title: '白色相簿 2',
    original_title: 'WHITE ALBUM2',
    status: 'paused',
    user_rating: 9.5,
    cover_path: null,
    cover_thumbnail_path: null,
    nsfw_level: 2,
    privacy_level: 1,
    total_playtime_seconds: 42100,
    last_played_at: Date.now() - 604800000,
    completed_at: null,
    version_count: 2,
    installation_count: 1,
    available_installation_count: 1,
    tag_summary: '冬日, 音乐, 情感',
    updated_at: Date.now()
  }
];

for (let index = 3; index <= 180; index += 1) {
  demoItems.push({
    work_id: `demo-${index}`,
    display_title: `Demo VN ${index.toString().padStart(3, '0')}`,
    original_title: index % 3 === 0 ? `デモ作品 ${index}` : null,
    status: index % 5 === 0 ? 'completed' : index % 2 === 0 ? 'playing' : 'unplayed',
    user_rating: index % 4 === 0 ? 8 + (index % 10) / 10 : null,
    cover_path: null,
    cover_thumbnail_path: null,
    nsfw_level: index % 11 === 0 ? 2 : 0,
    privacy_level: index % 19 === 0 ? 1 : 0,
    total_playtime_seconds: index * 420,
    last_played_at: Date.now() - index * 43_200_000,
    completed_at: index % 5 === 0 ? Date.now() - index * 86_400_000 : null,
    version_count: 1,
    installation_count: 1,
    available_installation_count: index % 17 === 0 ? 0 : 1,
    tag_summary: index % 2 === 0 ? 'demo, virtual shelf' : 'demo',
    updated_at: Date.now() - index * 60_000
  });
}

function filterDemoItems(search = '', status = '') {
  const normalized = search.trim().toLowerCase();
  return demoItems.filter((item) => {
    const matchesSearch =
      normalized.length === 0 ||
      item.display_title.toLowerCase().includes(normalized) ||
      item.original_title?.toLowerCase().includes(normalized) ||
      item.tag_summary?.toLowerCase().includes(normalized);
    const matchesStatus = !status || item.status === status;
    return matchesSearch && matchesStatus;
  });
}

const demoCandidates: ImportCandidate[] = [
  {
    id: 'candidate-demo',
    path: '/games/Example VN',
    candidate_type: 'folder',
    detected_title: 'Example VN',
    detected_executable: 'game.exe',
    size_bytes: 0,
    file_count: 12,
    confidence: 0.62,
    status: 'pending',
    matched_work_id: null,
    evidence_json: '{"reason":"demo"}',
    created_at: Date.now(),
    updated_at: Date.now()
  }
];

let demoPendingReceipts: PlaySession[] = [
  {
    id: 'receipt-demo',
    work_id: demoItems[0].work_id,
    title: demoItems[0].display_title,
    started_at: Date.now() - 5400000,
    ended_at: Date.now() - 1800000,
    duration_seconds: 3600,
    source: 'auto',
    confidence: 0.9,
    is_confirmed: false,
    timer_status: 'needs_review',
    note: null
  }
];

let demoScanJobs: ScanJob[] = [
  {
    id: 'scan-demo',
    root_id: 'root-demo',
    root_path: '/games',
    status: 'completed',
    started_at: Date.now() - 300000,
    finished_at: Date.now() - 299000,
    total_count: demoCandidates.length,
    matched_count: demoCandidates.length,
    failed_count: 0,
    log_json: '{"detector":"folder-executable-scan","candidate_count":1}',
    created_at: Date.now() - 300000,
    updated_at: Date.now() - 299000
  }
];

let demoLibraryRoots: LibraryRoot[] = [
  {
    id: 'root-demo',
    name: 'games',
    path: '/games',
    root_type: 'games',
    recursive: true,
    is_active: true,
    last_scanned_at: Date.now() - 299000,
    created_at: Date.now() - 300000,
    updated_at: Date.now() - 299000
  }
];

let demoOperations: OperationItem[] = [
  {
    id: 'operation-demo-launch',
    entity_type: 'launch_profile',
    entity_id: 'launch-demo-1',
    operation_type: 'upsert_default',
    summary: '更新默认启动方式',
    payload_json: '{"work_id":"demo-1","name":"默认启动"}',
    created_at: Date.now() - 120000,
    synced_at: null,
    is_synced: false
  },
  {
    id: 'operation-demo-note',
    entity_type: 'note',
    entity_id: 'note-demo',
    operation_type: 'create',
    summary: '创建笔记',
    payload_json: '{"work_id":"demo-1","note_type":"note"}',
    created_at: Date.now() - 240000,
    synced_at: null,
    is_synced: false
  },
  {
    id: 'operation-demo-import',
    entity_type: 'library',
    entity_id: 'bulk_import',
    operation_type: 'bulk_import',
    summary: '批量导入作品',
    payload_json: '{"work_ids":["demo-1","demo-2"]}',
    created_at: Date.now() - 3600000,
    synced_at: null,
    is_synced: false
  }
];

function appendDemoOperation(
  operation: Omit<OperationItem, 'id' | 'created_at' | 'synced_at' | 'is_synced'>
) {
  demoOperations = [
    {
      ...operation,
      id: `operation-${Date.now()}`,
      created_at: Date.now(),
      synced_at: null,
      is_synced: false
    },
    ...demoOperations
  ].slice(0, 30);
}

async function call<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (!isTauri) {
    if (fallback === undefined) {
      throw new Error(`No browser fallback for ${command}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 120));
    return fallback;
  }
  return invoke<T>(command, args);
}

export const api = {
  getTodayDesk() {
    const fallback: TodayDesk = {
      continue_items: demoItems.map((item) => ({
        ...item,
        default_launch_name: '默认启动',
        latest_save_label: '共通线结束'
      })),
      pending_import_count: demoCandidates.length,
      unconfirmed_session_count: demoPendingReceipts.length,
      month: {
        duration_seconds: 60520,
        active_days: 7,
        completed_count: 1,
        most_played_title: '白色相簿 2'
      },
      recent_items: demoItems,
      last_snapshot: {
        id: 'snapshot-demo',
        snapshot_type: 'daily',
        reason: '每日快照',
        path: 'backups/snapshots/demo.zip',
        created_at: Date.now() - 3600000
      }
    };
    return call<TodayDesk>('get_today_desk', undefined, fallback);
  },

  getDataLocations() {
    return call<DataLocations>('get_data_locations', undefined, {
      app_dir: '/home/user/.local/share/luki',
      database_path: '/home/user/.local/share/luki/library.db',
      assets_dir: '/home/user/.local/share/luki/assets',
      database_snapshots_dir: '/home/user/.local/share/luki/backups/snapshots',
      save_snapshots_dir: '/home/user/.local/share/luki/saves/snapshots',
      restore_backups_dir: '/home/user/.local/share/luki/saves/restore-backups'
    });
  },

  listLibraryItems(search = '', status = '', limit = 121, offset = 0) {
    const fallback = filterDemoItems(search, status).slice(offset, offset + limit);
    return call<LibraryItem[]>('list_library_items', { search, status, limit, offset }, fallback);
  },

  listLibraryPage(search = '', status = '', limit = 240, offset = 0) {
    const filtered = filterDemoItems(search, status);
    const fallback: LibraryPage = {
      items: filtered.slice(offset, offset + limit),
      total: filtered.length,
      limit,
      offset
    };
    return call<LibraryPage>('list_library_page', { search, status, limit, offset }, fallback);
  },

  getWorkDetail(workId: string) {
    const item = demoItems.find((candidate) => candidate.work_id === workId) ?? demoItems[0];
    const fallback: WorkDetail = {
      item,
      launch_profiles: [
        {
          id: `launch-${item.work_id}`,
          work_id: item.work_id,
          installation_id: `installation-${item.work_id}`,
          name: '默认启动',
          executable_path: `/games/${item.display_title}/game.exe`,
          working_dir: `/games/${item.display_title}`,
          arguments: null,
          is_default: true,
          is_available: true,
          created_at: item.updated_at,
          updated_at: item.updated_at
        }
      ],
      recent_sessions: [
        {
          id: 'session-demo',
          work_id: item.work_id,
          title: item.display_title,
          started_at: Date.now() - 7200000,
          ended_at: Date.now() - 3600000,
          duration_seconds: 3600,
          source: 'auto',
          confidence: 0.92,
          is_confirmed: true,
          timer_status: 'completed',
          note: '这次推进到一个适合建快照的位置。'
        }
      ],
      save_profiles: [
        {
          id: 'save-profile-demo',
          work_id: item.work_id,
          name: '默认存档',
          save_path: '/games/Example VN/save',
          engine: null,
          strategy: 'copy',
          is_active: true,
          created_at: Date.now() - 7200000,
          updated_at: Date.now() - 7200000
        }
      ],
      save_snapshots: [
        {
          id: 'save-snapshot-demo',
          work_id: item.work_id,
          save_profile_id: 'save-profile-demo',
          snapshot_path: '/app/saves/demo/files',
          note: '共通线结束',
          route_name: '共通线',
          progress_label: '分歧前',
          is_locked: false,
          created_at: Date.now() - 5400000,
          updated_at: Date.now() - 5400000
        }
      ],
      notes: [
        {
          id: 'note-demo',
          title: '分歧点',
          content: '下次从这里继续前先创建存档快照。',
          note_type: 'note',
          spoiler_level: 1,
          privacy_level: 0,
          created_at: Date.now() - 3600000,
          updated_at: Date.now() - 3600000
        }
      ],
      timeline_events: [
        {
          id: 'play_session:session-demo',
          work_id: item.work_id,
          event_type: 'play_session',
          source_id: 'session-demo',
          occurred_at: Date.now() - 3600000,
          title: '游玩 1 小时 0 分钟',
          summary: '这次推进到一个适合建快照的位置。',
          detail: 'auto · 已确认 · 可信度 92%',
          duration_seconds: 3600,
          privacy_level: 0,
          spoiler_level: 0
        },
        {
          id: 'save_snapshot:save-snapshot-demo',
          work_id: item.work_id,
          event_type: 'save_snapshot',
          source_id: 'save-snapshot-demo',
          occurred_at: Date.now() - 5400000,
          title: '分歧前',
          summary: '共通线',
          detail: '/app/saves/demo/files',
          duration_seconds: null,
          privacy_level: 0,
          spoiler_level: 0
        },
        {
          id: 'note:note-demo',
          work_id: item.work_id,
          event_type: 'note',
          source_id: 'note-demo',
          occurred_at: Date.now() - 3600000,
          title: '分歧点',
          summary: '下次从这里继续前先创建存档快照。',
          detail: '下次从这里继续前先创建存档快照。',
          duration_seconds: null,
          privacy_level: 0,
          spoiler_level: 1
        }
      ],
      recent_operations: demoOperations.filter(
        (operation) =>
          operation.entity_id === item.work_id || operation.payload_json.includes(item.work_id)
      )
    };
    return call<WorkDetail>('get_work_detail', { workId }, fallback);
  },

  listOperations(limit = 30) {
    return call<OperationItem[]>('list_operations', { limit }, demoOperations.slice(0, limit));
  },

  checkPathHealth() {
    appendDemoOperation({
      entity_type: 'library',
      entity_id: 'path_health',
      operation_type: 'path_health_check',
      summary: '检查路径健康',
      payload_json: JSON.stringify({
        checked_count: demoItems.length,
        available_count: demoItems.length,
        missing_count: 0,
        changed_count: 0
      })
    });
    return call<PathHealthReport>('check_path_health', undefined, {
      checked_count: demoItems.length,
      available_count: demoItems.length,
      missing_count: 0,
      changed_count: 0,
      issues: [],
      operation_id: `path-health-${Date.now()}`,
      checked_at: Date.now()
    });
  },

  listImportCandidates() {
    return call<ImportCandidate[]>('list_import_candidates', undefined, demoCandidates);
  },

  listLibraryRoots() {
    return call<LibraryRoot[]>('list_library_roots', undefined, demoLibraryRoots);
  },

  listScanJobs(limit = 10) {
    return call<ScanJob[]>('list_scan_jobs', { limit }, demoScanJobs.slice(0, limit));
  },

  scanLibraryRoot(path: string) {
    const now = Date.now();
    const root: LibraryRoot = {
      id: `root-${now}`,
      name: path.split(/[\\/]/).filter(Boolean).at(-1) ?? path,
      path,
      root_type: 'games',
      recursive: true,
      is_active: true,
      last_scanned_at: now,
      created_at: now,
      updated_at: now
    };
    demoLibraryRoots = [root, ...demoLibraryRoots.filter((item) => item.path !== path)].slice(0, 10);
    demoScanJobs = [
      {
        id: `scan-${now}`,
        root_id: root.id,
        root_path: path,
        status: 'completed',
        started_at: now - 500,
        finished_at: now,
        total_count: demoCandidates.length,
        matched_count: demoCandidates.length,
        failed_count: 0,
        log_json: JSON.stringify({ detector: 'browser-demo', candidate_count: demoCandidates.length }),
        created_at: now - 500,
        updated_at: now
      },
      ...demoScanJobs
    ].slice(0, 10);
    return call<ImportCandidate[]>('scan_library_root', { path }, demoCandidates);
  },

  acceptImportCandidate(candidateId: string) {
    return call<LibraryItem>('accept_import_candidate', { candidateId }, demoItems[0]);
  },

  acceptPendingImportCandidates() {
    appendDemoOperation({
      entity_type: 'library',
      entity_id: 'bulk_import',
      operation_type: 'bulk_import',
      summary: '批量导入作品',
      payload_json: JSON.stringify({ work_ids: demoItems.map((item) => item.work_id) })
    });
    return call<BulkImportResult>('accept_pending_import_candidates', undefined, {
      imported_items: demoItems,
      imported_count: demoItems.length,
      snapshot: {
        id: `snapshot-${Date.now()}`,
        snapshot_type: 'before_bulk_import',
        reason: '批量导入前快照',
        path: 'backups/snapshots/bulk-demo.db',
        created_at: Date.now()
      },
      operation_id: `bulk-${Date.now()}`
    });
  },

  undoLatestBulkImport() {
    return call<UndoResult>('undo_latest_bulk_import', undefined, {
      operation_id: `undo-${Date.now()}`,
      restored_candidate_count: demoCandidates.length,
      affected_work_count: demoItems.length
    });
  },

  softDeleteWork(workId: string) {
    return call<MutationResult>('soft_delete_work', { workId }, {
      operation_id: `delete-${Date.now()}`,
      affected_count: 1,
      message: '作品已移出书架'
    });
  },

  updateWorkStatus(workId: string, nextStatus: string) {
    return call<LibraryItem>('update_work_status', { workId, status: nextStatus }, {
      ...(demoItems.find((item) => item.work_id === workId) ?? demoItems[0]),
      status: nextStatus as LibraryItem['status'],
      completed_at: nextStatus === 'completed' ? Date.now() : null,
      updated_at: Date.now()
    });
  },

  updateWorkMetadata(
    workId: string,
    title: string,
    originalTitle: string,
    userRating: number | null,
    tagSummary: string,
    coverPath: string,
    nsfwLevel = 0,
    privacyLevel = 0
  ) {
    const item = demoItems.find((candidate) => candidate.work_id === workId) ?? demoItems[0];
    item.display_title = title;
    item.original_title = originalTitle || null;
    item.user_rating = userRating;
    item.tag_summary = tagSummary || null;
    item.cover_path = coverPath || null;
    item.cover_thumbnail_path = null;
    item.nsfw_level = nsfwLevel;
    item.privacy_level = privacyLevel;
    item.updated_at = Date.now();
    return call<LibraryItem>(
      'update_work_metadata',
      { workId, title, originalTitle, userRating, tagSummary, coverPath, nsfwLevel, privacyLevel },
      {
        ...item,
      }
    );
  },

  launchWork(workId: string) {
    const item = demoItems.find((candidate) => candidate.work_id === workId) ?? demoItems[0];
    return call<LaunchReceipt>('launch_work', { workId }, {
      session_id: `launch-${Date.now()}`,
      work_id: workId,
      title: item.display_title,
      launch_profile_name: '默认启动',
      executable_path: '/games/demo/game.exe',
      process_id: 10001,
      started_at: Date.now(),
      status: 'running'
    });
  },

  upsertDefaultLaunchProfile(
    workId: string,
    name: string,
    executablePath: string,
    workingDir: string,
    argumentsText: string
  ) {
    appendDemoOperation({
      entity_type: 'launch_profile',
      entity_id: `launch-${workId}`,
      operation_type: 'upsert_default',
      summary: '更新默认启动方式',
      payload_json: JSON.stringify({ work_id: workId, name: name || '默认启动' })
    });
    return call<LaunchProfile>(
      'upsert_default_launch_profile',
      { workId, name, executablePath, workingDir, arguments: argumentsText },
      {
        id: `launch-${workId}`,
        work_id: workId,
        installation_id: `installation-${workId}`,
        name: name || '默认启动',
        executable_path: executablePath,
        working_dir: workingDir || null,
        arguments: argumentsText || null,
        is_default: true,
        is_available: true,
        created_at: Date.now(),
        updated_at: Date.now()
      }
    );
  },

  recordManualSession(workId: string, durationSeconds: number, note: string) {
    return call<PlaySession>('record_manual_session', { workId, durationSeconds, note }, {
      id: `manual-${Date.now()}`,
      work_id: workId,
      title: demoItems.find((item) => item.work_id === workId)?.display_title ?? '作品',
      started_at: Date.now() - durationSeconds * 1000,
      ended_at: Date.now(),
      duration_seconds: durationSeconds,
      source: 'manual',
      confidence: 1,
      is_confirmed: true,
      timer_status: 'completed',
      note
    });
  },

  createNote(
    workId: string,
    title: string,
    content: string,
    noteType: string,
    spoilerLevel: number,
    privacyLevel: number
  ) {
    appendDemoOperation({
      entity_type: 'note',
      entity_id: `note-${Date.now()}`,
      operation_type: 'create',
      summary: '创建笔记',
      payload_json: JSON.stringify({ work_id: workId, note_type: noteType || 'note' })
    });
    return call<NoteItem>(
      'create_note',
      { workId, title, content, noteType, spoilerLevel, privacyLevel },
      {
        id: `note-${Date.now()}`,
        title: title || null,
        content,
        note_type: noteType || 'note',
        spoiler_level: spoilerLevel,
        privacy_level: privacyLevel,
        created_at: Date.now(),
        updated_at: Date.now()
      }
    );
  },

  listUnconfirmedSessions() {
    return call<PlaySession[]>('list_unconfirmed_sessions', undefined, demoPendingReceipts);
  },

  confirmPlaySession(sessionId: string, durationSeconds: number, note: string) {
    const receipt = demoPendingReceipts.find((session) => session.id === sessionId);
    if (receipt) {
      demoPendingReceipts = demoPendingReceipts.filter((session) => session.id !== sessionId);
      const item = demoItems.find((candidate) => candidate.work_id === receipt.work_id);
      if (item) {
        item.total_playtime_seconds += durationSeconds;
        item.last_played_at = Date.now();
        if (item.status === 'unplayed') item.status = 'playing';
      }
    }
    appendDemoOperation({
      entity_type: 'play_session',
      entity_id: sessionId,
      operation_type: 'confirm',
      summary: '确认游玩回执',
      payload_json: JSON.stringify({ work_id: receipt?.work_id ?? demoItems[0].work_id, duration_seconds: durationSeconds })
    });
    return call<PlaySession>(
      'confirm_play_session',
      { sessionId, durationSeconds, note },
      {
        id: sessionId,
        work_id: receipt?.work_id ?? demoItems[0].work_id,
        title: receipt?.title ?? demoItems[0].display_title,
        started_at: receipt?.started_at ?? Date.now() - durationSeconds * 1000,
        ended_at: receipt?.ended_at ?? Date.now(),
        duration_seconds: durationSeconds,
        source: 'auto',
        confidence: 0.9,
        is_confirmed: true,
        timer_status: 'completed',
        note: note || null
      }
    );
  },

  discardPlaySession(sessionId: string, reason: string) {
    demoPendingReceipts = demoPendingReceipts.filter((session) => session.id !== sessionId);
    return call<MutationResult>('discard_play_session', { sessionId, reason }, {
      operation_id: `discard-${Date.now()}`,
      affected_count: 1,
      message: '已标记为误启动，本次不会计入时长。'
    });
  },

  createSnapshot(reason: string) {
    return call<SnapshotInfo>('create_snapshot', { reason }, {
      id: `snapshot-${Date.now()}`,
      snapshot_type: 'manual',
      reason,
      path: 'backups/snapshots/manual-demo.zip',
      created_at: Date.now()
    });
  },

  createSaveProfile(workId: string, name: string, savePath: string) {
    return call<SaveProfile>('create_save_profile', { workId, name, savePath }, {
      id: `save-profile-${Date.now()}`,
      work_id: workId,
      name,
      save_path: savePath,
      engine: null,
      strategy: 'copy',
      is_active: true,
      created_at: Date.now(),
      updated_at: Date.now()
    });
  },

  createSaveSnapshot(
    workId: string,
    saveProfileId: string | null,
    sourcePath: string,
    note: string,
    routeName: string,
    progressLabel: string
  ) {
    return call<SaveSnapshot>(
      'create_save_snapshot',
      { workId, saveProfileId, sourcePath, note, routeName, progressLabel },
      {
        id: `save-snapshot-${Date.now()}`,
        work_id: workId,
        save_profile_id: saveProfileId,
        snapshot_path: `${sourcePath}/snapshot-demo`,
        note,
        route_name: routeName,
        progress_label: progressLabel,
        is_locked: false,
        created_at: Date.now(),
        updated_at: Date.now()
      }
    );
  },

  restoreSaveSnapshot(snapshotId: string, targetPath: string) {
    return call<SaveSnapshot>(
      'restore_save_snapshot',
      { snapshotId, targetPath },
      {
        id: snapshotId,
        work_id: demoItems[0].work_id,
        save_profile_id: 'save-profile-demo',
        snapshot_path: targetPath,
        note: '已恢复',
        route_name: null,
        progress_label: null,
        is_locked: false,
        created_at: Date.now(),
        updated_at: Date.now()
      }
    );
  },

  setSaveSnapshotLocked(snapshotId: string, isLocked: boolean) {
    appendDemoOperation({
      entity_type: 'save_snapshot',
      entity_id: snapshotId,
      operation_type: 'lock_update',
      summary: '更新存档快照锁定',
      payload_json: JSON.stringify({ snapshot_id: snapshotId, is_locked: isLocked })
    });
    return call<SaveSnapshot>(
      'set_save_snapshot_locked',
      { snapshotId, isLocked },
      {
        id: snapshotId,
        work_id: demoItems[0].work_id,
        save_profile_id: 'save-profile-demo',
        snapshot_path: '/app/saves/demo/files',
        note: '共通线结束',
        route_name: '共通线',
        progress_label: '分歧前',
        is_locked: isLocked,
        created_at: Date.now(),
        updated_at: Date.now()
      }
    );
  },

  deleteSaveSnapshot(snapshotId: string) {
    appendDemoOperation({
      entity_type: 'save_snapshot',
      entity_id: snapshotId,
      operation_type: 'delete',
      summary: '删除存档快照',
      payload_json: JSON.stringify({ snapshot_id: snapshotId })
    });
    return call<MutationResult>('delete_save_snapshot', { snapshotId }, {
      operation_id: `delete-save-${Date.now()}`,
      affected_count: 1,
      message: '存档快照已删除。'
    });
  },

  pickFolder() {
    return call<string | null>('pick_folder', undefined, null);
  },

  pickFile() {
    return call<string | null>('pick_file', undefined, null);
  }
};

export function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours <= 0) return `${minutes} 分钟`;
  return `${hours} 小时 ${minutes} 分钟`;
}

export function formatDate(value: number | null): string {
  if (!value) return '尚无记录';
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  }).format(new Date(value));
}

export function coverSrc(path: string | null): string | null {
  if (!path) return null;
  return isTauri ? convertFileSrc(path) : path;
}
