import { useCallback, useEffect, useRef, useState } from "react";
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

// Components
import { Header, StatusBar, ErrorBanner, AppModals, FloatingUI } from "./components/layout";
import { AutoIndexPrompt } from "./components/layout/AutoIndexPrompt";
import { SearchBar, SearchFilters, SearchResultList, CompactSearchBar } from "./components/search";
import { TypoSuggestion } from "./components/search/TypoSuggestion";
import SmartQueryInfo from "./components/search/SmartQueryInfo";
import AiAnswerPanel from "./components/search/AiAnswerPanel";
import { AiDisclaimerModal, isAiDisclaimerAccepted } from "./components/search/AiDisclaimerModal";
import { VectorIndexingBanner } from "./components/search/VectorIndexingBanner";
import { PreviewPanel } from "./components/search/PreviewPanel";
import { IndexingReportModal } from "./components/search/IndexingReportModal";
import { StatisticsModal } from "./components/search/StatisticsModal";
import { DuplicateFinderModal } from "./components/search/DuplicateFinderModal";
import { Sidebar } from "./components/sidebar";
import { ToastContainer } from "./components/ui/Toast";
import { OnboardingTour, resetOnboardingTour } from "./components/onboarding/OnboardingTour";
import { UpdateModal } from "./components/updater/UpdateModal";
import { DOCUFINDER_TOUR_STEPS, DOCUFINDER_TOUR_STORAGE_KEY } from "./components/onboarding/tourSteps";
import type { Settings } from "./types/settings";
import type { AddFolderResult } from "./types/index";

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

  // ── App Settings (cross-cutting) ──
  const {
    viewDensity, semanticEnabled, vectorIndexingMode,
    resultsPerPage, applySettings,
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
  const showReportIfNeeded = useCallback((results: AddFolderResult[]) => {
    const hasFailed = results.some((r) => r.failed_count > 0);
    if (hasFailed) ui.setReportResults(results);
  }, [ui.setReportResults]);

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

  // 폴더 0개 → 자동 인덱싱 안내
  useEffect(() => {
    if (idx.status && idx.status.watched_folders.length === 0 && !ui.showOnboarding) {
      ui.tryShowAutoIndexPrompt();
    }
  }, [idx.status, ui.showOnboarding, ui.tryShowAutoIndexPrompt]);

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
      onEscape: () => {
        if (search.selectedIndex >= 0) {
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
    ui.showToast("설정이 저장되었습니다", "success", 2000);
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
      const message = err instanceof Error ? err.message : String(err);
      ui.showToast(`초기화 실패: ${message}`, "error");
      throw err;
    }
  }, [idx, ui]);

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
              onCompositionStart={() => search.setComposing(true)}
              onCompositionEnd={(finalValue) => search.setComposing(false, finalValue)}
              searchMode={search.searchMode}
              onSearchModeChange={search.setSearchMode}
              isLoading={search.isLoading}
              status={idx.status}
              resultCount={search.filteredResults.length}
              onExpand={search.handleExpand}
              onAddFolder={handleAddFolder}
              onOpenSettings={() => ui.setSettingsOpen(true)}
              onOpenHelp={() => ui.setHelpOpen(true)}
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
              onOpenUpdate={() => setUpdateModalOpen(true)}
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
              onGoHome={() => {
                search.setQuery("");
                search.setSelectedIndex(-1);
                search.setParadigm("instant");
                search.resetAi();
                search.searchInputRef.current?.focus();
              }}
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
              onCompositionStart={() => search.setComposing(true)}
              onCompositionEnd={(finalValue) => search.setComposing(false, finalValue)}
              searchMode={search.searchMode}
              onSearchModeChange={search.setSearchMode}
              isLoading={search.isLoading}
              status={idx.status}
              resultCount={search.filteredResults.length}
              searchTime={search.searchTime}
              paradigm={search.paradigm}
              onParadigmChange={search.setParadigm}
              onSubmitNatural={handleSubmitQuery}
              watchedFolders={idx.status?.watched_folders ?? []}
              searchScope={search.filters.searchScope}
              onSearchScopeChange={(scope) => search.setFilters((prev) => ({ ...prev, searchScope: scope }))}
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
                  <AiAnswerPanel
                    answer={search.aiAnswer}
                    isStreaming={search.isAiStreaming}
                    analysis={search.aiAnalysis}
                    error={search.aiError}
                    onReset={search.resetAi}
                    currentQuestion={search.aiAskedQuery}
                    onExampleClick={(text) => {
                      search.setQuery(text);
                      if (!isAiDisclaimerAccepted()) {
                        setShowAiDisclaimer(true);
                      } else {
                        search.askAi(text, search.filters.searchScope);
                      }
                    }}
                  />
                ) : (
                  <SearchResultList
                    results={search.filteredResults}
                    filenameResults={search.filters.excludeFilename ? [] : search.filenameResults}
                    groupedResults={search.groupedResults}
                    viewMode={search.viewMode}
                    onViewModeChange={search.setViewMode}
                    viewDensity={viewDensity}
                    query={search.query}
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
                    semanticEnabled={semanticEnabled}
                    onAddFolder={handleAddFolder}
                    onSelectResult={search.setSelectedIndex}
                    onFindSimilar={semanticEnabled ? search.handleFindSimilar : undefined}
                    categories={categories}
                    paradigm={search.paradigm}
                    nlSubmitted={search.nlSubmitted}
                    parsedQuery={search.parsedQuery}
                    onSwitchToAnything={handleSwitchToAnything}
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
                <PreviewPanel
                  filePath={ui.previewFilePath}
                  highlightQuery={search.query}
                  onClose={ui.handlePreviewClose}
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
              </div>
            </>
          )}
          {ui.previewFilePath && previewOverlay && (
            <>
              <div
                className="absolute inset-0 z-40 bg-black/15 animate-fade-in"
                onClick={ui.handlePreviewClose}
              />
              <div
                className="absolute right-0 top-0 bottom-0 z-50 shadow-2xl preview-slide-in"
                style={{ width: Math.max(Math.min(ui.previewWidth, (contentFlexRef.current?.clientWidth ?? 600) * 0.85), MIN_PREVIEW_WIDTH), minWidth: MIN_PREVIEW_WIDTH }}
              >
                <PreviewPanel
                  filePath={ui.previewFilePath}
                  highlightQuery={search.query}
                  onClose={ui.handlePreviewClose}
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
      <AppModals
        settingsOpen={ui.settingsOpen}
        onSettingsClose={handleSettingsClose}
        onThemeChange={ui.setTheme}
        onSettingsSaved={handleSettingsSaved}
        onClearData={handleClearData}
        onAutoIndexAllDrives={idx.autoIndexAllDrives}
        helpOpen={ui.helpOpen}
        onHelpClose={() => ui.setHelpOpen(false)}
        onRestartTour={restartTour}
        showOnboarding={ui.showOnboarding}
        onCompleteOnboarding={() => { ui.completeOnboarding(); ui.setShowAutoIndexPrompt(true); }}
        onSkipOnboarding={() => { ui.skipOnboarding(); ui.setShowAutoIndexPrompt(true); }}
      />
      <ToastContainer toasts={ui.toasts} onDismiss={ui.dismissToast} />
      <IndexingReportModal
        isOpen={ui.reportResults.length > 0}
        onClose={() => ui.setReportResults([])}
        results={ui.reportResults}
      />

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

      <DuplicateFinderModal
        isOpen={ui.duplicateOpen}
        onClose={() => ui.setDuplicateOpen(false)}
        onOpenFile={handleOpenFile}
        onOpenFolder={handleOpenFolder}
        showToast={ui.showToast}
      />


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
      />

      {/* 기능 투어 — 첫 방문 시 자동 시작, 헬프 메뉴에서 재시작 가능 */}
      <OnboardingTour
        steps={DOCUFINDER_TOUR_STEPS}
        storageKey={DOCUFINDER_TOUR_STORAGE_KEY}
        autoStart
        runKey={tourRunKey}
      />
    </div>
  );
}

export default App;
