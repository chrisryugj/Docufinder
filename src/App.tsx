import { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Hooks
import { useSearch, useIndexStatus, useVectorIndexing, useKeyboardShortcuts, useRecentSearches, useExport, useToast, useTheme, useCollapsibleSearch, useAutoComplete } from "./hooks";
import { clearSearchCache } from "./hooks/useSearch";
import { useFirstRun } from "./hooks/useFirstRun";
import { useFileActions } from "./hooks/useFileActions";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAiSearch } from "./hooks/useAiSearch";
import { useAppEvents } from "./hooks/useAppEvents";
import { useUpdater } from "./hooks/useUpdater";
import { useBookmarks } from "./hooks/useBookmarks";
import { useWindowFocus } from "./hooks/useWindowFocus";
import { useSimilarDocuments } from "./hooks/useSimilarDocuments";
import { useDocumentCategories } from "./hooks/useDocumentCategories";
import { useRecentSearchSaver } from "./hooks/useRecentSearchSaver";
import { useResultSelection } from "./hooks/useResultSelection";
import { setupGlobalErrorHandlers, logToBackend } from "./utils/errorLogger";

// Components
import { Header, StatusBar, ErrorBanner, AppModals, FloatingUI } from "./components/layout";
import { FloatingErrorBanner } from "./components/layout/FloatingErrorBanner";
import { AutoIndexPrompt } from "./components/layout/AutoIndexPrompt";
import { SearchBar, SearchFilters, SearchResultList, CompactSearchBar } from "./components/search";
import SmartQueryInfo from "./components/search/SmartQueryInfo";
import { AiAnswerPanel } from "./components/search/AiAnswerPanel";
import { VectorIndexingBanner } from "./components/search/VectorIndexingBanner";
import { PreviewPanel } from "./components/search/PreviewPanel";
import { IndexingReportModal } from "./components/search/IndexingReportModal";
import { StatisticsModal } from "./components/search/StatisticsModal";
import { DuplicateFinderModal } from "./components/search/DuplicateFinderModal";
import { ExpiryAlertModal } from "./components/search/ExpiryAlertModal";
import { Sidebar } from "./components/sidebar";
import { ToastContainer } from "./components/ui/Toast";
import { UpdateBanner } from "./components/ui/UpdateBanner";
import type { Settings } from "./types/settings";
import type { AddFolderResult } from "./types/index";

const EXPAND_FOCUS_DELAY_MS = 100;

function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const compactSearchInputRef = useRef<HTMLInputElement>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [statsOpen, setStatsOpen] = useState(false);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [expiryOpen, setExpiryOpen] = useState(false);
  const [reportResults, setReportResults] = useState<AddFolderResult[]>([]);
  const [pendingHwpFiles, setPendingHwpFiles] = useState<string[]>([]);
  const [showAutoIndexPrompt, setShowAutoIndexPrompt] = useState(false);
  const autoIndexPromptShownRef = useRef(false);
  const isMountedRef = useRef(true);
  useEffect(() => () => { isMountedRef.current = false; }, []);

  // 테마
  const { setTheme } = useTheme();

  // 첫 실행 (면책 조항 + 온보딩)
  const {
    showDisclaimer,
    showOnboarding,
    acceptDisclaimer,
    completeOnboarding,
    skipOnboarding,
    exitApp,
  } = useFirstRun();

  // 검색 상태
  const {
    query,
    setQuery,
    results,
    filenameResults,
    filteredResults,
    groupedResults,
    searchTime,
    isLoading,
    error: searchError,
    clearError: clearSearchError,
    searchMode,
    setSearchMode,
    filters,
    setFilters,
    viewMode,
    setViewMode,
    refineQuery,
    setRefineQuery,
    clearRefine,
    setComposing,
    invalidate: invalidateSearch,
    paradigm,
    setParadigm,
    submitNaturalQuery,
    parsedQuery,
    nlSubmitted,
  } = useSearch({ minConfidence: 0 });

  // 스크롤 기반 검색 영역 축소 (query 의존 → useSearch 뒤에 배치)
  const {
    isCollapsed,
    handleScroll,
    scrollToTop,
    scrollContainerRef,
    showScrollTopButton: showScrollTop,
    expand,
  } = useCollapsibleSearch({
    threshold: 80,
    onCollapse: () => searchInputRef.current?.blur(),
    searchInputRef,
    query,
  });

  // 인덱스 상태
  const {
    status,
    isIndexing,
    progress,
    error: indexError,
    clearError: clearIndexError,
    refreshStatus,
    addFolder,
    addFolderByPath,
    removeFolder,
    cancelIndexing,
    autoIndexAllDrives,
  } = useIndexStatus();

  // 최근 검색
  const {
    searches: recentSearches,
    addSearch,
    removeSearch,
    clearSearches,
  } = useRecentSearches();

  // 자동완성
  const autoComplete = useAutoComplete({ query });

  // 토스트 알림
  const { toasts, showToast, updateToast, dismissToast } = useToast();

  // 벡터 인덱싱 (2단계 백그라운드)
  const {
    status: vectorStatus,
    progress: vectorProgress,
    justCompleted: vectorJustCompleted,
    clearCompleted: clearVectorCompleted,
    refreshStatus: refreshVectorStatus,
    cancel: cancelVectorIndexing,
    startManual: startVectorIndexing,
    isRunning: isVectorIndexing,
    error: vectorError,
    clearError: clearVectorError,
  } = useVectorIndexing();

  // 앱 설정 (minConfidence, viewDensity, semanticEnabled, 하이라이트 색상)
  const {
    minConfidence,
    viewDensity,
    semanticEnabled,
    vectorIndexingMode,
    resultsPerPage,
    aiEnabled,
    applySettings,
  } = useAppSettings({ setSearchMode });

  // AI 검색 (Gemini RAG)
  const {
    aiAnalysis,
    isAiLoading,
    aiError,
    requestAiAnalysis,
    clearAiAnalysis,
  } = useAiSearch();

  // 파일/폴더 액션 (열기, 복사, 추가, 제거)
  const {
    handleOpenFile,
    handleCopyPath,
    handleOpenFolder,
    handleAddFolder: rawHandleAddFolder,
    handleAddFolderByPath: rawHandleAddFolderByPath,
    handleRemoveFolder,
  } = useFileActions({
    query,
    addSearch,
    showToast,
    updateToast,
    addFolder,
    addFolderByPath,
    removeFolder,
    invalidateSearch,
    refreshVectorStatus,
  });

  // 인덱싱 결과 리포트 (실패 또는 HWP 파일 존재 시 표시)
  const showReportIfNeeded = useCallback((results: AddFolderResult[]) => {
    const hasFailed = results.some((r) => r.failed_count > 0);
    const hasHwp = results.some((r) => (r.hwp_files?.length ?? 0) > 0);
    if (hasFailed || hasHwp) {
      setReportResults(results);
    }
  }, []);

  const handleAddFolder = useCallback(async () => {
    const results = await rawHandleAddFolder();
    if (results) showReportIfNeeded(results);
    return results;
  }, [rawHandleAddFolder, showReportIfNeeded]);

  const handleAddFolderByPath = useCallback(async (path: string) => {
    const result = await rawHandleAddFolderByPath(path);
    if (result) showReportIfNeeded([result]);
    return result;
  }, [rawHandleAddFolderByPath, showReportIfNeeded]);

  // 글로벌 에러 핸들러 등록 (프론트엔드 에러 → Rust 로그 파일)
  useEffect(() => {
    setupGlobalErrorHandlers();
  }, []);

  // 전역 우클릭 방지 (커스텀 컨텍스트 메뉴가 있는 요소 제외)
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest("[data-context-menu]")) return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", handler);
    return () => document.removeEventListener("contextmenu", handler);
  }, []);

  // 렌더 완료 후 창 표시 + 포커스 (visible: false safety net)
  // start_minimized인 경우 백엔드에서 이미 hide()했으므로, visible 상태일 때만 show
  useEffect(() => {
    const win = getCurrentWindow();
    win.isVisible().then((visible) => {
      if (visible) {
        win.setFocus().catch(() => {});
      }
    }).catch(() => {
      win.show();
      win.setFocus().catch(() => {});
    });
  }, []);

  // 앱 시작 시 등록 폴더 0개면 자동 인덱싱 안내 다이얼로그
  useEffect(() => {
    if (
      !autoIndexPromptShownRef.current &&
      status &&
      status.watched_folders.length === 0 &&
      !showDisclaimer &&
      !showOnboarding
    ) {
      autoIndexPromptShownRef.current = true;
      setShowAutoIndexPrompt(true);
    }
  }, [status, showDisclaimer, showOnboarding]);

  // FTS 인덱싱 완료 시 검색 캐시 무효화 (stale 결과 방지)
  useEffect(() => {
    if (progress?.phase === "completed") {
      clearSearchCache();
      if (query.trim()) {
        invalidateSearch();
      }
    }
  }, [progress?.phase, query, invalidateSearch]);

  // 벡터 인덱싱 완료 시 토스트 + 캐시 무효화
  useEffect(() => {
    if (vectorJustCompleted) {
      showToast("시맨틱 검색 준비 완료!", "success");
      clearVectorCompleted();
      clearSearchCache();
      if (query.trim()) {
        invalidateSearch();
      }
    }
  }, [vectorJustCompleted, showToast, clearVectorCompleted, query, invalidateSearch]);

  // HWP 감지 콜백 (증분 인덱싱 시)
  const handleHwpDetected = useCallback((paths: string[]) => {
    setPendingHwpFiles((prev) => [...prev, ...paths]);
    showToast(
      `새 HWP 파일 ${paths.length}개 발견 — 변환하려면 아래 배너를 확인하세요`,
      "info",
      5000
    );
  }, [showToast]);

  // Tauri 이벤트 리스너 (증분 인덱싱 + 모델 다운로드 + HWP 감지)
  useAppEvents({ query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast, updateToast, onHwpDetected: handleHwpDetected });

  // OTA 자동 업데이트
  const updater = useUpdater();

  // 미리보기 패널
  const [previewFilePath, setPreviewFilePath] = useState<string | null>(null);
  const handlePreviewClose = useCallback(() => setPreviewFilePath(null), []);
  const [previewWidth, setPreviewWidth] = useState(360);
  const isResizingRef = useRef(false);
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isResizingRef.current = true;
    const startX = e.clientX;
    const startWidth = previewWidth;
    const onMove = (ev: MouseEvent) => {
      if (!isResizingRef.current) return;
      const delta = startX - ev.clientX;
      setPreviewWidth(Math.max(280, Math.min(700, startWidth + delta)));
    };
    const onUp = () => {
      isResizingRef.current = false;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [previewWidth]);

  // 북마크
  const { bookmarks, addBookmark, removeBookmark, isBookmarked } = useBookmarks({ showToast });

  // 유사 문서 검색
  const { similarResults, similarSourceFile, handleFindSimilar, clearSimilarResults } = useSimilarDocuments(showToast);

  // 문서 카테고리 자동 분류
  const categories = useDocumentCategories(filteredResults, semanticEnabled);

  // 내보내기 (토스트 연동)
  const { exportToCSV, exportToXLSX, packageToZip, copyToClipboard } = useExport({ showToast });

  // SearchResultList용 메모이제이션 (인라인 함수 → 안정적 참조)
  const handleExportCSV = useCallback(() => exportToCSV(filteredResults, query), [exportToCSV, filteredResults, query]);
  const handleExportXLSX = useCallback(() => exportToXLSX(filteredResults, query), [exportToXLSX, filteredResults, query]);
  const handlePackageZip = useCallback(() => packageToZip(filteredResults), [packageToZip, filteredResults]);
  const handleCopyAll = useCallback(() => copyToClipboard(filteredResults, query), [copyToClipboard, filteredResults, query]);
  const memoizedRefineKeywords = useMemo(
    () => refineQuery.trim() ? refineQuery.trim().split(/\s+/) : undefined,
    [refineQuery]
  );

  // 에러 통합 (검색 + 인덱싱 + 벡터)
  const error = searchError || indexError || vectorError;
  const clearError = useCallback(() => {
    clearSearchError();
    clearIndexError();
    clearVectorError();
  }, [clearSearchError, clearIndexError, clearVectorError]);

  // 윈도우 포커스 복귀 시 검색창 자동 포커스
  useWindowFocus(searchInputRef, settingsOpen);

  // searchMode 변경 시 keywordOnly 필터 리셋 (함수형 업데이트로 stale closure 방지)
  useEffect(() => {
    if (searchMode !== "hybrid") {
      setFilters((prev) => prev.keywordOnly ? { ...prev, keywordOnly: false } : prev);
    }
  }, [searchMode, setFilters]);

  // 사이드바 토글
  const toggleSidebar = useCallback(() => {
    setSidebarOpen((prev) => !prev);
  }, []);

  // 검색어 선택 (최근 검색에서)
  const handleSelectSearch = useCallback(
    (searchQuery: string) => {
      setQuery(searchQuery);
      searchInputRef.current?.focus();
    },
    [setQuery]
  );

  // 검색어 변경
  const handleQueryChange = useCallback(
    (newQuery: string) => {
      setQuery(newQuery);
    },
    [setQuery]
  );

  // autoComplete.close를 안정적 ref로 보관 (매 렌더 재생성 방지)
  const autoCompleteCloseRef = useRef(autoComplete.close);
  autoCompleteCloseRef.current = autoComplete.close;

  // 결과 등장 후 800ms 뒤 추천어 자동 닫기
  useEffect(() => {
    if (!autoComplete.isOpen) return;
    if (results.length === 0 && filenameResults.length === 0) return;
    const timer = setTimeout(() => autoCompleteCloseRef.current(), 800);
    return () => clearTimeout(timer);
  }, [results, filenameResults, autoComplete.isOpen]);

  // 자동완성 항목 선택
  const handleSuggestionSelect = useCallback(
    (text: string) => {
      setQuery(text);
      autoCompleteCloseRef.current();
      searchInputRef.current?.focus();
    },
    [setQuery]
  );

  // 자연어 모드: 검색 완료 시 AI 자동 분석 (ref로 stale closure 방지)
  const aiAutoRef = useRef({ aiEnabled, paradigm, query, filteredResults, isLoading, requestAiAnalysis });
  aiAutoRef.current = { aiEnabled, paradigm, query, filteredResults, isLoading, requestAiAnalysis };
  useEffect(() => {
    const { aiEnabled: ai, paradigm: p, query: q, filteredResults: fr, isLoading: loading, requestAiAnalysis: req } = aiAutoRef.current;
    if (ai && p === "natural" && parsedQuery && fr.length > 0 && !loading) {
      req(q, fr);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- aiAutoRef로 모든 값 참조, parsedQuery만 트리거
  }, [parsedQuery]);

  // 즉시 모드: AI 수동 트리거
  const handleAskAi = useCallback(() => {
    if (query.trim() && filteredResults.length > 0) {
      requestAiAnalysis(query, filteredResults);
    }
  }, [query, filteredResults, requestAiAnalysis]);

  // 검색 결과가 있고 3초 유지 시 최근 검색에 자동 저장
  useRecentSearchSaver(query, filteredResults.length, addSearch);

  // 키보드 단축키
  useKeyboardShortcuts(
    {
      onFocusSearch: () => {
        // 화면에 보이는 검색창에 포커스 (CompactSearchBar 또는 SearchBar)
        const compact = compactSearchInputRef.current;
        const main = searchInputRef.current;
        const target = compact && compact.offsetParent !== null ? compact : main;
        target?.focus();
        target?.select();
      },
      onEscape: () => {
        if (selectedIndex >= 0) {
          setSelectedIndex(-1);
        } else {
          setQuery("");
          searchInputRef.current?.blur();
        }
      },
      onToggleSidebar: toggleSidebar,
      onArrowUp: () => {
        setSelectedIndex((prev) => Math.max(0, prev - 1));
      },
      onArrowDown: () => {
        setSelectedIndex((prev) =>
          Math.min(filteredResults.length - 1, prev + 1)
        );
      },
      onEnter: () => {
        if (selectedIndex >= 0 && selectedIndex < filteredResults.length) {
          const result = filteredResults[selectedIndex];
          handleOpenFile(result.file_path, result.page_number);
        }
      },
      onCopy: () => {
        if (selectedIndex >= 0 && selectedIndex < filteredResults.length) {
          const result = filteredResults[selectedIndex];
          handleCopyPath(result.file_path);
        }
      },
    },
    searchInputRef
  );

  // 결과 선택 + 미리보기 연동
  const { selectedIndex, setSelectedIndex } = useResultSelection(filteredResults, setPreviewFilePath);

  // 검색 영역 확장 핸들러
  const handleExpand = useCallback(() => {
    expand();
    scrollToTop();
    setTimeout(() => searchInputRef.current?.focus(), EXPAND_FOCUS_DELAY_MS);
  }, [expand, scrollToTop]);

  // 설정 모달 콜백
  const handleSettingsClose = useCallback(() => {
    setSettingsOpen(false);
    // Modal cleanup 후 검색창 포커스 복원 (rAF로 페인트 이후 보장)
    requestAnimationFrame(() => {
      searchInputRef.current?.focus();
    });
  }, []);

  const handleSettingsSaved = useCallback((settings: Settings) => {
    const wasEnabled = semanticEnabled;
    const wasAutoMode = vectorIndexingMode === "auto";
    applySettings(settings);
    clearSearchCache(); // 설정 변경 시 캐시된 검색 결과 무효화
    const nowEnabled = settings.semantic_search_enabled ?? false;
    const nowAutoMode = (settings.vector_indexing_mode ?? "manual") === "auto";
    if (isVectorIndexing && (!nowEnabled || !nowAutoMode)) {
      cancelVectorIndexing();
    }
    // 시맨틱 검색 켜질 때 + 자동 모드 → 벡터 인덱싱 자동 재개
    if (nowEnabled && nowAutoMode && !isVectorIndexing && (!wasEnabled || !wasAutoMode)) {
      // 반환값으로 최신 상태 확인 (stale closure 방지, unmount guard)
      refreshVectorStatus().then((freshStatus) => {
        if (!isMountedRef.current) return;
        if ((freshStatus?.pending_chunks ?? 0) > 0) {
          startVectorIndexing();
        }
      }).catch(() => {});
    }
  }, [applySettings, semanticEnabled, vectorIndexingMode, isVectorIndexing, cancelVectorIndexing, clearSearchCache, refreshVectorStatus, startVectorIndexing]);

  const handleClearData = useCallback(async () => {
    await invoke("clear_all_data");
    clearSearchCache();
    await Promise.all([refreshStatus(), refreshVectorStatus()]);
  }, [refreshStatus, refreshVectorStatus]);

  return (
    <div className="min-h-screen" style={{ backgroundColor: 'var(--color-bg-primary)', color: 'var(--color-text-primary)' }}>
      {/* Noise texture overlay */}
      <div className="noise-overlay" aria-hidden="true" />

      {/* Floating Alerts (API Key warnings, etc) */}
      <FloatingErrorBanner 
        message={aiError?.toLowerCase().includes("api") ? aiError : null} 
        isError={true} 
        onDismiss={clearAiAnalysis} 
      />

      {/* OTA 업데이트 배너 */}
      <UpdateBanner updater={updater} />

      {/* 사이드바 */}
      <Sidebar
        isOpen={sidebarOpen}
        onToggle={toggleSidebar}
        watchedFolders={status?.watched_folders ?? []}
        onAddFolder={handleAddFolder}
        onAddFolderByPath={handleAddFolderByPath}
        onRemoveFolder={handleRemoveFolder}
        isIndexing={isIndexing}
        onFoldersChange={refreshStatus}
        recentSearches={recentSearches}
        onSelectSearch={handleSelectSearch}
        onRemoveSearch={removeSearch}
        onClearSearches={clearSearches}
        bookmarks={bookmarks}
        onBookmarkSelect={(filePath, pageNumber) => {
          setPreviewFilePath(filePath);
          handleOpenFile(filePath, pageNumber);
        }}
        onBookmarkRemove={removeBookmark}
      />

      {/* 메인 콘텐츠 */}
      <div
        className="flex flex-col h-screen transition-all duration-200 ease-out"
        style={{ paddingLeft: sidebarOpen ? "var(--sidebar-width)" : "var(--sidebar-collapsed-width)" }}
      >
        {/* Compact Search Bar (스크롤 시 표시) */}
        {isCollapsed && (
          <div className="sticky top-0 z-30 bg-[var(--color-bg-primary)]/95 backdrop-blur-md">
            <CompactSearchBar
              ref={compactSearchInputRef}
              query={query}
              onQueryChange={handleQueryChange}
              onCompositionStart={() => setComposing(true)}
              onCompositionEnd={(finalValue) => setComposing(false, finalValue)}
              searchMode={searchMode}
              onSearchModeChange={setSearchMode}
              isLoading={isLoading}
              status={status}
              resultCount={filteredResults.length}
              onExpand={handleExpand}
              onAddFolder={handleAddFolder}
              onOpenSettings={() => setSettingsOpen(true)}
              onOpenHelp={() => setHelpOpen(true)}
              isIndexing={isIndexing}
              isSidebarOpen={sidebarOpen}
              filters={filters}
              onFiltersChange={setFilters}
              viewMode={viewMode}
              onViewModeChange={setViewMode}
              refineQuery={refineQuery}
              onRefineQueryChange={setRefineQuery}
              onRefineQueryClear={clearRefine}
              totalResultCount={results.length}
              paradigm={paradigm}
              onParadigmChange={setParadigm}
              onSubmitNatural={submitNaturalQuery}
            />
          </div>
        )}

        {/* Expanded Header */}
        {!isCollapsed && (
          <div className="sticky top-0 z-20 bg-[var(--color-bg-primary)]/90 backdrop-blur-md border-b" style={{ borderColor: 'var(--color-border)' }}>
            <Header
              onAddFolder={handleAddFolder}
              onOpenSettings={() => setSettingsOpen(true)}
              onOpenHelp={() => setHelpOpen(true)}
              onOpenStats={() => setStatsOpen(true)}
              onOpenDuplicates={() => setDuplicateOpen(true)}
              onOpenExpiry={() => setExpiryOpen(true)}
              onGoHome={() => {
                setQuery("");
                setSelectedIndex(-1);
                searchInputRef.current?.focus();
              }}
              isIndexing={isIndexing}
              isSidebarOpen={sidebarOpen}
              hasQuery={query.length > 0}
            />
          </div>
        )}

        {/* Search Bar + Filters Area — 스크롤 컨테이너 밖 (collapse 시 스크롤 점프 방지) */}
        {!isCollapsed && (
          <div className="px-4 pt-4 pb-2">
            <SearchBar
              ref={searchInputRef}
              query={query}
              onQueryChange={handleQueryChange}
              onCompositionStart={() => setComposing(true)}
              onCompositionEnd={(finalValue) => setComposing(false, finalValue)}
              searchMode={searchMode}
              onSearchModeChange={setSearchMode}
              isLoading={isLoading}
              status={status}
              resultCount={filteredResults.length}
              searchTime={searchTime}
              suggestions={autoComplete.suggestions}
              isSuggestionsOpen={autoComplete.isOpen}
              suggestionsSelectedIndex={autoComplete.selectedIndex}
              onSuggestionSelect={handleSuggestionSelect}
              onSuggestionsKeyDown={autoComplete.handleKeyDown}
              onSuggestionsClose={autoComplete.close}
              onSuggestionsSetIndex={autoComplete.setSelectedIndex}
              paradigm={paradigm}
              onParadigmChange={setParadigm}
              onSubmitNatural={submitNaturalQuery}
            />

            {/* 벡터 인덱싱 상태 배너 */}
            <VectorIndexingBanner
              isVisible={isVectorIndexing}
              progress={vectorProgress}
              onCancel={cancelVectorIndexing}
            />

            {/* 필터 바 / 파싱 결과 */}
            {query && (results.length > 0 || filenameResults.length > 0) && (
              <div className="max-w-4xl mx-auto mt-2 pb-3 border-b" style={{ borderColor: "var(--color-border)" }}>
                {paradigm === "natural" && parsedQuery ? (
                  <SmartQueryInfo
                    parsed={parsedQuery}
                    onClear={() => {
                      // 필터 제거: 원본 쿼리로 재검색
                      submitNaturalQuery();
                    }}
                  />
                ) : (
                  <SearchFilters
                    filters={filters}
                    onFiltersChange={setFilters}
                    showRefineSearch={results.length > 0 || filenameResults.length > 0}
                    searchMode={searchMode}
                    refineQuery={refineQuery}
                    onRefineQueryChange={setRefineQuery}
                    onRefineQueryClear={clearRefine}
                    watchedFolders={status?.watched_folders ?? []}
                  />
                )}
              </div>
            )}

            {error && <div className="mt-3"><ErrorBanner message={error} onDismiss={clearError} /></div>}
          </div>
        )}

        {/* Scrollable Content + Preview Split */}
        <div className="flex-1 flex overflow-hidden">
          {/* 검색 결과 영역 */}
          <div
            ref={scrollContainerRef}
            onScroll={(e) => { handleScroll(e); autoComplete.close(); }}
            className="flex-1 overflow-y-auto overflow-x-hidden"
            style={{ overflowAnchor: "none" }}
          >
            {isCollapsed && error && (
              <div className="px-6 pt-2"><ErrorBanner message={error} onDismiss={clearError} /></div>
            )}

            <main className="px-5 sm:px-8 pb-20 h-full">
              <div className={`mx-auto mt-4 h-full ${query.trim() ? (previewFilePath ? "max-w-3xl" : "max-w-4xl") : "w-full max-w-[1400px]"}`}>
                {/* 유사 문서 결과 배너 */}
                {similarResults.length > 0 && (
                  <div className="mb-4 p-3 rounded-lg border" style={{ backgroundColor: "var(--color-bg-secondary)", borderColor: "var(--color-border)" }}>
                    <div className="flex items-center justify-between mb-2">
                      <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">
                        "{similarSourceFile}"와 유사한 문서 ({similarResults.length}건)
                      </h3>
                      <button
                        onClick={clearSimilarResults}
                        className="text-xs px-2 py-1 rounded hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-muted)]"
                      >
                        닫기
                      </button>
                    </div>
                    <div className="space-y-1">
                      {similarResults.slice(0, 10).map((r, i) => (
                        <div
                          key={`sim-${i}`}
                          className="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-[var(--color-bg-tertiary)] cursor-pointer transition-colors"
                          onClick={() => handleOpenFile(r.file_path, r.page_number)}
                        >
                          <span className="text-xs font-mono text-[var(--color-text-muted)] w-6 text-right">{r.confidence}%</span>
                          <span className="text-sm truncate text-[var(--color-text-primary)]">{r.file_name}</span>
                          <span className="text-[10px] text-[var(--color-text-muted)] truncate ml-auto max-w-[200px]">
                            {r.content_preview?.slice(0, 80)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* AI 물어보기 버튼 (즉시 모드, 결과 있을 때, AI 답변 없을 때) */}
                {aiEnabled && !aiAnalysis && !isAiLoading && !aiError && filteredResults.length > 0 && (
                  <div className="mb-3 flex justify-end">
                    <button
                      onClick={handleAskAi}
                      className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors hover:opacity-90"
                      style={{
                        backgroundColor: "var(--color-accent)",
                        color: "white",
                      }}
                    >
                      <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 3l1.5 5.5L19 10l-5.5 1.5L12 17l-1.5-5.5L5 10l5.5-1.5Z"/></svg>
                      AI에게 물어보기
                    </button>
                  </div>
                )}

                {/* AI 답변 패널 */}
                {aiEnabled && (aiAnalysis || isAiLoading || (aiError && !aiError.toLowerCase().includes("api"))) && (
                  <AiAnswerPanel
                    analysis={aiAnalysis}
                    isLoading={isAiLoading}
                    error={aiError && !aiError.toLowerCase().includes("api") ? aiError : null}
                    onDismiss={clearAiAnalysis}
                    onOpenFile={(filePath) => handleOpenFile(filePath, undefined)}
                  />
                )}

                <SearchResultList
                  results={filteredResults}
                  filenameResults={filters.excludeFilename ? [] : filenameResults}
                  groupedResults={groupedResults}
                  viewMode={viewMode}
                  onViewModeChange={setViewMode}
                  viewDensity={viewDensity}
                  query={query}
                  isLoading={isLoading}
                  selectedIndex={selectedIndex}
                  onOpenFile={handleOpenFile}
                  onCopyPath={handleCopyPath}
                  onOpenFolder={handleOpenFolder}
                  onExportCSV={handleExportCSV}
                  onExportXLSX={handleExportXLSX}
                  onPackageZip={handlePackageZip}
                  onCopyAll={handleCopyAll}
                  refineKeywords={memoizedRefineKeywords}
                  resultCount={filteredResults.length}
                  totalResultCount={results.length}
                  minConfidence={minConfidence}
                  searchTime={searchTime}
                  resultsPerPage={resultsPerPage}
                  indexedFiles={status?.indexed_files ?? 0}
                  indexedFolders={status?.watched_folders?.length ?? 0}
                  recentSearches={recentSearches}
                  onSelectSearch={handleSelectSearch}
                  semanticEnabled={semanticEnabled}
                  onAddFolder={handleAddFolder}
                  onSelectResult={setSelectedIndex}
                  onFindSimilar={semanticEnabled ? handleFindSimilar : undefined}
                  categories={categories}
                  paradigm={paradigm}
                  nlSubmitted={nlSubmitted}
                  parsedQuery={parsedQuery}
                />
              </div>
            </main>
          </div>

          {/* 미리보기 패널 */}
          {previewFilePath && (
            <>
              {/* 리사이즈 핸들 */}
              <div
                onMouseDown={handleResizeStart}
                className="w-1 shrink-0 cursor-col-resize hover:bg-[var(--color-accent)] transition-colors group relative"
                style={{ backgroundColor: "var(--color-border)" }}
                title="드래그하여 너비 조절"
              >
                <div className="absolute inset-y-0 -left-1 -right-1" />
              </div>
              <div className="shrink-0" style={{ width: previewWidth }}>
                <PreviewPanel
                  filePath={previewFilePath}
                  highlightQuery={query}
                  onClose={handlePreviewClose}
                  onOpenFile={handleOpenFile}
                  onCopyPath={handleCopyPath}
                  onOpenFolder={handleOpenFolder}
                  onBookmark={addBookmark}
                  isBookmarked={isBookmarked(previewFilePath)}
                />
              </div>
            </>
          )}
        </div>

        <StatusBar
          status={status}
          progress={progress}
          vectorStatus={vectorStatus}
          onCancelIndexing={cancelIndexing}
          onCancelVectorIndexing={cancelVectorIndexing}
          onStartVectorIndexing={startVectorIndexing}
          semanticEnabled={semanticEnabled}
        />
      </div>

      <AppModals
        settingsOpen={settingsOpen}
        onSettingsClose={handleSettingsClose}
        onThemeChange={setTheme}
        onSettingsSaved={handleSettingsSaved}
        onClearData={handleClearData}
        onAutoIndexAllDrives={autoIndexAllDrives}
        helpOpen={helpOpen}
        onHelpClose={() => setHelpOpen(false)}
        showDisclaimer={showDisclaimer}
        onAcceptDisclaimer={acceptDisclaimer}
        onExitApp={exitApp}
        showOnboarding={showOnboarding}
        onCompleteOnboarding={() => { completeOnboarding(); setShowAutoIndexPrompt(true); }}
        onSkipOnboarding={() => { skipOnboarding(); setShowAutoIndexPrompt(true); }}
      />
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />
      <IndexingReportModal
        isOpen={reportResults.length > 0 || pendingHwpFiles.length > 0}
        onClose={() => { setReportResults([]); setPendingHwpFiles([]); }}
        results={pendingHwpFiles.length > 0 && reportResults.length === 0
          ? [{ success: true, indexed_count: 0, failed_count: 0, hwp_files: pendingHwpFiles } as AddFolderResult]
          : reportResults
        }
        onReindex={async (convertedPaths) => {
          // 변환된 HWPX 파일이 속한 watched folder를 찾아 resume_indexing
          const watchedFolders = status?.watched_folders ?? [];
          const foldersToSync = new Set<string>();
          const strip = (p: string) => p.replace(/^\\\\\?\\/, "").replace(/\\/g, "/").toLowerCase();
          for (const hwpxPath of convertedPaths) {
            const normalized = strip(hwpxPath);
            for (const folder of watchedFolders) {
              if (normalized.startsWith(strip(folder))) {
                foldersToSync.add(folder);
                break;
              }
            }
          }
          let indexedCount = 0;
          for (const folder of foldersToSync) {
            try {
              const result = await invoke<AddFolderResult>("resume_indexing", { path: folder });
              indexedCount += result.indexed_count;
            } catch (err) {
              logToBackend("error", `Re-indexing failed for ${folder}`, String(err), "App");
            }
          }
          showToast(`${indexedCount}개 HWPX 파일 인덱싱 완료`, "success");
          refreshStatus();
        }}
      />

      {/* 자동 인덱싱 안내 다이얼로그 */}
      <StatisticsModal
        isOpen={statsOpen}
        onClose={() => setStatsOpen(false)}
        onFilterByType={(fileType) => {
          // 파일 유형 맵핑 (hwpx, docx 등 → FileTypeFilter)
          const typeMap: Record<string, import("./types/search").FileTypeFilter> = {
            hwpx: "hwpx", hwp: "hwpx",
            docx: "docx", doc: "docx",
            pptx: "pptx", ppt: "pptx",
            xlsx: "xlsx", xls: "xlsx",
            pdf: "pdf",
            txt: "txt", md: "txt",
          };
          const filterType = typeMap[fileType] || "all";
          setFilters((prev) => ({ ...prev, fileType: filterType }));
          if (!query) setQuery("*"); // 빈 쿼리일 때 전체 조회 트리거
        }}
        onOpenFile={handleOpenFile}
      />

      <DuplicateFinderModal
        isOpen={duplicateOpen}
        onClose={() => setDuplicateOpen(false)}
        onOpenFile={handleOpenFile}
        onOpenFolder={handleOpenFolder}
        showToast={showToast}
      />

      <ExpiryAlertModal
        isOpen={expiryOpen}
        onClose={() => setExpiryOpen(false)}
        onOpenFile={handleOpenFile}
        showToast={showToast}
      />

      <AutoIndexPrompt
        isOpen={showAutoIndexPrompt}
        onClose={() => setShowAutoIndexPrompt(false)}
        onAutoIndex={autoIndexAllDrives}
        onSelectFolder={handleAddFolder}
      />

      <FloatingUI
        vectorStatus={vectorStatus}
        vectorProgress={vectorProgress}
        onCancelVectorIndexing={cancelVectorIndexing}
        showScrollTop={showScrollTop}
        onScrollToTop={scrollToTop}
      />
    </div>
  );
}

export default App;
