<script lang="ts">
  import { onMount } from 'svelte';
  import { api, coverSrc, formatDate, formatDuration } from './lib/api';
  import type {
    ImportCandidate,
    LaunchReceipt,
    LibraryItem,
    LibraryRoot,
    OperationItem,
    PathHealthReport,
    PlaySession,
    SaveSnapshot,
    ScanJob,
    SnapshotInfo,
    TodayDesk,
    WorkDetail
  } from './lib/types';

  type ViewKey = 'today' | 'shelf' | 'import' | 'archive' | 'timeline' | 'settings';
  type ConfirmState = {
    title: string;
    message: string;
    actionLabel: string;
    variant?: 'primary' | 'danger';
    onConfirm: () => Promise<void> | void;
  };

  const views: Array<{ key: ViewKey; label: string; summary: string }> = [
    { key: 'today', label: '今日', summary: '继续、整理和安全状态' },
    { key: 'shelf', label: '书架', summary: '浏览、搜索和筛选收藏' },
    { key: 'import', label: '整理', summary: '扫描候选并安全入库' },
    { key: 'archive', label: '档案', summary: '作品详情、计时和笔记' },
    { key: 'timeline', label: '时间线', summary: '游玩记录和月度轨迹' },
    { key: 'settings', label: '设置', summary: '快照、数据和隐私' }
  ];

  const statusOptions = [
    { value: '', label: '全部状态' },
    { value: 'unplayed', label: '未玩' },
    { value: 'playing', label: '在玩' },
    { value: 'completed', label: '已通' },
    { value: 'paused', label: '暂停' },
    { value: 'dropped', label: '弃坑' },
    { value: 'wishlist', label: '想玩' },
    { value: 'archived', label: '归档' }
  ];

  const LIBRARY_PAGE_SIZE = 240;
  const GRID_CARD_MIN_WIDTH = 146;
  const GRID_CARD_GAP = 14;
  const GRID_ROW_HEIGHT = 290;
  const LIST_ROW_HEIGHT = 86;
  const VIRTUAL_OVERSCAN_ROWS = 4;

  let activeView: ViewKey = 'today';
  let today: TodayDesk | null = null;
  let library: Array<LibraryItem | null> = [];
  let libraryTotal = 0;
  let libraryLoadedPages: Record<number, boolean> = {};
  let libraryLoadingPages: Record<number, boolean> = {};
  let libraryHasMore = false;
  let libraryLoadingMore = false;
  let libraryScrollTop = 0;
  let libraryViewportHeight = 640;
  let libraryShelfWidth = 900;
  let candidates: ImportCandidate[] = [];
  let libraryRoots: LibraryRoot[] = [];
  let scanJobs: ScanJob[] = [];
  let pendingReceipts: PlaySession[] = [];
  let operations: OperationItem[] = [];
  let receiptDrafts: Record<string, { minutes: string; note: string }> = {};
  let selectedWorkId = '';
  let workDetail: WorkDetail | null = null;
  let search = '';
  let status = '';
  let shelfViewMode: 'grid' | 'list' = 'grid';
  let scanPath = '';
  let editTitle = '';
  let editOriginalTitle = '';
  let editRating = '';
  let editTags = '';
  let editCoverPath = '';
  let editNsfwLevel = '0';
  let editPrivacyLevel = '0';
  let privacyMode = false;
  let launchProfileName = '默认启动';
  let launchExecutablePath = '';
  let launchWorkingDir = '';
  let launchArguments = '';
  let noteTitle = '';
  let noteContent = '';
  let noteSpoiler = false;
  let manualMinutes = 60;
  let manualNote = '';
  let saveProfileName = '默认存档';
  let savePath = '';
  let snapshotNote = '';
  let snapshotRoute = '';
  let snapshotProgress = '';
  let restoreTargetPath = '';
  let snapshotReason = '手动快照';
  let lastSnapshot: SnapshotInfo | null = null;
  let lastSession: PlaySession | null = null;
  let lastLaunch: LaunchReceipt | null = null;
  let lastSaveSnapshot: SaveSnapshot | null = null;
  let pathHealthReport: PathHealthReport | null = null;
  let lastOperationNotice = '';
  let loading = true;
  let busy = false;
  let error = '';
  let confirmDialog: ConfirmState | null = null;

  onMount(() => {
    privacyMode = window.localStorage.getItem('luki_privacy_mode') === '1';
    void refreshAll();
  });

  async function refreshAll() {
    loading = true;
    error = '';
    try {
      today = await api.getTodayDesk();
      await loadLibraryPage(true);
      candidates = await api.listImportCandidates();
      libraryRoots = await api.listLibraryRoots();
      scanJobs = await api.listScanJobs(8);
      pendingReceipts = await api.listUnconfirmedSessions();
      operations = await api.listOperations(30);
      seedReceiptDrafts();
      const firstItem = firstLoadedLibraryItem();
      if (!selectedWorkId && firstItem) {
        selectedWorkId = firstItem.work_id;
      }
      if (selectedWorkId) {
        await loadWork(selectedWorkId);
      }
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  }

  function seedReceiptDrafts() {
    const nextDrafts = { ...receiptDrafts };
    for (const session of pendingReceipts) {
      nextDrafts[session.id] ??= {
        minutes: Math.max(1, Math.round(session.duration_seconds / 60)).toString(),
        note: session.note ?? ''
      };
    }
    receiptDrafts = nextDrafts;
  }

  function updateReceiptDraft(
    sessionId: string,
    field: 'minutes' | 'note',
    value: string
  ) {
    receiptDrafts = {
      ...receiptDrafts,
      [sessionId]: {
        minutes: receiptDrafts[sessionId]?.minutes ?? '1',
        note: receiptDrafts[sessionId]?.note ?? '',
        [field]: value
      }
    };
  }

  function loadedLibraryItems(): LibraryItem[] {
    return library.filter((item): item is LibraryItem => item !== null);
  }

  function firstLoadedLibraryItem(): LibraryItem | null {
    return loadedLibraryItems()[0] ?? null;
  }

  function alignLibraryOffset(offset: number): number {
    return Math.max(0, Math.floor(offset / LIBRARY_PAGE_SIZE) * LIBRARY_PAGE_SIZE);
  }

  async function loadLibraryPage(reset = false, requestedOffset = 0) {
    const offset = reset ? 0 : alignLibraryOffset(requestedOffset);
    const pageIndex = Math.floor(offset / LIBRARY_PAGE_SIZE);
    if (!reset && (libraryLoadedPages[pageIndex] || libraryLoadingPages[pageIndex])) return;

    libraryLoadingPages = { ...libraryLoadingPages, [pageIndex]: true };
    if (reset) {
      libraryScrollTop = 0;
      library = [];
      libraryTotal = 0;
      libraryLoadedPages = {};
    }

    try {
      const page = await api.listLibraryPage(search, status, LIBRARY_PAGE_SIZE, offset);
      const nextLibrary =
        reset || library.length !== page.total
          ? Array<LibraryItem | null>(page.total).fill(null)
          : [...library];
      for (const [index, item] of page.items.entries()) {
        nextLibrary[page.offset + index] = item;
      }
      library = nextLibrary;
      libraryTotal = page.total;
      libraryLoadedPages = { ...libraryLoadedPages, [pageIndex]: true };
      libraryHasMore = loadedLibraryItems().length < libraryTotal;
    } finally {
      const { [pageIndex]: _finished, ...remaining } = libraryLoadingPages;
      libraryLoadingPages = remaining;
    }
  }

  function ensureLibraryRange(startIndex: number, endIndex: number) {
    if (!libraryTotal) return;
    const firstPage = Math.floor(Math.max(0, startIndex) / LIBRARY_PAGE_SIZE);
    const lastPage = Math.floor(Math.max(0, endIndex - 1) / LIBRARY_PAGE_SIZE);
    for (let pageIndex = firstPage; pageIndex <= lastPage; pageIndex += 1) {
      if (!libraryLoadedPages[pageIndex] && !libraryLoadingPages[pageIndex]) {
        void loadLibraryPage(false, pageIndex * LIBRARY_PAGE_SIZE);
      }
    }
  }

  function handleLibraryScroll(event: Event) {
    const target = event.currentTarget as HTMLElement;
    libraryScrollTop = target.scrollTop;
  }

  async function refreshLibrary() {
    error = '';
    await loadLibraryPage(true);
    if (!selectedWorkId) {
      const firstItem = firstLoadedLibraryItem();
      if (firstItem) await loadWork(firstItem.work_id);
    }
  }

  async function loadMoreLibrary() {
    libraryLoadingMore = true;
    error = '';
    try {
      const firstMissingIndex = library.findIndex((item) => item === null);
      await loadLibraryPage(false, firstMissingIndex >= 0 ? firstMissingIndex : library.length);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      libraryLoadingMore = false;
    }
  }

  async function loadWork(workId: string) {
    selectedWorkId = workId;
    workDetail = await api.getWorkDetail(workId);
    editTitle = workDetail.item.display_title;
    editOriginalTitle = workDetail.item.original_title ?? '';
    editRating = workDetail.item.user_rating?.toString() ?? '';
    editTags = workDetail.item.tag_summary ?? '';
    editCoverPath = workDetail.item.cover_path ?? '';
    editNsfwLevel = workDetail.item.nsfw_level.toString();
    editPrivacyLevel = workDetail.item.privacy_level.toString();
    const defaultLaunch = workDetail.launch_profiles.find((profile) => profile.is_default)
      ?? workDetail.launch_profiles[0]
      ?? null;
    launchProfileName = defaultLaunch?.name ?? '默认启动';
    launchExecutablePath = defaultLaunch?.executable_path ?? '';
    launchWorkingDir = defaultLaunch?.working_dir ?? '';
    launchArguments = defaultLaunch?.arguments ?? '';
  }

  async function runScan() {
    if (!scanPath.trim()) {
      error = '请输入要扫描的目录路径。';
      return;
    }
    busy = true;
    error = '';
    try {
      candidates = await api.scanLibraryRoot(scanPath.trim());
      libraryRoots = await api.listLibraryRoots();
      scanJobs = await api.listScanJobs(8);
      const latestScan = scanJobs[0];
      if (latestScan) {
        lastOperationNotice = `扫描完成：发现 ${latestScan.matched_count} 个候选，失败 ${latestScan.failed_count} 项。`;
      }
      activeView = 'import';
      today = await api.getTodayDesk();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function chooseScanPath() {
    const folder = await api.pickFolder();
    if (folder) scanPath = folder;
  }

  async function acceptCandidate(candidateId: string) {
    busy = true;
    error = '';
    try {
      const item = await api.acceptImportCandidate(candidateId);
      await refreshAll();
      await loadWork(item.work_id);
      activeView = 'archive';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function acceptAllCandidates() {
    busy = true;
    error = '';
    try {
      const result = await api.acceptPendingImportCandidates();
      lastOperationNotice = `已批量导入 ${result.imported_count} 部作品，并创建 ${result.snapshot.snapshot_type} 快照。`;
      await refreshAll();
      activeView = 'shelf';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  function confirmAcceptAllCandidates() {
    confirmDialog = {
      title: '批量导入候选',
      message: '导入前会自动创建数据库快照，并把所有待确认候选写入正式书架。',
      actionLabel: '创建快照并导入',
      onConfirm: acceptAllCandidates
    };
  }

  async function undoBulkImport() {
    busy = true;
    error = '';
    try {
      const result = await api.undoLatestBulkImport();
      lastOperationNotice = `已撤销批量导入，移出 ${result.affected_work_count} 部作品，还原 ${result.restored_candidate_count} 个候选。`;
      selectedWorkId = '';
      workDetail = null;
      await refreshAll();
      activeView = 'import';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  function confirmUndoBulkImport() {
    confirmDialog = {
      title: '撤销上次批量导入',
      message: '这会把上次批量导入的作品移出书架，并把对应候选还原为待确认。',
      actionLabel: '撤销导入',
      variant: 'danger',
      onConfirm: undoBulkImport
    };
  }

  async function recordManualSession() {
    if (!selectedWorkId) return;
    busy = true;
    error = '';
    try {
      lastSession = await api.recordManualSession(
        selectedWorkId,
        Math.max(1, manualMinutes) * 60,
        manualNote
      );
      manualNote = '';
      await refreshAll();
      await loadWork(selectedWorkId);
      activeView = 'timeline';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function createWorkNote() {
    if (!selectedWorkId) return;
    if (!noteContent.trim()) {
      error = '笔记内容不能为空。';
      return;
    }
    busy = true;
    error = '';
    try {
      await api.createNote(
        selectedWorkId,
        noteTitle,
        noteContent,
        'note',
        noteSpoiler ? 1 : 0,
        0
      );
      noteTitle = '';
      noteContent = '';
      noteSpoiler = false;
      lastOperationNotice = '笔记已写入作品时间线。';
      await refreshAll();
      await loadWork(selectedWorkId);
      activeView = 'timeline';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function confirmReceipt(sessionId: string) {
    const draft = receiptDrafts[sessionId] ?? { minutes: '1', note: '' };
    const minutes = Number(draft.minutes);
    if (!Number.isFinite(minutes) || minutes <= 0) {
      error = '回执时长需要是大于 0 的分钟数。';
      return;
    }
    busy = true;
    error = '';
    try {
      lastSession = await api.confirmPlaySession(
        sessionId,
        Math.round(minutes * 60),
        draft.note
      );
      lastOperationNotice = `已确认 ${lastSession.title} 的游玩回执，计入 ${formatDuration(lastSession.duration_seconds)}。`;
      activeView = 'timeline';
      await refreshAll();
      if (selectedWorkId) await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function discardReceipt(sessionId: string) {
    busy = true;
    error = '';
    try {
      const result = await api.discardPlaySession(sessionId, '用户标记为误启动');
      lastOperationNotice = result.message;
      await refreshAll();
      activeView = 'timeline';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  function confirmDiscardReceipt(sessionId: string) {
    confirmDialog = {
      title: '标记为误启动',
      message: '这条游玩回执会保留在记录中，但不会计入作品总时长和月度统计。',
      actionLabel: '不计入时长',
      variant: 'danger',
      onConfirm: () => discardReceipt(sessionId)
    };
  }

  async function launchSelectedWork() {
    if (!selectedWorkId) return;
    busy = true;
    error = '';
    try {
      lastLaunch = await api.launchWork(selectedWorkId);
      await refreshAll();
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function updateSelectedStatus(nextStatus: string) {
    if (!selectedWorkId) return;
    busy = true;
    error = '';
    try {
      await api.updateWorkStatus(selectedWorkId, nextStatus);
      await refreshAll();
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function saveWorkMetadata() {
    if (!selectedWorkId) return;
    const rating = editRating.trim() ? Number(editRating) : null;
    if (rating !== null && Number.isNaN(rating)) {
      error = '评分需要是数字，范围 0-10。';
      return;
    }
    const nsfwLevel = Number(editNsfwLevel);
    const privacyLevel = Number(editPrivacyLevel);
    busy = true;
    error = '';
    try {
      await api.updateWorkMetadata(
        selectedWorkId,
        editTitle,
        editOriginalTitle,
        rating,
        editTags,
        editCoverPath,
        Number.isFinite(nsfwLevel) ? nsfwLevel : 0,
        Number.isFinite(privacyLevel) ? privacyLevel : 0
      );
      lastOperationNotice = '基础资料已保存。';
      await refreshAll();
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function chooseCoverPath() {
    const file = await api.pickFile();
    if (file) editCoverPath = file;
  }

  async function chooseLaunchExecutable() {
    const file = await api.pickFile();
    if (!file) return;
    launchExecutablePath = file;
    if (!launchWorkingDir) {
      const separator = file.includes('\\') ? '\\' : '/';
      launchWorkingDir = file.split(separator).slice(0, -1).join(separator);
    }
  }

  async function chooseLaunchWorkingDir() {
    const folder = await api.pickFolder();
    if (folder) launchWorkingDir = folder;
  }

  async function saveLaunchProfile() {
    if (!selectedWorkId) return;
    if (!launchExecutablePath.trim()) {
      error = '请选择启动文件。';
      return;
    }
    busy = true;
    error = '';
    try {
      await api.upsertDefaultLaunchProfile(
        selectedWorkId,
        launchProfileName,
        launchExecutablePath.trim(),
        launchWorkingDir.trim(),
        launchArguments
      );
      lastOperationNotice = '默认启动方式已更新。';
      await refreshAll();
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function softDeleteSelectedWork() {
    if (!selectedWorkId) return;
    busy = true;
    error = '';
    try {
      const result = await api.softDeleteWork(selectedWorkId);
      lastOperationNotice = result.message;
      selectedWorkId = '';
      workDetail = null;
      await refreshAll();
      activeView = 'shelf';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  function confirmSoftDeleteSelectedWork() {
    confirmDialog = {
      title: '移出书架',
      message: '作品会从书架和搜索结果中移除，原始文件不会被删除。',
      actionLabel: '移出书架',
      variant: 'danger',
      onConfirm: softDeleteSelectedWork
    };
  }

  async function createSaveProfile() {
    if (!selectedWorkId || !savePath.trim()) {
      error = '请输入存档路径。';
      return;
    }
    busy = true;
    error = '';
    try {
      const profile = await api.createSaveProfile(selectedWorkId, saveProfileName, savePath.trim());
      savePath = profile.save_path;
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function chooseSavePath() {
    const folder = await api.pickFolder();
    if (folder) savePath = folder;
  }

  async function chooseRestoreTarget() {
    const folder = await api.pickFolder();
    if (folder) restoreTargetPath = folder;
  }

  async function createSaveSnapshot() {
    if (!selectedWorkId) return;
    const profile = workDetail?.save_profiles[0] ?? null;
    const sourcePath = profile?.save_path ?? savePath.trim();
    if (!sourcePath) {
      error = '先添加存档路径，或输入一个可复制的存档目录。';
      return;
    }
    busy = true;
    error = '';
    try {
      lastSaveSnapshot = await api.createSaveSnapshot(
        selectedWorkId,
        profile?.id ?? null,
        sourcePath,
        snapshotNote,
        snapshotRoute,
        snapshotProgress
      );
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function restoreSaveSnapshot(snapshotId: string) {
    const targetPath = restoreTargetPath.trim() || workDetail?.save_profiles[0]?.save_path || '';
    if (!targetPath) {
      error = '请输入恢复目标路径，或先添加存档配置。';
      return;
    }
    busy = true;
    error = '';
    try {
      lastSaveSnapshot = await api.restoreSaveSnapshot(snapshotId, targetPath);
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function setSaveSnapshotLocked(snapshotId: string, isLocked: boolean) {
    busy = true;
    error = '';
    try {
      await api.setSaveSnapshotLocked(snapshotId, isLocked);
      lastOperationNotice = isLocked ? '存档快照已锁定。' : '存档快照已解除锁定。';
      await refreshAll();
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function deleteSaveSnapshot(snapshotId: string) {
    busy = true;
    error = '';
    try {
      const result = await api.deleteSaveSnapshot(snapshotId);
      lastOperationNotice = result.message;
      lastSaveSnapshot = null;
      await refreshAll();
      await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  function confirmRestoreSaveSnapshot(snapshotId: string) {
    confirmDialog = {
      title: '恢复存档快照',
      message: '恢复前会备份当前目标路径，然后把所选快照复制回目标位置。',
      actionLabel: '恢复快照',
      variant: 'danger',
      onConfirm: () => restoreSaveSnapshot(snapshotId)
    };
  }

  function confirmDeleteSaveSnapshot(snapshotId: string) {
    confirmDialog = {
      title: '删除存档快照',
      message: '这会删除这个恢复点的备份文件。已锁定的快照需要先解除锁定。',
      actionLabel: '删除快照',
      variant: 'danger',
      onConfirm: () => deleteSaveSnapshot(snapshotId)
    };
  }

  async function createSnapshot() {
    busy = true;
    error = '';
    try {
      lastSnapshot = await api.createSnapshot(snapshotReason);
      today = await api.getTodayDesk();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  async function checkPathHealth() {
    busy = true;
    error = '';
    try {
      pathHealthReport = await api.checkPathHealth();
      lastOperationNotice = `已检查 ${pathHealthReport.checked_count} 个安装位置，${pathHealthReport.missing_count} 个不可用。`;
      operations = await api.listOperations(30);
      await loadLibraryPage(true);
      if (selectedWorkId) await loadWork(selectedWorkId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  function statusLabel(value: string): string {
    return statusOptions.find((item) => item.value === value)?.label ?? value;
  }

  function timelineEventLabel(value: string): string {
    if (value === 'play_session') return '游玩';
    if (value === 'save_snapshot') return '存档';
    if (value === 'play_note' || value === 'play_receipt') return '回执笔记';
    return '笔记';
  }

  function operationDetail(operation: OperationItem): string {
    try {
      const parsed = JSON.parse(operation.payload_json) as Record<string, unknown>;
      const keys = Object.keys(parsed).slice(0, 3);
      if (!keys.length) return operation.entity_type;
      return keys.map((key) => `${key}: ${String(parsed[key])}`).join(' · ');
    } catch {
      return operation.payload_json.slice(0, 120);
    }
  }

  function scanJobStatusLabel(value: string): string {
    if (value === 'completed') return '已完成';
    if (value === 'running') return '扫描中';
    if (value === 'failed') return '失败';
    if (value === 'cancelled') return '已取消';
    return value;
  }

  function candidateEvidence(candidate: ImportCandidate): string[] {
    if (!candidate.evidence_json) return ['暂无扫描证据'];
    try {
      const parsed = JSON.parse(candidate.evidence_json) as Record<string, unknown>;
      return Object.entries(parsed).map(([key, value]) => `${key}: ${String(value)}`);
    } catch {
      return [candidate.evidence_json];
    }
  }

  function togglePrivacyMode() {
    privacyMode = !privacyMode;
    window.localStorage.setItem('luki_privacy_mode', privacyMode ? '1' : '0');
  }

  function isSensitiveItem(item: Pick<LibraryItem, 'nsfw_level' | 'privacy_level'>) {
    return item.nsfw_level > 0 || item.privacy_level > 0;
  }

  function shouldMaskCover(
    item: Pick<LibraryItem, 'nsfw_level' | 'privacy_level'>,
    mode = privacyMode
  ) {
    return mode && isSensitiveItem(item);
  }

  function displayTitle(
    item: Pick<LibraryItem, 'display_title' | 'privacy_level'>,
    mode = privacyMode
  ) {
    return mode && item.privacy_level > 0 ? '隐私作品' : item.display_title;
  }

  function displayOriginalTitle(
    item: Pick<LibraryItem, 'original_title' | 'privacy_level'>,
    mode = privacyMode
  ) {
    if (mode && item.privacy_level > 0) return '标题已隐藏';
    return item.original_title ?? '未设置原名';
  }

  function coverFallback(
    item: Pick<LibraryItem, 'display_title' | 'privacy_level' | 'nsfw_level'>,
    mode = privacyMode
  ) {
    if (shouldMaskCover(item, mode)) return '已遮挡';
    return displayTitle(item, mode).slice(0, 1);
  }

  function itemCoverSrc(item: LibraryItem, mode = privacyMode) {
    if (shouldMaskCover(item, mode)) return null;
    return coverSrc(item.cover_thumbnail_path ?? item.cover_path);
  }

  async function confirmActiveAction() {
    if (!confirmDialog) return;
    const action = confirmDialog.onConfirm;
    confirmDialog = null;
    await action();
  }

  $: loadedLibrary = loadedLibraryItems();
  $: libraryLoadedCount = loadedLibrary.length;
  $: libraryHasMore = libraryLoadedCount < libraryTotal;
  $: libraryColumns =
    shelfViewMode === 'grid'
      ? Math.max(1, Math.floor((libraryShelfWidth + GRID_CARD_GAP) / (GRID_CARD_MIN_WIDTH + GRID_CARD_GAP)))
      : 1;
  $: virtualRowHeight = shelfViewMode === 'grid' ? GRID_ROW_HEIGHT : LIST_ROW_HEIGHT;
  $: virtualTotalRows = Math.ceil(libraryTotal / libraryColumns);
  $: virtualStartRow = Math.max(
    0,
    Math.floor(libraryScrollTop / virtualRowHeight) - VIRTUAL_OVERSCAN_ROWS
  );
  $: virtualVisibleRows =
    Math.ceil(libraryViewportHeight / virtualRowHeight) + VIRTUAL_OVERSCAN_ROWS * 2;
  $: virtualEndRow = Math.min(virtualTotalRows, virtualStartRow + virtualVisibleRows);
  $: virtualStartIndex = virtualStartRow * libraryColumns;
  $: virtualEndIndex = Math.min(libraryTotal, virtualEndRow * libraryColumns);
  $: virtualLibraryItems = library.slice(virtualStartIndex, virtualEndIndex);
  $: virtualTopPadding = virtualStartRow * virtualRowHeight;
  $: virtualBottomPadding = Math.max(0, (virtualTotalRows - virtualEndRow) * virtualRowHeight);
  $: if (activeView === 'shelf' && virtualEndIndex > virtualStartIndex) {
    ensureLibraryRange(virtualStartIndex, virtualEndIndex + LIBRARY_PAGE_SIZE);
  }
  $: selectedItem =
    loadedLibrary.find((item) => item.work_id === selectedWorkId) ?? workDetail?.item;
</script>

<main class="shell">
  <aside class="sidebar" aria-label="主导航">
    <div class="brand">
      <div class="brand-mark">L</div>
      <div>
        <strong>Luki</strong>
        <span>VN 私人书房</span>
      </div>
    </div>

    <nav>
      {#each views as view}
        <button
          class:active={activeView === view.key}
          type="button"
          on:click={() => (activeView = view.key)}
        >
          <span>{view.label}</span>
          <small>{view.summary}</small>
        </button>
      {/each}
    </nav>
  </aside>

  <section class="workspace">
    <header class="topbar">
      <div>
        <p class="eyebrow">Local-first visual novel library</p>
        <h1>{views.find((view) => view.key === activeView)?.label}</h1>
      </div>
      <div class="top-actions">
        {#if busy}
          <span class="status-pill">后台处理中</span>
        {/if}
        {#if privacyMode}
          <span class="status-pill privacy-active">隐私模式</span>
        {/if}
        <button type="button" class="secondary" on:click={togglePrivacyMode}>
          {privacyMode ? '关闭隐私' : '隐私模式'}
        </button>
        <button type="button" class="secondary" on:click={refreshAll}>刷新</button>
      </div>
    </header>

    {#if error}
      <div class="notice error">
        <strong>需要处理</strong>
        <span>{error}</span>
      </div>
    {/if}

    {#if lastOperationNotice}
      <div class="notice">
        <strong>操作完成</strong>
        <span>{lastOperationNotice}</span>
      </div>
    {/if}

    {#if loading}
      <div class="panel loading-panel">正在打开私人书房...</div>
    {:else if activeView === 'today'}
      <div class="content-grid today-grid">
        <section class="panel hero-panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">继续游玩</p>
              <h2>回到上次中断的位置</h2>
            </div>
            <button type="button" on:click={() => (activeView = 'shelf')}>打开书架</button>
          </div>

          {#if today?.continue_items.length}
            <div class="continue-list">
              {#each today.continue_items as item}
                <article class="work-row featured">
                  <div class="cover compact" class:sensitive={shouldMaskCover(item, privacyMode)}>
                    {#if itemCoverSrc(item, privacyMode)}
                      <img src={itemCoverSrc(item, privacyMode) ?? ''} alt="" />
                    {:else}
                      {coverFallback(item, privacyMode)}
                    {/if}
                  </div>
                  <div>
                    <h3>{displayTitle(item, privacyMode)}</h3>
                    <p>{displayOriginalTitle(item, privacyMode)}</p>
                    <div class="meta-line">
                      <span>{statusLabel(item.status)}</span>
                      <span>{formatDuration(item.total_playtime_seconds)}</span>
                      <span>{formatDate(item.last_played_at)}</span>
                    </div>
                  </div>
                  <div class="row-actions">
                    <button type="button" on:click={() => loadWork(item.work_id).then(() => (activeView = 'archive'))}>
                      档案
                    </button>
                    <button class="primary" type="button" on:click={() => loadWork(item.work_id).then(launchSelectedWork)}>
                      继续游玩
                    </button>
                  </div>
                </article>
              {/each}
            </div>
          {:else}
            <div class="empty-state">
              <h3>还没有可以继续的作品</h3>
              <p>先扫描一个游戏目录，候选项会进入整理工作台，不会直接污染正式库。</p>
              <div class="inline-form">
                <input bind:value={scanPath} placeholder="/path/to/visual-novels" />
                <button type="button" class="secondary" on:click={chooseScanPath}>选择目录</button>
                <button type="button" on:click={runScan}>扫描目录</button>
              </div>
            </div>
          {/if}
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">待整理</p>
              <h2>{today?.pending_import_count ?? 0} 项候选</h2>
            </div>
            <button type="button" class="secondary" on:click={() => (activeView = 'import')}>进入整理</button>
          </div>
          <div class="metric-stack">
            <div>
              <strong>{today?.unconfirmed_session_count ?? 0}</strong>
              <span>待确认计时</span>
              {#if pendingReceipts.length}
                <button type="button" class="secondary compact-action" on:click={() => (activeView = 'timeline')}>
                  查看回执
                </button>
              {/if}
            </div>
            <div>
              <strong>{today?.last_snapshot ? formatDate(today.last_snapshot.created_at) : '尚无'}</strong>
              <span>最近快照</span>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">本月轨迹</p>
              <h2>{formatDuration(today?.month.duration_seconds ?? 0)}</h2>
            </div>
          </div>
          <div class="metric-grid">
            <div><strong>{today?.month.active_days ?? 0}</strong><span>游玩天数</span></div>
            <div><strong>{today?.month.completed_count ?? 0}</strong><span>通关作品</span></div>
            <div><strong>{today?.month.most_played_title ?? '暂无'}</strong><span>停留最多</span></div>
          </div>
        </section>

        <section class="panel wide">
          <div class="section-heading">
            <div>
              <p class="eyebrow">最近收藏</p>
              <h2>书架中的近况</h2>
            </div>
          </div>
          <div class="shelf-strip">
            {#each today?.recent_items ?? [] as item}
              <button type="button" class="mini-cover" on:click={() => loadWork(item.work_id).then(() => (activeView = 'archive'))}>
                <span class:sensitive={shouldMaskCover(item, privacyMode)}>
                  {#if itemCoverSrc(item, privacyMode)}
                    <img src={itemCoverSrc(item, privacyMode) ?? ''} alt="" />
                  {:else}
                    {coverFallback(item, privacyMode)}
                  {/if}
                </span>
                <small>{displayTitle(item, privacyMode)}</small>
              </button>
            {/each}
          </div>
        </section>
      </div>
    {:else if activeView === 'shelf'}
      <section class="panel">
        <div class="toolbar">
          <input bind:value={search} on:input={refreshLibrary} placeholder="搜索标题、原名、标签" />
          <select bind:value={status} on:change={refreshLibrary}>
            {#each statusOptions as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
          <div class="segmented-control" aria-label="书架视图">
            <button
              type="button"
              class:active={shelfViewMode === 'grid'}
              on:click={() => (shelfViewMode = 'grid')}
            >
              网格
            </button>
            <button
              type="button"
              class:active={shelfViewMode === 'list'}
              on:click={() => (shelfViewMode = 'list')}
            >
              列表
            </button>
          </div>
          <span>已加载 {libraryLoadedCount}/{libraryTotal} 部作品{libraryHasMore ? '，滚动继续加载' : ''}</span>
        </div>
        {#if libraryTotal > 0}
          <div
            class="virtual-shelf"
            bind:clientHeight={libraryViewportHeight}
            bind:clientWidth={libraryShelfWidth}
            on:scroll={handleLibraryScroll}
          >
            <div class="virtual-spacer" style={`height: ${virtualTopPadding}px;`}></div>
            <div
              class={`${shelfViewMode === 'grid' ? 'library-grid' : 'library-list'} virtual-items`}
              style={`--virtual-columns: ${libraryColumns};`}
            >
              {#each virtualLibraryItems as item, index (item?.work_id ?? `placeholder-${virtualStartIndex + index}`)}
                {#if item}
                  <button
                    type="button"
                    class={shelfViewMode === 'grid' ? 'work-card' : 'work-list-row'}
                    class:selected={selectedWorkId === item.work_id}
                    on:click={() => loadWork(item.work_id).then(() => (activeView = 'archive'))}
                  >
                    <span class="cover" class:sensitive={shouldMaskCover(item, privacyMode)}>
                      {#if itemCoverSrc(item, privacyMode)}
                        <img src={itemCoverSrc(item, privacyMode) ?? ''} alt="" />
                      {:else}
                        {coverFallback(item, privacyMode)}
                      {/if}
                    </span>
                    <span class="title">{displayTitle(item, privacyMode)}</span>
                    <span class="sub">
                      {statusLabel(item.status)} · {formatDuration(item.total_playtime_seconds)}
                      {#if item.tag_summary && !(privacyMode && item.privacy_level > 0)} · {item.tag_summary}{/if}
                    </span>
                    {#if shelfViewMode === 'list'}
                      <span class="sub">最近 {formatDate(item.last_played_at)} · 安装 {item.available_installation_count}/{item.installation_count}</span>
                    {/if}
                  </button>
                {:else}
                  <div
                    class={shelfViewMode === 'grid' ? 'work-card skeleton-card' : 'work-list-row skeleton-card'}
                    aria-hidden="true"
                  >
                    <span class="cover skeleton-block"></span>
                    <span class="title skeleton-line"></span>
                    <span class="sub skeleton-line short"></span>
                    {#if shelfViewMode === 'list'}
                      <span class="sub skeleton-line"></span>
                    {/if}
                  </div>
                {/if}
              {/each}
            </div>
            <div class="virtual-spacer" style={`height: ${virtualBottomPadding}px;`}></div>
          </div>
          {#if libraryHasMore}
            <div class="load-more-row">
              <button type="button" class="secondary" disabled={libraryLoadingMore} on:click={loadMoreLibrary}>
                {libraryLoadingMore ? '正在加载...' : '加载下一页'}
              </button>
            </div>
          {/if}
        {:else}
          <div class="empty-state">
            <h3>没有匹配的作品</h3>
            <p>清除筛选，或从整理工作台导入新的候选。</p>
          </div>
        {/if}
      </section>
    {:else if activeView === 'import'}
      <section class="panel">
        <div class="section-heading">
          <div>
            <p class="eyebrow">整理入库</p>
            <h2>扫描结果先进入候选区</h2>
          </div>
        </div>
        <div class="inline-form">
          <input bind:value={scanPath} placeholder="输入游戏根目录路径" />
          <button type="button" class="secondary" on:click={chooseScanPath}>选择目录</button>
          <button type="button" on:click={runScan}>扫描目录</button>
        </div>
      </section>

      <div class="content-grid two">
        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">扫描任务</p>
              <h2>最近 {scanJobs.length} 次扫描</h2>
            </div>
          </div>
          <div class="task-list">
            {#each scanJobs as job}
              <article>
                <div>
                  <strong>{job.root_path ?? '未知目录'}</strong>
                  <span>{scanJobStatusLabel(job.status)} · {formatDate(job.finished_at ?? job.started_at)}</span>
                </div>
                <small>候选 {job.matched_count}/{job.total_count} · 失败 {job.failed_count}</small>
              </article>
            {:else}
              <p class="muted">还没有扫描任务。选择目录后，这里会保留任务结果。</p>
            {/each}
          </div>
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">库目录</p>
              <h2>{libraryRoots.length} 个根目录</h2>
            </div>
          </div>
          <div class="task-list">
            {#each libraryRoots as root}
              <article>
                <div>
                  <strong>{root.name ?? root.path}</strong>
                  <span>{root.path}</span>
                </div>
                <small>{root.is_active ? '启用' : '停用'} · 最近扫描 {formatDate(root.last_scanned_at)}</small>
              </article>
            {:else}
              <p class="muted">还没有库目录记录。</p>
            {/each}
          </div>
        </section>
      </div>

      <section class="panel">
        <div class="section-heading">
          <div>
            <p class="eyebrow">候选项</p>
            <h2>{candidates.length} 项扫描结果</h2>
          </div>
          <div class="row-actions">
            <button type="button" on:click={confirmAcceptAllCandidates}>批量导入全部</button>
            <button type="button" class="secondary" on:click={confirmUndoBulkImport}>撤销上次批量导入</button>
          </div>
        </div>
        <div class="table-list">
          {#each candidates as candidate}
            <article class="candidate-row">
              <div>
                <strong>{candidate.detected_title ?? '未命名候选'}</strong>
                <span>{candidate.path}</span>
                <small>置信度 {Math.round(candidate.confidence * 100)}% · {candidate.detected_executable ?? '未检测到启动文件'}</small>
                <details class="evidence">
                  <summary>扫描证据</summary>
                  <ul>
                    {#each candidateEvidence(candidate) as evidence}
                      <li>{evidence}</li>
                    {/each}
                  </ul>
                </details>
              </div>
              <div class="row-actions">
                <span class="status-pill">{candidate.status}</span>
                {#if candidate.status !== 'imported'}
                  <button type="button" on:click={() => acceptCandidate(candidate.id)}>确认入库</button>
                {/if}
              </div>
            </article>
          {:else}
            <div class="empty-state">
              <h3>候选区为空</h3>
              <p>扫描目录后，Luki 会把结果放在这里等待确认。</p>
            </div>
          {/each}
        </div>
      </section>
    {:else if activeView === 'archive'}
      {#if workDetail}
        <section class="panel archive-head">
          <div class="cover large" class:sensitive={shouldMaskCover(workDetail.item, privacyMode)}>
            {#if itemCoverSrc(workDetail.item, privacyMode)}
              <img src={itemCoverSrc(workDetail.item, privacyMode) ?? ''} alt="" />
            {:else}
              {coverFallback(workDetail.item, privacyMode)}
            {/if}
          </div>
          <div>
            <p class="eyebrow">作品档案</p>
            <h2>{displayTitle(workDetail.item, privacyMode)}</h2>
            <p>{displayOriginalTitle(workDetail.item, privacyMode)} · {statusLabel(workDetail.item.status)}</p>
            <div class="meta-line">
              <span>总时长 {formatDuration(workDetail.item.total_playtime_seconds)}</span>
              <span>最近 {formatDate(workDetail.item.last_played_at)}</span>
              <span>{privacyMode && workDetail.item.privacy_level > 0 ? '标签已隐藏' : workDetail.item.tag_summary ?? '暂无标签'}</span>
            </div>
            <div class="archive-actions">
              <button class="primary" type="button" on:click={launchSelectedWork}>继续游玩</button>
              <select
                value={workDetail.item.status}
                on:change={(event) => updateSelectedStatus(event.currentTarget.value)}
              >
                {#each statusOptions.filter((option) => option.value) as option}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
              <button type="button" class="secondary" on:click={confirmSoftDeleteSelectedWork}>移出书架</button>
            </div>
          </div>
        </section>

        {#if lastLaunch}
          <div class="notice">
            <strong>已启动</strong>
            <span>{lastLaunch.title} · {lastLaunch.launch_profile_name} · 进程 {lastLaunch.process_id ?? '未知'}</span>
          </div>
        {/if}

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">启动方式</p>
              <h2>默认启动配置</h2>
            </div>
            <button type="button" on:click={saveLaunchProfile}>保存启动方式</button>
          </div>
          {#if workDetail.launch_profiles.length}
            <div class="launch-summary">
              {#each workDetail.launch_profiles as profile}
                <article>
                  <strong>{profile.name}{profile.is_default ? ' · 默认' : ''}</strong>
                  <span>{profile.executable_path}</span>
                  <small>{profile.is_available ? '路径可用' : '路径不可用，需要重新定位'}</small>
                </article>
              {/each}
            </div>
          {:else}
            <p class="muted">还没有启动方式。选择启动文件后，Luki 会把它作为默认继续入口。</p>
          {/if}
          <div class="metadata-grid launch-editor">
            <label>
              名称
              <input bind:value={launchProfileName} />
            </label>
            <label>
              启动文件
              <input bind:value={launchExecutablePath} placeholder="/path/to/game.exe" />
            </label>
            <label>
              工作目录
              <input bind:value={launchWorkingDir} placeholder="默认使用启动文件所在目录" />
            </label>
            <label>
              启动参数
              <input bind:value={launchArguments} placeholder="可选" />
            </label>
          </div>
          <div class="row-actions launch-actions">
            <button type="button" class="secondary" on:click={chooseLaunchExecutable}>选择启动文件</button>
            <button type="button" class="secondary" on:click={chooseLaunchWorkingDir}>选择工作目录</button>
            <button type="button" class="primary" on:click={launchSelectedWork}>测试启动</button>
          </div>
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">基础资料</p>
              <h2>用户手写内容优先</h2>
            </div>
            <button type="button" on:click={saveWorkMetadata}>保存资料</button>
          </div>
          <div class="metadata-grid">
            <label>
              标题
              <input bind:value={editTitle} />
            </label>
            <label>
              原名
              <input bind:value={editOriginalTitle} />
            </label>
            <label>
              评分
              <input bind:value={editRating} placeholder="0-10" />
            </label>
            <label>
              标签摘要
              <input bind:value={editTags} placeholder="Key, 夏日, 共通线" />
            </label>
            <label>
              封面路径
              <input bind:value={editCoverPath} placeholder="/path/to/cover.jpg" />
            </label>
            <label>
              NSFW 等级
              <select bind:value={editNsfwLevel}>
                <option value="0">普通</option>
                <option value="1">轻度敏感</option>
                <option value="2">成人内容</option>
                <option value="3">强隐藏</option>
              </select>
            </label>
            <label>
              隐私等级
              <select bind:value={editPrivacyLevel}>
                <option value="0">公开显示</option>
                <option value="1">隐私模式隐藏标题</option>
                <option value="2">隐私模式强隐藏</option>
                <option value="3">报告和搜索后续排除</option>
              </select>
            </label>
          </div>
          <div class="row-actions launch-actions">
            <button type="button" class="secondary" on:click={chooseCoverPath}>选择封面文件</button>
          </div>
        </section>

        <div class="content-grid two">
          <section class="panel">
            <div class="section-heading">
              <div>
                <p class="eyebrow">补记计时</p>
                <h2>可信记录可以被修正</h2>
              </div>
            </div>
            <div class="inline-form stacked">
              <label>
                分钟数
                <input type="number" min="1" bind:value={manualMinutes} />
              </label>
              <label>
                一句笔记
                <input bind:value={manualNote} placeholder="例如：推进到共通线结束" />
              </label>
              <button type="button" on:click={recordManualSession}>记录本次游玩</button>
            </div>
          </section>

          <section class="panel">
            <div class="section-heading">
              <div>
                <p class="eyebrow">最近记录</p>
                <h2>{workDetail.recent_sessions.length} 次会话</h2>
              </div>
            </div>
            <div class="timeline-list">
              {#each workDetail.recent_sessions as session}
                <article>
                  <strong>{formatDuration(session.duration_seconds)}</strong>
                  <span>{formatDate(session.started_at)} · {session.source} · 可信度 {Math.round(session.confidence * 100)}%</span>
                  {#if session.note}<p>{session.note}</p>{/if}
                </article>
              {:else}
                <p class="muted">还没有游玩记录。</p>
              {/each}
            </div>
          </section>

          <section class="panel">
            <div class="section-heading">
              <div>
                <p class="eyebrow">笔记</p>
                <h2>{workDetail.notes.length} 条记录</h2>
              </div>
              <button type="button" on:click={createWorkNote}>写入时间线</button>
            </div>
            <div class="inline-form stacked">
              <label>
                标题
                <input bind:value={noteTitle} placeholder="可选，例如：分歧点" />
              </label>
              <label>
                内容
                <textarea bind:value={noteContent} placeholder="写下一句路线、感想或整理备注"></textarea>
              </label>
              <label class="check-row">
                <input type="checkbox" bind:checked={noteSpoiler} />
                <span>包含剧透，时间线中标记提醒</span>
              </label>
            </div>
            <div class="timeline-list compact">
              {#each workDetail.notes.slice(0, 3) as note}
                <article>
                  <strong>{note.title ?? '笔记'}</strong>
                  <span>{formatDate(note.created_at)} · {note.note_type}{note.spoiler_level > 0 ? ' · 剧透' : ''}</span>
                  <p>{note.content}</p>
                </article>
              {:else}
                <p class="muted">还没有笔记。第一条笔记会进入作品时间线。</p>
              {/each}
            </div>
          </section>

          <section class="panel">
            <div class="section-heading">
              <div>
                <p class="eyebrow">存档保险箱</p>
                <h2>路径、快照和恢复点</h2>
              </div>
            </div>
            <div class="inline-form stacked">
              <label>
                配置名称
                <input bind:value={saveProfileName} />
              </label>
              <label>
                存档路径
                <input bind:value={savePath} placeholder="/path/to/save-folder" />
              </label>
              <button type="button" class="secondary" on:click={chooseSavePath}>选择存档目录</button>
              <button type="button" on:click={createSaveProfile}>添加存档配置</button>
            </div>

            <div class="save-list">
              {#each workDetail.save_profiles as profile}
                <article>
                  <strong>{profile.name}</strong>
                  <span>{profile.save_path}</span>
                  <small>{profile.strategy} · {profile.is_active ? '启用' : '停用'}</small>
                </article>
              {:else}
                <p class="muted">还没有存档配置。添加路径后可以创建恢复点。</p>
              {/each}
            </div>

            <div class="inline-form stacked">
              <label>
                快照备注
                <input bind:value={snapshotNote} placeholder="例如：共通线结束" />
              </label>
              <label>
                路线
                <input bind:value={snapshotRoute} placeholder="例如：共通线" />
              </label>
              <label>
                进度标签
                <input bind:value={snapshotProgress} placeholder="例如：分歧前" />
              </label>
              <button type="button" on:click={createSaveSnapshot}>创建存档快照</button>
            </div>
          </section>
        </div>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">存档时间线</p>
              <h2>{workDetail.save_snapshots.length} 个恢复点</h2>
            </div>
          </div>
          <div class="inline-form">
            <input bind:value={restoreTargetPath} placeholder="恢复目标路径，留空使用默认存档路径" />
            <button type="button" class="secondary" on:click={chooseRestoreTarget}>选择恢复目录</button>
          </div>
          <div class="timeline-list">
            {#each workDetail.save_snapshots as snapshot}
              <article>
                <strong>{snapshot.progress_label ?? snapshot.note ?? '存档快照'}</strong>
                <span>
                  {formatDate(snapshot.created_at)} · {snapshot.route_name ?? '未设置路线'}
                  · {snapshot.is_locked ? '已锁定' : '未锁定'}
                </span>
                <p>{snapshot.snapshot_path}</p>
                <div class="row-actions snapshot-actions">
                  <button type="button" class="secondary" on:click={() => confirmRestoreSaveSnapshot(snapshot.id)}>
                    恢复到这个节点
                  </button>
                  <button
                    type="button"
                    class="secondary"
                    on:click={() => setSaveSnapshotLocked(snapshot.id, !snapshot.is_locked)}
                  >
                    {snapshot.is_locked ? '解除锁定' : '锁定'}
                  </button>
                  <button
                    type="button"
                    class="secondary danger-text"
                    disabled={snapshot.is_locked}
                    on:click={() => confirmDeleteSaveSnapshot(snapshot.id)}
                  >
                    删除
                  </button>
                </div>
              </article>
            {:else}
              <p class="muted">创建快照后，这里会形成存档时间线。</p>
            {/each}
          </div>
          {#if lastSaveSnapshot}
            <div class="notice">
              <strong>存档操作已完成</strong>
              <span>{lastSaveSnapshot.snapshot_path}</span>
            </div>
          {/if}
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">操作历史</p>
              <h2>这部作品最近的修改</h2>
            </div>
          </div>
          <div class="operation-list">
            {#each workDetail.recent_operations as operation}
              <article>
                <div>
                  <strong>{operation.summary}</strong>
                  <span>{formatDate(operation.created_at)} · {operation.entity_type}</span>
                </div>
                <details>
                  <summary>详情</summary>
                  <p>{operationDetail(operation)}</p>
                </details>
              </article>
            {:else}
              <p class="muted">这部作品还没有可展示的操作历史。</p>
            {/each}
          </div>
        </section>
      {:else}
        <div class="empty-state">
          <h3>还没有选中作品</h3>
          <p>从书架或整理工作台选择一部作品进入档案。</p>
        </div>
      {/if}
    {:else if activeView === 'timeline'}
      {#if pendingReceipts.length}
        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">游玩回执</p>
              <h2>{pendingReceipts.length} 条计时需要确认</h2>
            </div>
          </div>
          <div class="receipt-list">
            {#each pendingReceipts as session}
              <article>
                <div>
                  <strong>{session.title}</strong>
                  <span>
                    {formatDate(session.started_at)} · 估算 {formatDuration(session.duration_seconds)}
                    · 可信度 {Math.round(session.confidence * 100)}%
                  </span>
                </div>
                <label>
                  分钟数
                  <input
                    type="number"
                    min="1"
                    value={receiptDrafts[session.id]?.minutes ?? '1'}
                    on:input={(event) =>
                      updateReceiptDraft(session.id, 'minutes', event.currentTarget.value)}
                  />
                </label>
                <label>
                  一句笔记
                  <input
                    value={receiptDrafts[session.id]?.note ?? ''}
                    placeholder="可选，例如：推进到分歧前"
                    on:input={(event) =>
                      updateReceiptDraft(session.id, 'note', event.currentTarget.value)}
                  />
                </label>
                <div class="row-actions">
                  <button type="button" on:click={() => confirmReceipt(session.id)}>确认计入</button>
                  <button type="button" class="secondary" on:click={() => confirmDiscardReceipt(session.id)}>
                    误启动
                  </button>
                </div>
              </article>
            {/each}
          </div>
        </section>
      {/if}
      <section class="panel">
        <div class="section-heading">
          <div>
            <p class="eyebrow">时间线</p>
            <h2>{selectedItem ? displayTitle(selectedItem, privacyMode) : '选择一部作品'}</h2>
          </div>
        </div>
        {#if lastSession}
          <div class="notice">
            <strong>刚刚记录</strong>
            <span>{formatDuration(lastSession.duration_seconds)} · {lastSession.note ?? '无笔记'}</span>
          </div>
        {/if}
        <div class="timeline-list">
          {#each workDetail?.timeline_events ?? [] as event}
            <article class:spoiler={event.spoiler_level > 0}>
              <strong>{timelineEventLabel(event.event_type)} · {event.title}</strong>
              <span>{formatDate(event.occurred_at)}{event.detail ? ` · ${event.detail}` : ''}</span>
              {#if event.summary}<p>{event.summary}</p>{/if}
            </article>
          {:else}
            <p class="muted">时间线会混合游玩、存档和笔记。当前作品还没有记录。</p>
          {/each}
        </div>
      </section>
    {:else if activeView === 'settings'}
      <div class="content-grid two">
        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">数据安全</p>
              <h2>数据库快照</h2>
            </div>
          </div>
          <div class="inline-form stacked">
            <label>
              快照原因
              <input bind:value={snapshotReason} />
            </label>
            <button type="button" on:click={createSnapshot}>创建快照</button>
          </div>
          {#if lastSnapshot}
            <div class="notice">
              <strong>已创建快照</strong>
              <span>{lastSnapshot.path}</span>
            </div>
          {:else if today?.last_snapshot}
            <p class="muted">最近快照：{today.last_snapshot.path}</p>
          {:else}
            <p class="muted">还没有快照。批量导入和迁移前会自动创建保险点，手动快照也可以随时创建。</p>
          {/if}
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">实现状态</p>
              <h2>MVP base</h2>
            </div>
          </div>
          <ul class="check-list">
            <li>SQLite 本地库和 WAL 设置</li>
            <li>今日书桌聚合 API</li>
            <li>书架缓存列表</li>
            <li>扫描候选和确认入库</li>
            <li>手动计时和月度统计</li>
            <li>VACUUM INTO 数据库快照</li>
          </ul>
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">路径健康</p>
              <h2>安装位置诊断</h2>
            </div>
            <button type="button" on:click={checkPathHealth}>检查路径健康</button>
          </div>
          {#if pathHealthReport}
            <div class="metric-grid path-health-metrics">
              <div><strong>{pathHealthReport.checked_count}</strong><span>已检查</span></div>
              <div><strong>{pathHealthReport.available_count}</strong><span>可用安装</span></div>
              <div><strong>{pathHealthReport.missing_count}</strong><span>不可用</span></div>
            </div>
            <div class="path-issue-list">
              {#each pathHealthReport.issues as issue}
                <article>
                  <strong>{issue.title}</strong>
                  <span>{issue.root_path}</span>
                  {#if issue.executable_path}<small>{issue.executable_path}</small>{/if}
                </article>
              {:else}
                <p class="muted">所有安装位置当前都可用。</p>
              {/each}
            </div>
          {:else}
            <p class="muted">检查会刷新书架中的可用安装数量。移动硬盘离线或目录被移动时，这里会列出需要重新定位的作品。</p>
          {/if}
        </section>

        <section class="panel">
          <div class="section-heading">
            <div>
              <p class="eyebrow">操作历史</p>
              <h2>最近 {operations.length} 条修改</h2>
            </div>
          </div>
          <div class="operation-list">
            {#each operations as operation}
              <article>
                <div>
                  <strong>{operation.summary}</strong>
                  <span>{formatDate(operation.created_at)} · {operation.entity_type} · {operation.is_synced ? '已同步' : '本地'}</span>
                </div>
                <details>
                  <summary>详情</summary>
                  <p>{operationDetail(operation)}</p>
                </details>
              </article>
            {:else}
              <p class="muted">还没有操作历史。导入、编辑、计时和存档动作都会写入这里。</p>
            {/each}
          </div>
        </section>
      </div>
    {/if}
  </section>

  {#if confirmDialog}
    <div class="modal-backdrop" role="presentation">
      <div class="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
        <p class="eyebrow">需要确认</p>
        <h2 id="confirm-title">{confirmDialog.title}</h2>
        <p>{confirmDialog.message}</p>
        <div class="dialog-actions">
          <button type="button" class="secondary" on:click={() => (confirmDialog = null)}>取消</button>
          <button
            type="button"
            class:danger={confirmDialog.variant === 'danger'}
            on:click={confirmActiveAction}
          >
            {confirmDialog.actionLabel}
          </button>
        </div>
      </div>
    </div>
  {/if}
</main>
