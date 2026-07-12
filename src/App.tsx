import { useCallback, useEffect, useMemo, useRef, useState, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Contexts
import { UIProvider, IndexProvider, SearchProvider, useUIContext, useIndexContext, useSearchContext } from "./contexts";

// Hooks (cross-cutting — need multiple contexts)
import { useKeyboardShortcuts, useDocumentCategories } from "./hooks";
import { clearSearchCache } from "./hooks/useSearch";
import { useFileActions } from "./hooks/useFileActions";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAppEvents } from "./hooks/useAppEvents";
import { useWindowFocus } from "./hooks/useWindowFocus";
import { useUpdater } from "./hooks/useUpdater";
import { setupGlobalErrorHandlers } from "./utils/errorLogger";
import { getErrorMessage } from "./types/error";

// Components
import { Header, StatusBar, ErrorBanner, FloatingUI } from "./components/layout";
import { AutoIndexPrompt } from "./components/layout/AutoIndexPrompt";
import { SearchBar, SearchFilters, SearchResultList, CompactSearchBar } from "./components/search";
import { TypoSuggestion } from "./components/search/TypoSuggestion";
import SmartQueryInfo from "./components/search/SmartQueryInfo";
import { AiDisclaimerModal, isAiDisclaimerAccepted } from "./components/search/AiDisclaimerModal";
import { VectorIndexingBanner } from "./components/search/VectorIndexingBanner";
import { IndexingReportModal } from "./components/search/IndexingReportModal";
import { LazyMount } from "./components/LazyMount";
import { CommandPalette, type Command } from "./components/CommandPalette";
import {
  FolderPlus, Settings as SettingsIcon, HelpCircle, BarChart3, CopyCheck,
  PanelLeft, SunMoon, Home as HomeIcon, Download, Compass, Search as SearchIcon,
  Sparkles, FileText, MessageCircleQuestion, Bookmark, Clock,
} from "lucide-react";

// ── 코드 스플리팅 (P2-1) ──────────────────────────────
// 초기 렌더에 불필요한 무거운 컴포넌트는 lazy 로딩. PreviewPanel/AiAnswerPanel 은
// react-markdown + katex(~200kB) 의 유일 소비자라, 둘을 떼면 그 청크가 통째로
// 초기 번들에서 빠진다. 모달류는 LazyMount 로 첫 오픈 전까지 마운트 지연.
const AiAnswerPanel = lazy(() => import("./components/search/AiAnswerPanel"));
const PreviewPanel = lazy(() =>
  import("./components/search/PreviewPanel").then((m) => ({ default: m.PreviewPanel }))
);
const AppModals = lazy(() =>
  import("./components/layout/AppModals").then((m) => ({ default: m.AppModals }))
);
import type { HelpSection } from "./components/help/HelpModal";
const StatisticsModal = lazy(() =>
  import("./components/search/StatisticsModal").then((m) => ({ default: m.StatisticsModal }))
);
const DuplicateFinderModal = lazy(() =>
  import("./components/search/DuplicateFinderModal").then((m) => ({ default: m.DuplicateFinderModal }))
);
import { Sidebar } from "./components/sidebar";
import { ToastContainer } from "./components/ui/Toast";
import { OnboardingTour, resetOnboardingTour } from "./components/onboarding/OnboardingTour";
import { UpdateModal } from "./components/updater/UpdateModal";
import { DOCUFINDER_TOUR_STEPS, DOCUFINDER_TOUR_STORAGE_KEY } from "./components/onboarding/tourSteps";
import type { Settings } from "./types/settings";
import type { AddFolderResult } from "./types/index";
import type { SearchResult, SourceRef } from "./types/search";

// excludeFilename 활성 시 매 렌더 새 [] 참조가 생성되어 1,000줄짜리 SearchResultList의
// memo 비교를 깨는 것 방지 (frontend-debt-13) — 항상 같은 참조를 전달.
const EMPTY_RESULTS: SearchResult[] = [];

// ── App Shell (Provider 래핑) ──────────────────────────

function App() {
  return (
    <UIProvider>
      <IndexProvider>
        <SearchProvider>
          <AppContent />
        </SearchProvider>
      </IndexProvider>
    </UIProvider>
  );
}

// ── AppContent (cross-cutting 글루 + JSX) ──────────────

function AppContent() {
  const ui = useUIContext();
  const idx = useIndexContext();
  const search = useSearchContext();

  // 기능 투어 재시작 키 — 증가 시 투어 강제 재시작
  const [tourRunKey, setTourRunKey] = useState(0);
  const restartTour = useCallback(() => {
    resetOnboardingTour(DOCUFINDER_TOUR_STORAGE_KEY);
    setTourRunKey((k) => k + 1);
  }, []);

  // 명령 팔레트 (Cmd/Ctrl+K)
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);

  // ── App Settings (cross-cutting) ──
  const {
    viewDensity, semanticEnabled, vectorIndexingMode,
    resultsPerPage, openOnSingleClick, showResultPath, applySettings,
  } = useAppSettings({
    setSearchMode: search.setSearchMode,
    setMinConfidence: search.setMinConfidence,
  });

  // Document categories (cross-cutting: search results + settings)
  const categories = useDocumentCategories(search.filteredResults, semanticEnabled);

  // ── Preview Overlay 감지 (결과 영역 < 400px이면 overlay 전환) ──
  const contentFlexRef = useRef<HTMLDivElement>(null);
  const [previewOverlay, setPreviewOverlay] = useState(false);
  const MIN_RESULTS_WIDTH = 480;
  const MIN_PREVIEW_WIDTH = 380;

  useEffect(() => {
    const el = contentFlexRef.current;
    if (!el || !ui.previewFilePath) {
      setPreviewOverlay(false);
      return;
    }
    const check = () => {
      const pw = Math.max(ui.previewWidth, MIN_PREVIEW_WIDTH);
      setPreviewOverlay(el.clientWidth < pw + MIN_RESULTS_WIDTH);
    };
    check();
    const ro = new ResizeObserver(check);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ui.previewFilePath, ui.previewWidth]);

  // ── AI Disclaimer ──
  const [showAiDisclaimer, setShowAiDisclaimer] = useState(false);

  // ── 자동 업데이트 ──
  const updater = useUpdater(true);
  const [updateModalOpen, setUpdateModalOpen] = useState(false);
  // 업데이트 사용 가능 / 진행 중 / 재시작 대기 상태가 되면 모달 자동 열기
  useEffect(() => {
    if (
      updater.state.phase === "available" ||
      updater.state.phase === "ready-to-restart"
    ) {
      setUpdateModalOpen(true);
    }
  }, [updater.state.phase]);

  const executeAiQuery = useCallback(() => {
    search.askAi(search.query, search.filters.searchScope);
  }, [search.askAi, search.query, search.filters.searchScope]);

  // ── Submit handler (paradigm-aware) ──
  const handleSubmitQuery = useCallback(() => {
    if (search.paradigm === "question") {
      if (!isAiDisclaimerAccepted()) {
        setShowAiDisclaimer(true);
        return;
      }
      executeAiQuery();
    } else {
      search.submitNaturalQuery();
    }
  }, [search.paradigm, search.submitNaturalQuery, executeAiQuery]);

  // ── Anything 진입점: 현재 검색어 유지하며 Anything 모드로 전환 ──
  const handleSwitchToAnything = useCallback(() => {
    search.setParadigm("question");
  }, [search.setParadigm]);

  // ── 홈으로(검색 초기화) — Header + 명령 팔레트 공유 ──
  const handleGoHome = useCallback(() => {
    search.setQuery("");
    search.setSelectedIndex(-1);
    search.setParadigm("instant");
    search.resetAi();
    search.searchInputRef.current?.focus();
  }, [search.setQuery, search.setSelectedIndex, search.setParadigm, search.resetAi, search.searchInputRef]);

  // ── 안정 콜백 (frontend-debt-14) — SearchBar/CompactSearchBar memo가
  // 인라인 화살표 prop으로 매 렌더(토스트, 인덱싱 progress) 깨지지 않도록 useCallback 래핑 ──
  const handleCompositionStart = useCallback(() => search.setComposing(true), [search.setComposing]);
  const handleCompositionEnd = useCallback(
    (finalValue: string) => search.setComposing(false, finalValue),
    [search.setComposing]
  );
  const handleOpenSettings = useCallback(() => ui.setSettingsOpen(true), [ui.setSettingsOpen]);
  // 도움말 초기 탭 — 검색창 ? 버튼은 "search"(연산자 표)로 바로 연다.
  // 닫을 때 "start"로 리셋해 일반 경로(메뉴·팔레트)는 기존 동작 유지.
  const [helpSection, setHelpSection] = useState<HelpSection>("start");
  const handleOpenSearchHelp = useCallback(() => {
    setHelpSection("search");
    ui.setHelpOpen(true);
  }, [ui.setHelpOpen]);
  const handleHelpClose = useCallback(() => {
    ui.setHelpOpen(false);
    setHelpSection("start");
  }, [ui.setHelpOpen]);

  const handleOpenHelp = useCallback(() => ui.setHelpOpen(true), [ui.setHelpOpen]);
  const handleOpenUpdate = useCallback(() => setUpdateModalOpen(true), []);
  const handleSearchScopeChange = useCallback(
    (scope: string | null) => search.setFilters((prev) => ({ ...prev, searchScope: scope })),
    [search.setFilters]
  );

  // ── Sidebar memo 보존 (frontend-debt-12) ──
  // SearchContext의 handleSaveSmartFolder는 query를 useCallback deps로 가져 키스트로크마다
  // 참조가 바뀜 → ref로 최신 구현을 읽는 안정 래퍼로 전달 (useFileActions의 P2-2 queryRef 패턴과 동일 취지).
  const saveSmartFolderRef = useRef(search.handleSaveSmartFolder);
  useEffect(() => { saveSmartFolderRef.current = search.handleSaveSmartFolder; });
  const handleSaveSmartFolder = useCallback(() => saveSmartFolderRef.current(), []);

  // ── File Actions (cross-cutting) ──
  const {
    handleOpenFile, handleCopyPath, handleOpenFolder,
    handleAddFolder: rawHandleAddFolder,
    handleAddFolderByPath: rawHandleAddFolderByPath,
    handleRemoveFolder,
  } = useFileActions({
    query: search.query,
    addSearch: search.addSearch,
    showToast: ui.showToast,
    updateToast: ui.updateToast,
    addFolder: idx.addFolder,
    addFolderByPath: idx.addFolderByPath,
    removeFolder: idx.removeFolder,
    invalidateSearch: search.invalidateSearch,
    refreshVectorStatus: idx.refreshVectorStatus,
  });

  // ── Report helper ──
  // 결과(ui.reportResults)는 닫아도 유지 — StatusBar "실패 N건"으로 재열람 가능
  const [reportOpen, setReportOpen] = useState(false);
  const showReportIfNeeded = useCallback((results: AddFolderResult[]) => {
    const hasFailed = results.some((r) => r.failed_count > 0);
    if (hasFailed) {
      ui.setReportResults(results);
      setReportOpen(true);
    }
  }, [ui.setReportResults]);
  const reportFailedCount = useMemo(
    () => ui.reportResults.reduce((sum, r) => sum + r.failed_count, 0),
    [ui.reportResults],
  );
  const handleShowReport = useCallback(() => setReportOpen(true), []);

  const handleAddFolder = useCallback(async () => {
    const results = await rawHandleAddFolder();
    if (results) showReportIfNeeded(results);
    return results;
  }, [rawHandleAddFolder, showReportIfNeeded]);

  const handleAddFolderByPath = useCallback(async (path: string) => {
    await rawHandleAddFolderByPath(path);
  }, [rawHandleAddFolderByPath]);

  // ── Global setup effects ──
  useEffect(() => { setupGlobalErrorHandlers(); }, []);

  // 전역 우클릭 방지 (input/textarea는 허용 — 붙여넣기 등 네이티브 동작 보장)
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest("[data-context-menu]")) return;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      if (target.isContentEditable) return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", handler);
    return () => document.removeEventListener("contextmenu", handler);
  }, []);

  // 렌더 완료 후 창 표시
  useEffect(() => {
    const win = getCurrentWindow();
    win.isVisible().then((visible) => {
      if (visible) win.setFocus().catch(() => {});
    }).catch(() => {
      win.show();
      win.setFocus().catch(() => {});
    });
  }, []);

  // 폴더 0개 → 자동 인덱싱 안내 (첫 실행: AutoIndexPrompt + OnboardingTour)
  useEffect(() => {
    if (idx.status && idx.status.watched_folders.length === 0) {
      ui.tryShowAutoIndexPrompt();
    }
  }, [idx.status, ui.tryShowAutoIndexPrompt]);

  // ── Cross-cutting: 인덱싱 완료 → 캐시 무효화 ──
  const prevIndexPhaseRef = useRef(idx.progress?.phase);
  useEffect(() => {
    const phase = idx.progress?.phase;
    if (phase === "completed" && prevIndexPhaseRef.current !== "completed") {
      clearSearchCache();
      if (search.query.trim()) search.invalidateSearch();
    }
    prevIndexPhaseRef.current = phase;
  }, [idx.progress?.phase, search.query, search.invalidateSearch]);

  // 벡터 인덱싱 완료 → 토스트
  useEffect(() => {
    if (idx.vectorJustCompleted) {
      ui.showToast("시맨틱 검색 준비 완료!", "success");
      idx.clearVectorCompleted();
      clearSearchCache();
      if (search.query.trim()) search.invalidateSearch();
    }
  }, [idx.vectorJustCompleted, idx.clearVectorCompleted, ui.showToast, search.query, search.invalidateSearch]);

  // Tauri 이벤트 리스너
  useAppEvents({
    query: search.query,
    invalidateSearch: search.invalidateSearch,
    refreshStatus: idx.refreshStatus,
    refreshVectorStatus: idx.refreshVectorStatus,
    showToast: ui.showToast,
    updateToast: ui.updateToast,
  });

  // 윈도우 ���커스 → 검색창 포커스
  useWindowFocus(search.searchInputRef, ui.settingsOpen);

  // 에러 통합
  const error = search.searchError || idx.indexError || idx.vectorError;
  const clearError = useCallback(() => {
    search.clearSearchError();
    idx.clearIndexError();
    idx.clearVectorError();
  }, [search.clearSearchError, idx.clearIndexError, idx.clearVectorError]);

  // 북마크 선택 → 미리보기 + 파일 열기
  const handleBookmarkSelect = useCallback((filePath: string, pageNumber?: number | null) => {
    ui.setPreviewFilePath(filePath);
    handleOpenFile(filePath, pageNumber ?? undefined);
  }, [handleOpenFile, ui.setPreviewFilePath]);

  // 검증가능 인용 점프 — AI 답변 [출처N]/참조문서 클릭 → 인앱 미리보기에서 해당 위치로.
  // handleBookmarkSelect와 동일하게 useResultSelection 우회(인용 파일은 검색결과에 없을 수 있음).
  const [citationJump, setCitationJump] = useState<
    { filePath: string; anchors: string[]; page: number | null; token: number } | null
  >(null);
  const citeTokenRef = useRef(0);
  const handleCitationJump = useCallback((src: SourceRef) => {
    ui.setPreviewFilePath(src.file_path);
    citeTokenRef.current += 1;
    setCitationJump({
      filePath: src.file_path,
      anchors: src.anchors,
      page: src.page_number,
      token: citeTokenRef.current,
    });
  }, [ui.setPreviewFilePath]);

  // 미리보기 닫을 때 stale 인용 점프 제거 (닫았다 다시 열어도 과거 점프 재발 방지)
  const handlePreviewClose = useCallback(() => {
    setCitationJump(null);
    ui.handlePreviewClose();
  }, [ui.handlePreviewClose]);

  // 검색 결과 카드 클릭 → 미리보기를 해당 청크 위치로 점프 (인용 점프와 같은 배관).
  // content_preview 는 청크 앞부분 원문이라 findJumpTarget 의 텍스트 앵커로 쓸 수 있다.
  // 청크 시작부가 표/헤딩 경계에 걸려 렌더 텍스트와 어긋날 수 있어 뒤쪽 오프셋도
  // 앵커로 추가한다 (findJumpTarget 이 순서대로 재시도).
  const handleSelectResult = useCallback((index: number) => {
    search.setSelectedIndex(index);
    const r = search.filteredResults[index];
    const preview = r?.content_preview;
    if (preview) {
      citeTokenRef.current += 1;
      setCitationJump({
        filePath: r.file_path,
        anchors: [preview, preview.slice(40), preview.slice(80)].filter((s) => s.length >= 6),
        page: r.page_number,
        token: citeTokenRef.current,
      });
    }
  }, [search.setSelectedIndex, search.filteredResults]);

  // ── 0건 제안 칩 콜백 ──
  // 자연어 필터(날짜/파일타입/제외)를 떼고 파싱된 키워드만 즉시 모드로 재검색
  const handleRetryWithoutFilters = useCallback(() => {
    const kw = search.parsedQuery?.keywords?.trim();
    if (!kw) return;
    search.setParadigm("instant");
    search.handleSelectSearch(kw);
  }, [search.parsedQuery, search.setParadigm, search.handleSelectSearch]);

  const handleFocusSearch = useCallback(() => {
    const el = search.searchInputRef.current;
    el?.focus();
    el?.select();
  }, [search.searchInputRef]);

  // 내용 검색 0건 → 파일명 검색으로 전환 (searchMode 변경이 디바운스 재검색을 트리거)
  const handleSwitchToFilenameSearch = useCallback(() => {
    search.setParadigm("instant");
    search.setSearchMode("filename");
  }, [search.setParadigm, search.setSearchMode]);

  // ── Keyboard Shortcuts ──
  useKeyboardShortcuts(
    {
      onFocusSearch: () => {
        const compact = search.compactSearchInputRef.current;
        const main = search.searchInputRef.current;
        const target = compact && compact.offsetParent !== null ? compact : main;
        target?.focus();
        // 질문 모드(textarea, 긴 입력)에서 전체 선택 시 실수 타이핑으로 내용 유실 위험 → 포커스만
        if (target && target.tagName !== "TEXTAREA") {
          (target as HTMLInputElement).select();
        }
      },
      onCommandPalette: () => setCommandPaletteOpen((o) => !o),
      onEscape: () => {
        // 우선순위: 프리뷰 닫기 → 선택 해제 → 검색어 삭제 (ux-audit-11 — Esc는 가장 위 레이어부터).
        // 북마크/AI 인용으로 연 프리뷰(selectedIndex=-1)에서 검색어가 날아가는 문제 방지.
        if (ui.previewFilePath) {
          handlePreviewClose();
        } else if (search.selectedIndex >= 0) {
          search.setSelectedIndex(-1);
        } else {
          search.setQuery("");
          search.searchInputRef.current?.blur();
        }
      },
      onToggleSidebar: ui.toggleSidebar,
      onArrowUp: () => {
        if (search.selectedIndex <= 0) {
          // -1 또는 0이면: 선택 해제 → 검색창 포커스
          search.setSelectedIndex(-1);
          search.searchInputRef.current?.focus();
        } else {
          search.setSelectedIndex(search.selectedIndex - 1);
        }
      },
      onArrowDown: () => search.setSelectedIndex(Math.min(search.filteredResults.length - 1, search.selectedIndex + 1)),
      onEnter: () => {
        if (search.selectedIndex >= 0 && search.selectedIndex < search.filteredResults.length) {
          const r = search.filteredResults[search.selectedIndex];
          handleOpenFile(r.file_path, r.page_number);
        }
      },
      // 검색 입력 포커스 중 Enter (ux-audit-1): "타이핑 → ↓ 선택 → Enter로 열기" 동선 복원.
      // - instant 패러다임 한정 — natural/question은 Enter=제출이라 가로채면 안 됨
      // - 검색 입력(메인/컴팩트)에서만 동작 — 다른 입력(태그, 프리셋 이름 등)의 Enter는 그대로 둠
      onEnterInInput: () => {
        if (search.paradigm !== "instant") return false;
        const active = document.activeElement;
        if (active !== search.searchInputRef.current && active !== search.compactSearchInputRef.current) {
          return false;
        }
        if (search.selectedIndex >= 0 && search.selectedIndex < search.filteredResults.length) {
          const r = search.filteredResults[search.selectedIndex];
          handleOpenFile(r.file_path, r.page_number);
          return true;
        }
        return false;
      },
      onCopy: () => {
        if (search.selectedIndex >= 0 && search.selectedIndex < search.filteredResults.length) {
          handleCopyPath(search.filteredResults[search.selectedIndex].file_path);
        }
      },
    },
    search.searchInputRef
  );

  // ── Settings callbacks ──
  const handleSettingsClose = useCallback(() => {
    ui.setSettingsOpen(false);
    requestAnimationFrame(() => search.searchInputRef.current?.focus());
  }, [ui.setSettingsOpen, search.searchInputRef]);

  const handleSettingsSaved = useCallback((settings: Settings) => {
    const wasEnabled = semanticEnabled;
    const wasAutoMode = vectorIndexingMode === "auto";
    applySettings(settings);
    clearSearchCache();
    // 토스트 없음 — 설정 모달이 자체 '저장됨' 인디케이터 표시 (자동저장 토스트 스팸 방지)
    const nowEnabled = settings.semantic_search_enabled ?? false;
    const nowAutoMode = (settings.vector_indexing_mode ?? "manual") === "auto";
    if (idx.isVectorIndexing && (!nowEnabled || !nowAutoMode)) {
      idx.cancelVectorIndexing();
    }
    if (nowEnabled && nowAutoMode && !idx.isVectorIndexing && (!wasEnabled || !wasAutoMode)) {
      idx.refreshVectorStatus().then((freshStatus) => {
        if (!ui.isMountedRef.current) return;
        if ((freshStatus?.pending_chunks ?? 0) > 0) idx.startVectorIndexing();
      }).catch(() => {});
    }
  }, [applySettings, semanticEnabled, vectorIndexingMode, idx.isVectorIndexing, idx.cancelVectorIndexing, idx.refreshVectorStatus, idx.startVectorIndexing, ui.isMountedRef]);

  const handleResumeIndexing = useCallback(async () => {
    if (idx.cancelledFolderPath) {
      try {
        await invoke("resume_indexing", { path: idx.cancelledFolderPath });
        idx.refreshStatus();
      } catch {
        ui.showToast("인덱싱 재시작 실패", "error");
      }
    }
  }, [idx.cancelledFolderPath, idx.refreshStatus, ui.showToast]);

  const handleClearData = useCallback(async () => {
    try {
      await invoke("clear_all_data");
      clearSearchCache();
      await Promise.all([idx.refreshStatus(), idx.refreshVectorStatus()]);
      ui.showToast("모든 인덱스 데이터가 초기화되었습니다", "success");
    } catch (err) {
      ui.showToast(`초기화 실패: ${getErrorMessage(err)}`, "error");
      throw err;
    }
  }, [idx, ui]);

  // ── 명령 팔레트 커맨드 목록 ──
  const commands = useMemo<Command[]>(() => {
    const ico = "w-4 h-4";
    const list: Command[] = [
      // 작업
      { id: "add-folder", group: "작업", label: "폴더 추가", keywords: "folder add 인덱싱 index", icon: <FolderPlus className={ico} />, run: () => { handleAddFolder(); } },
      { id: "duplicates", group: "작업", label: "중복 문서 찾기", keywords: "duplicate 중복", icon: <CopyCheck className={ico} />, run: () => ui.setDuplicateOpen(true) },
      { id: "stats", group: "작업", label: "통계 보기", keywords: "statistics 통계 차트", icon: <BarChart3 className={ico} />, run: () => ui.setStatsOpen(true) },
      { id: "home", group: "작업", label: "홈으로 (검색 초기화)", keywords: "home reset 초기화", icon: <HomeIcon className={ico} />, run: handleGoHome },
      // 검색 모드
      { id: "mode-keyword", group: "검색 모드", label: "키워드 검색", hint: search.searchMode === "keyword" ? "현재" : undefined, keywords: "keyword 키워드", icon: <SearchIcon className={ico} />, run: () => { search.setParadigm("instant"); search.setSearchMode("keyword"); } },
      ...(semanticEnabled
        ? [{ id: "mode-hybrid", group: "검색 모드", label: "하이브리드 검색 (시맨틱)", hint: search.searchMode === "hybrid" ? "현재" : undefined, keywords: "hybrid semantic 시맨틱 의미", icon: <Sparkles className={ico} />, run: () => { search.setParadigm("instant"); search.setSearchMode("hybrid"); } } as Command]
        : []),
      { id: "mode-filename", group: "검색 모드", label: "파일명 검색", hint: search.searchMode === "filename" ? "현재" : undefined, keywords: "filename 파일명", icon: <FileText className={ico} />, run: () => { search.setParadigm("instant"); search.setSearchMode("filename"); } },
      { id: "mode-ai", group: "검색 모드", label: "AI에게 질문", keywords: "ai question 질문 anything", icon: <MessageCircleQuestion className={ico} />, run: () => search.setParadigm("question") },
      // 보기
      { id: "sidebar", group: "보기", label: "사이드바 토글", keywords: "sidebar 사이드바", icon: <PanelLeft className={ico} />, run: ui.toggleSidebar },
      { id: "theme", group: "보기", label: "테마 전환 (라이트 ↔ 다크)", keywords: "theme dark light 다크 라이트 어둡게 밝게", icon: <SunMoon className={ico} />, run: () => { const isDark = document.documentElement.classList.contains("dark"); ui.setTheme(isDark ? "light" : "dark"); } },
      // 설정/기타
      { id: "settings", group: "설정", label: "설정 열기", keywords: "settings 설정 환경", icon: <SettingsIcon className={ico} />, run: () => ui.setSettingsOpen(true) },
      { id: "help", group: "설정", label: "도움말", keywords: "help 도움말 단축키", icon: <HelpCircle className={ico} />, run: () => ui.setHelpOpen(true) },
      { id: "tour", group: "설정", label: "기능 둘러보기 다시 보기", keywords: "tour 투어 둘러보기 가이드", icon: <Compass className={ico} />, run: restartTour },
      { id: "update", group: "설정", label: "업데이트 확인", keywords: "update 업데이트", icon: <Download className={ico} />, run: () => setUpdateModalOpen(true) },
    ];
    // 동적: 스마트 폴더
    for (const f of search.smartFolders) {
      list.push({ id: `sf-${f.id}`, group: "스마트 폴더", label: f.name, hint: f.query, keywords: `${f.name} ${f.query}`, icon: <Bookmark className={ico} />, run: () => search.handleApplySmartFolder(f) });
    }
    // 동적: 최근 검색 (최대 6)
    for (const r of search.recentSearches.slice(0, 6)) {
      list.push({ id: `rs-${r.timestamp}`, group: "최근 검색", label: r.query, keywords: r.query, icon: <Clock className={ico} />, run: () => search.handleSelectSearch(r.query) });
    }
    return list;
  }, [
    handleAddFolder, handleGoHome, restartTour, semanticEnabled, search.searchMode,
    search.smartFolders, search.recentSearches, search.setParadigm, search.setSearchMode,
    search.handleApplySmartFolder, search.handleSelectSearch,
    ui.setDuplicateOpen, ui.setStatsOpen, ui.toggleSidebar, ui.setTheme, ui.setSettingsOpen, ui.setHelpOpen,
  ]);

  // ── Render ──

  return (
    <div className="h-screen mx-auto relative overflow-hidden" style={{ backgroundColor: 'var(--color-bg-primary)', color: 'var(--color-text-primary)', maxWidth: '1920px' }}>
      {/* Skip-to-main-content for keyboard/screen reader users */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-[10000] focus:px-4 focus:py-2 focus:rounded-lg focus:text-sm focus:font-medium focus:shadow-lg"
        style={{ backgroundColor: 'var(--color-accent)', color: '#fff' }}
      >
        본문으로 건너뛰기
      </a>
      <div className="noise-overlay" aria-hidden="true" />

      <Sidebar
        isOpen={ui.sidebarOpen}
        onToggle={ui.toggleSidebar}
        watchedFolders={idx.status?.watched_folders ?? []}
        onAddFolder={handleAddFolder}
        onAddFolderByPath={handleAddFolderByPath}
        onRemoveFolder={handleRemoveFolder}
        isIndexing={idx.isIndexing}
        isAutoIndexing={idx.isAutoIndexing}
        onFoldersChange={idx.refreshStatus}
        recentSearches={search.recentSearches}
        onSelectSearch={search.handleSelectSearch}
        onRemoveSearch={search.removeSearch}
        onClearSearches={search.clearSearches}
        smartFolders={search.smartFolders}
        onApplySmartFolder={search.handleApplySmartFolder}
        onRemoveSmartFolder={search.removeSmartFolder}
        onSaveSmartFolder={handleSaveSmartFolder}
        // Sidebar는 currentQuery를 truthiness 판정에만 사용(스마트폴더 저장 버튼 노출, Sidebar.tsx:244).
        // 라이브 쿼리 문자열을 그대로 넘기면 키스트로크마다 Sidebar memo가 깨지므로(frontend-debt-12)
        // 쿼리 존재 여부만 안정 sentinel 값으로 전달.
        currentQuery={search.query.trim() ? "*" : ""}
        bookmarks={ui.bookmarks}
        onBookmarkSelect={handleBookmarkSelect}
        onBookmarkRemove={ui.removeBookmark}
        batch={idx.batch}
        onCancelBatch={idx.cancelBatch}
        onDismissBatch={idx.dismissBatch}
      />

      <div
        className="flex flex-col h-full transition-all duration-200 ease-out"
        style={{ paddingLeft: ui.sidebarOpen ? "var(--sidebar-width)" : "var(--sidebar-collapsed-width)" }}
      >
        {/* Compact Search Bar */}
        {search.isCollapsed && (
          <div className="sticky top-0 z-30 bg-[var(--color-bg-primary)]/95 backdrop-blur-md">
            <CompactSearchBar
              ref={search.compactSearchInputRef}
              query={search.query}
              onQueryChange={search.handleQueryChange}
              onCompositionStart={handleCompositionStart}
              onCompositionEnd={handleCompositionEnd}
              searchMode={search.searchMode}
              onSearchModeChange={search.setSearchMode}
              isLoading={search.isLoading}
              status={idx.status}
              resultCount={search.filteredResults.length}
              onExpand={search.handleExpand}
              onAddFolder={handleAddFolder}
              onOpenSettings={handleOpenSettings}
              onOpenHelp={handleOpenHelp}
              isIndexing={idx.isIndexing}
              isSidebarOpen={ui.sidebarOpen}
              filters={search.filters}
              onFiltersChange={search.setFilters}
              viewMode={search.viewMode}
              onViewModeChange={search.setViewMode}
              refineQuery={search.refineQuery}
              onRefineQueryChange={search.setRefineQuery}
              onRefineQueryClear={search.clearRefine}
              totalResultCount={search.results.length}
              paradigm={search.paradigm}
              onParadigmChange={search.setParadigm}
              onSubmitNatural={handleSubmitQuery}
              updatePhase={updater.state.phase}
              onOpenUpdate={handleOpenUpdate}
            />
          </div>
        )}

        {/* Expanded Header */}
        {!search.isCollapsed && (
          <div className="sticky top-0 z-20 bg-[var(--color-bg-primary)]/90 backdrop-blur-md border-b" style={{ borderColor: 'var(--color-border)' }}>
            <Header
              onAddFolder={handleAddFolder}
              onOpenSettings={() => ui.setSettingsOpen(true)}
              onOpenHelp={() => ui.setHelpOpen(true)}
              onOpenStats={() => ui.setStatsOpen(true)}
              onOpenDuplicates={() => ui.setDuplicateOpen(true)}
              onGoHome={handleGoHome}
              isIndexing={idx.isIndexing}
              isSidebarOpen={ui.sidebarOpen}
              hasQuery={search.query.length > 0}
              updatePhase={updater.state.phase}
              onOpenUpdate={() => setUpdateModalOpen(true)}
            />
          </div>
        )}

        {/* Search Bar (expanded only) */}
        {!search.isCollapsed && (
          <div className="px-4 pt-4 pb-2">
            <SearchBar
              ref={search.searchInputRef}
              query={search.query}
              onQueryChange={search.handleQueryChange}
              onCompositionStart={handleCompositionStart}
              onCompositionEnd={handleCompositionEnd}
              isLoading={search.isLoading}
              resultCount={search.filteredResults.length}
              searchTime={search.searchTime}
              paradigm={search.paradigm}
              onParadigmChange={search.setParadigm}
              hasIndex={(idx.status?.indexed_files ?? 0) > 0}
              onSubmitNatural={handleSubmitQuery}
              watchedFolders={idx.status?.watched_folders ?? []}
              searchScope={search.filters.searchScope}
              onSearchScopeChange={handleSearchScopeChange}
              onOpenSearchHelp={handleOpenSearchHelp}
            />

            <VectorIndexingBanner
              isVisible={idx.isVectorIndexing}
              progress={idx.vectorProgress}
              onCancel={idx.cancelVectorIndexing}
            />

            {search.typoSuggestion && (
              <div className="mt-1.5">
                <TypoSuggestion
                  suggestions={search.typoSuggestion.suggestions}
                  onAccept={(word) => { search.setQuery(word); search.dismissTypo(); }}
                  onDismiss={search.dismissTypo}
                />
              </div>
            )}

            {error && (
              <div className="mt-3">
                <ErrorBanner
                  message={error}
                  onDismiss={clearError}
                  onRetry={search.query.trim() ? () => { clearError(); search.invalidateSearch(); } : undefined}
                />
              </div>
            )}
          </div>
        )}

        {/* Filter bar — 스크롤 중에도 항상 보이도록 top-level (flex-col 에서 scroll 영역 위에 고정).
            `relative z-40` 로 stacking level 확보 — 하위 CustomSelect 드롭다운이 결과 카드
            (transform 애니메이션으로 자체 stacking context 생성) 위로 뜨도록 한다. */}
        {search.paradigm !== "question" && search.query && (search.results.length > 0 || search.filenameResults.length > 0) && (
          <div
            className={`${search.isCollapsed ? "px-4 pt-2" : "px-4"} pb-2 border-b bg-[var(--color-bg-primary)]/95 backdrop-blur-md relative z-40`}
            style={{ borderColor: "var(--color-border)" }}
          >
            {search.paradigm === "natural" && search.parsedQuery ? (
              <SmartQueryInfo parsed={search.parsedQuery} onClear={() => search.submitNaturalQuery()} />
            ) : (
              <SearchFilters
                filters={search.filters}
                onFiltersChange={search.setFilters}
                showRefineSearch={search.results.length > 0 || search.filenameResults.length > 0}
                searchMode={search.searchMode}
                onSearchModeChange={search.setSearchMode}
                refineQuery={search.refineQuery}
                onRefineQueryChange={search.setRefineQuery}
                onRefineQueryClear={search.clearRefine}
                watchedFolders={idx.status?.watched_folders ?? []}
                keywordMatchMode={search.keywordMatchMode}
                onKeywordMatchModeChange={search.setKeywordMatchMode}
                presets={search.presets}
                onSavePreset={search.handleSavePreset}
                onApplyPreset={search.handleApplyPreset}
                onRemovePreset={search.removePreset}
              />
            )}
          </div>
        )}

        {/* Scrollable Content + Preview */}
        <div ref={contentFlexRef} className="flex-1 flex overflow-hidden relative">
          <div
            ref={search.scrollContainerRef}
            onScroll={search.handleScroll}
            className="flex-1 overflow-y-auto overflow-x-hidden results-scroll"
            style={{ overflowAnchor: "none" }}
          >
            {search.isCollapsed && error && (
              <div className="px-6 pt-2">
                <ErrorBanner
                  message={error}
                  onDismiss={clearError}
                  onRetry={search.query.trim() ? () => { clearError(); search.invalidateSearch(); } : undefined}
                />
              </div>
            )}

            <main id="main-content" tabIndex={-1} className={`h-full outline-none ${search.paradigm === "question" ? "px-2 sm:px-4 pb-4" : "px-5 sm:px-8 pb-20"}`}>
              <div className={`h-full ${(search.paradigm === "question" || ui.previewFilePath) ? "content-column" : ""} ${search.paradigm === "question" ? "mt-1" : "mt-4"}`}>
                {/* 유사 문서 배너 */}
                {search.similarResults.length > 0 && (
                  <div className="mb-4 p-3 rounded-lg border" style={{ backgroundColor: "var(--color-bg-secondary)", borderColor: "var(--color-border)" }}>
                    <div className="flex items-center justify-between mb-2">
                      <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">
                        "{search.similarSourceFile}"와 유사한 문서 ({search.similarResults.length}건)
                      </h3>
                      <button onClick={search.clearSimilarResults} className="text-xs px-2 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)]">닫기</button>
                    </div>
                    <div className="space-y-1">
                      {search.similarResults.slice(0, 10).map((r, i) => (
                        <div
                          key={`sim-${i}`}
                          className="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-[var(--color-bg-tertiary)] cursor-pointer transition-colors"
                          onClick={() => handleOpenFile(r.file_path, r.page_number)}
                        >
                          <span className="text-xs font-mono text-[var(--color-text-muted)] w-6 text-right">{r.confidence}%</span>
                          <span className="text-sm truncate text-[var(--color-text-primary)]">{r.file_name}</span>
                          <span className="text-[10px] text-[var(--color-text-muted)] truncate ml-auto max-w-[200px]">{r.content_preview?.slice(0, 80)}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {search.paradigm === "question" ? (
                  <Suspense fallback={null}>
                    <AiAnswerPanel
                      answer={search.aiAnswer}
                      isStreaming={search.isAiStreaming}
                      analysis={search.aiAnalysis}
                      error={search.aiError}
                      onReset={search.resetAi}
                      currentQuestion={search.aiAskedQuery}
                      onCite={handleCitationJump}
                      onExampleClick={(text) => {
                        search.setQuery(text);
                        if (!isAiDisclaimerAccepted()) {
                          setShowAiDisclaimer(true);
                        } else {
                          search.askAi(text, search.filters.searchScope);
                        }
                      }}
                    />
                  </Suspense>
                ) : (
                  <SearchResultList
                    results={search.filteredResults}
                    filenameResults={search.filters.excludeFilename ? EMPTY_RESULTS : search.filenameResults}
                    groupedResults={search.groupedResults}
                    viewMode={search.viewMode}
                    onViewModeChange={search.setViewMode}
                    viewDensity={viewDensity}
                    query={search.query}
                    highlightQuery={search.searchedQuery}
                    isLoading={search.isLoading}
                    selectedIndex={search.selectedIndex}
                    onOpenFile={handleOpenFile}
                    onCopyPath={handleCopyPath}
                    onOpenFolder={handleOpenFolder}
                    onExportCSV={search.handleExportCSV}
                    onCopyAll={search.handleCopyAll}
                    refineKeywords={search.memoizedRefineKeywords}
                    resultCount={search.filteredResults.length}
                    totalResultCount={search.results.length}
                    minConfidence={search.minConfidence}
                    searchTime={search.searchTime}
                    resultsPerPage={resultsPerPage}
                    indexedFiles={idx.status?.indexed_files ?? 0}
                    indexedFolders={idx.status?.watched_folders?.length ?? 0}
                    recentSearches={search.recentSearches}
                    onSelectSearch={search.handleSelectSearch}
                    onRemoveSearch={search.removeSearch}
                    semanticEnabled={semanticEnabled}
                    onAddFolder={handleAddFolder}
                    onSelectResult={handleSelectResult}
                    onFindSimilar={semanticEnabled ? search.handleFindSimilar : undefined}
                    categories={categories}
                    paradigm={search.paradigm}
                    nlSubmitted={search.nlSubmitted}
                    parsedQuery={search.parsedQuery}
                    onSwitchToAnything={handleSwitchToAnything}
                    isIndexing={idx.isIndexing}
                    indexProgress={idx.progress}
                    searchMode={search.searchMode}
                    onRetryWithoutFilters={handleRetryWithoutFilters}
                    onFocusSearch={handleFocusSearch}
                    onSwitchToFilenameSearch={handleSwitchToFilenameSearch}
                    openOnSingleClick={openOnSingleClick}
                    showResultPath={showResultPath}
                    onPreviewFile={ui.setPreviewFilePath}
                  />
                )}
              </div>
            </main>
          </div>

          {/* Preview Panel — push(넓은 창) / overlay(좁은 창) 자동 전환 */}
          {ui.previewFilePath && !previewOverlay && (
            <>
              <div
                onMouseDown={ui.handleResizeStart}
                className="w-1 shrink-0 cursor-col-resize hover:bg-[var(--color-accent)] transition-colors group relative"
                style={{ backgroundColor: "var(--color-border)" }}
                title="드래그하여 너비 조절"
              >
                <div className="absolute inset-y-0 -left-1 -right-1" />
              </div>
              <div className="shrink-0" style={{ width: Math.max(ui.previewWidth, MIN_PREVIEW_WIDTH), minWidth: MIN_PREVIEW_WIDTH, maxWidth: '50%' }}>
                <Suspense fallback={null}>
                  <PreviewPanel
                    filePath={ui.previewFilePath}
                    highlightQuery={search.searchedQuery}
                    jumpTarget={citationJump && citationJump.filePath === ui.previewFilePath ? citationJump : undefined}
                    onClose={handlePreviewClose}
                    onOpenFile={handleOpenFile}
                    onCopyPath={handleCopyPath}
                    onOpenFolder={handleOpenFolder}
                    onBookmark={ui.addBookmark}
                    isBookmarked={ui.isBookmarked(ui.previewFilePath)}
                    tags={ui.previewTags}
                    tagSuggestions={ui.tagSuggestions}
                    onAddTag={ui.handleAddTag}
                    onRemoveTag={ui.handleRemoveTag}
                  />
                </Suspense>
              </div>
            </>
          )}
          {ui.previewFilePath && previewOverlay && (
            <>
              <div
                className="absolute inset-0 z-40 bg-black/15 animate-fade-in"
                onClick={handlePreviewClose}
              />
              <div
                className="absolute right-0 top-0 bottom-0 z-50 shadow-2xl preview-slide-in"
                style={{ width: Math.max(Math.min(ui.previewWidth, (contentFlexRef.current?.clientWidth ?? 600) * 0.85), MIN_PREVIEW_WIDTH), minWidth: MIN_PREVIEW_WIDTH }}
              >
                {/* overlay 모드에도 리사이즈 핸들 — push 에서 넓히다 overlay 로 전환되면
                    핸들이 사라져 다시 줄일 수 없던 버그 수정. 좁히면 push 모드로 자동 복귀. */}
                <div
                  onMouseDown={ui.handleResizeStart}
                  className="absolute inset-y-0 left-0 w-1 z-10 cursor-col-resize hover:bg-[var(--color-accent)] transition-colors"
                  style={{ backgroundColor: "var(--color-border)" }}
                  title="드래그하여 너비 조절"
                >
                  <div className="absolute inset-y-0 -left-1 -right-1" />
                </div>
                <Suspense fallback={null}>
                  <PreviewPanel
                    filePath={ui.previewFilePath}
                    highlightQuery={search.searchedQuery}
                    jumpTarget={citationJump && citationJump.filePath === ui.previewFilePath ? citationJump : undefined}
                    onClose={handlePreviewClose}
                    onOpenFile={handleOpenFile}
                    onCopyPath={handleCopyPath}
                    onOpenFolder={handleOpenFolder}
                    onBookmark={ui.addBookmark}
                    isBookmarked={ui.isBookmarked(ui.previewFilePath)}
                    tags={ui.previewTags}
                    tagSuggestions={ui.tagSuggestions}
                    onAddTag={ui.handleAddTag}
                    onRemoveTag={ui.handleRemoveTag}
                  />
                </Suspense>
              </div>
            </>
          )}
        </div>

        <StatusBar
          status={idx.status}
          progress={idx.progress}
          batch={idx.batch}
          onCancelIndexing={idx.cancelIndexing}
          onCancelBatch={idx.cancelBatch}
          onResumeIndexing={handleResumeIndexing}
          hasCancelledFolders={!!idx.cancelledFolderPath}
          failedCount={reportFailedCount}
          onShowReport={handleShowReport}
        />
      </div>

      <AiDisclaimerModal
        isOpen={showAiDisclaimer}
        onAccept={() => {
          setShowAiDisclaimer(false);
          executeAiQuery();
        }}
        onDecline={() => setShowAiDisclaimer(false)}
      />
      <LazyMount active={ui.settingsOpen || ui.helpOpen}>
        <AppModals
          settingsOpen={ui.settingsOpen}
          onSettingsClose={handleSettingsClose}
          onThemeChange={ui.setTheme}
          onSettingsSaved={handleSettingsSaved}
          onClearData={handleClearData}
          onAutoIndexAllDrives={idx.autoIndexAllDrives}
          helpOpen={ui.helpOpen}
          onHelpClose={handleHelpClose}
          onRestartTour={restartTour}
          helpInitialSection={helpSection}
        />
      </LazyMount>
      <ToastContainer toasts={ui.toasts} onDismiss={ui.dismissToast} />
      <IndexingReportModal
        isOpen={reportOpen && ui.reportResults.length > 0}
        onClose={() => setReportOpen(false)}
        results={ui.reportResults}
      />

      <LazyMount active={ui.statsOpen}>
        <StatisticsModal
          isOpen={ui.statsOpen}
          onClose={() => ui.setStatsOpen(false)}
          onFilterByType={(fileType) => {
            const typeMap: Record<string, import("./types/search").FileTypeFilter> = {
              hwpx: "hwpx", hwp: "hwpx", docx: "docx", doc: "docx",
              pptx: "pptx", ppt: "pptx", xlsx: "xlsx", xls: "xlsx",
              pdf: "pdf", txt: "txt", md: "txt",
            };
            const ft = typeMap[fileType];
            search.setFilters((prev) => ({ ...prev, fileTypes: ft ? [ft] : [] }));
            if (!search.query) search.setQuery("*");
          }}
          onOpenFile={handleOpenFile}
          onSearchQuery={search.handleSelectSearch}
        />
      </LazyMount>

      <LazyMount active={ui.duplicateOpen}>
        <DuplicateFinderModal
          isOpen={ui.duplicateOpen}
          onClose={() => ui.setDuplicateOpen(false)}
          onOpenFile={handleOpenFile}
          onOpenFolder={handleOpenFolder}
          showToast={ui.showToast}
        />
      </LazyMount>


      <AutoIndexPrompt
        isOpen={ui.showAutoIndexPrompt}
        onClose={() => ui.setShowAutoIndexPrompt(false)}
        onAutoIndex={idx.autoIndexAllDrives}
        onSelectFolder={handleAddFolder}
        onIndexFolderByPath={handleAddFolderByPath}
      />

      <FloatingUI
        showScrollTop={search.showScrollTop}
        onScrollToTop={search.scrollToTop}
      />

      <UpdateModal
        isOpen={updateModalOpen}
        onClose={() => {
          setUpdateModalOpen(false);
          updater.dismiss();
        }}
        state={updater.state}
        onInstall={updater.downloadAndInstall}
        onRestart={updater.restart}
        onCancel={updater.cancel}
        onOpenReleasePage={updater.openReleasePage}
      />

      {/* 기능 투어 — 온보딩 단일 흐름: AutoIndexPrompt가 닫히고 + 폴더가 실제 추가된 뒤에만 1회 자동 시작.
          '나중에 할게요'(폴더 0개)면 투어를 띄우지 않아 모달→투어 중복 안내를 제거. 헬프 메뉴에서 재시작 가능. */}
      <OnboardingTour
        steps={DOCUFINDER_TOUR_STEPS}
        storageKey={DOCUFINDER_TOUR_STORAGE_KEY}
        autoStart={!ui.showAutoIndexPrompt && (idx.status?.watched_folders?.length ?? 0) > 0}
        runKey={tourRunKey}
      />

      <CommandPalette
        isOpen={commandPaletteOpen}
        onClose={() => setCommandPaletteOpen(false)}
        commands={commands}
      />
    </div>
  );
}

export default App;
