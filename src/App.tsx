import { useCallback, useEffect, useState } from "react";
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
import { setupGlobalErrorHandlers, logToBackend } from "./utils/errorLogger";

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
import { UpdateBanner } from "./components/ui/UpdateBanner";
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

  // ── App Settings (cross-cutting) ──
  const {
    minConfidence, viewDensity, semanticEnabled, vectorIndexingMode,
    resultsPerPage, applySettings,
  } = useAppSettings({ setSearchMode: search.setSearchMode });

  // Document categories (cross-cutting: search results + settings)
  const categories = useDocumentCategories(search.filteredResults, semanticEnabled);

  // ── AI Disclaimer ──
  const [showAiDisclaimer, setShowAiDisclaimer] = useState(false);

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
    const hasHwp = results.some((r) => (r.hwp_files?.length ?? 0) > 0);
    if (hasFailed || hasHwp) ui.setReportResults(results);
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

  // 전역 우클릭 방지
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if ((e.target as HTMLElement).closest("[data-context-menu]")) return;
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
  useEffect(() => {
    if (idx.progress?.phase === "completed") {
      clearSearchCache();
      if (search.query.trim()) search.invalidateSearch();
    }
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

  // HWP 감지 콜백
  const handleHwpDetected = useCallback((paths: string[]) => {
    ui.setPendingHwpFiles((prev) => [...prev, ...paths]);
    ui.showToast(`새 HWP 파일 ${paths.length}개 발견 — 변환하려면 아래 배너를 확인하세요`, "info", 5000);
  }, [ui.setPendingHwpFiles, ui.showToast]);

  // Tauri 이벤트 리스너
  useAppEvents({
    query: search.query,
    invalidateSearch: search.invalidateSearch,
    refreshStatus: idx.refreshStatus,
    refreshVectorStatus: idx.refreshVectorStatus,
    showToast: ui.showToast,
    updateToast: ui.updateToast,
    onHwpDetected: handleHwpDetected,
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
        target?.select();
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
    await invoke("clear_all_data");
    clearSearchCache();
    await Promise.all([idx.refreshStatus(), idx.refreshVectorStatus()]);
  }, [idx]);

  // ── Render ──

  return (
    <div className="h-screen mx-auto relative overflow-hidden" style={{ backgroundColor: 'var(--color-bg-primary)', color: 'var(--color-text-primary)', maxWidth: '1920px' }}>
      <div className="noise-overlay" aria-hidden="true" />


      <UpdateBanner updater={ui.updater} />

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
                search.searchInputRef.current?.focus();
              }}
              isIndexing={idx.isIndexing}
              isSidebarOpen={ui.sidebarOpen}
              hasQuery={search.query.length > 0}
            />
          </div>
        )}

        {/* Search Bar + Filters */}
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
              suggestions={search.autoComplete.suggestions}
              isSuggestionsOpen={search.autoComplete.isOpen}
              suggestionsSelectedIndex={search.autoComplete.selectedIndex}
              onSuggestionSelect={search.handleSuggestionSelect}
              onSuggestionsKeyDown={search.autoComplete.handleKeyDown}
              onSuggestionsClose={search.autoComplete.close}
              onSuggestionsSetIndex={search.autoComplete.setSelectedIndex}
              paradigm={search.paradigm}
              onParadigmChange={search.setParadigm}
              onSubmitNatural={handleSubmitQuery}
            />

            <VectorIndexingBanner
              isVisible={idx.isVectorIndexing}
              progress={idx.vectorProgress}
              onCancel={idx.cancelVectorIndexing}
            />

            {search.paradigm !== "question" && search.query && (search.results.length > 0 || search.filenameResults.length > 0) && (
              <div className="mt-2 pb-3 border-b" style={{ borderColor: "var(--color-border)" }}>
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
                    presets={search.presets}
                    onSavePreset={search.handleSavePreset}
                    onApplyPreset={search.handleApplyPreset}
                    onRemovePreset={search.removePreset}
                  />
                )}
              </div>
            )}

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

        {/* Scrollable Content + Preview */}
        <div className="flex-1 flex overflow-hidden">
          <div
            ref={search.scrollContainerRef}
            onScroll={(e) => { search.handleScroll(e); search.autoComplete.close(); }}
            className="flex-1 overflow-y-auto overflow-x-hidden"
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

            <main className="px-5 sm:px-8 pb-20 h-full">
              <div className="mt-4 h-full">
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
                    minConfidence={minConfidence}
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
                  />
                )}
              </div>
            </main>
          </div>

          {/* Preview Panel */}
          {ui.previewFilePath && (
            <>
              <div
                onMouseDown={ui.handleResizeStart}
                className="w-1 shrink-0 cursor-col-resize hover:bg-[var(--color-accent)] transition-colors group relative"
                style={{ backgroundColor: "var(--color-border)" }}
                title="드래그하여 너비 조절"
              >
                <div className="absolute inset-y-0 -left-1 -right-1" />
              </div>
              <div className="shrink-0" style={{ width: ui.previewWidth, minWidth: 280, maxWidth: '50%' }}>
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
          onCancelIndexing={idx.cancelIndexing}
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
        showOnboarding={ui.showOnboarding}
        onCompleteOnboarding={() => { ui.completeOnboarding(); ui.setShowAutoIndexPrompt(true); }}
        onSkipOnboarding={() => { ui.skipOnboarding(); ui.setShowAutoIndexPrompt(true); }}
      />
      <ToastContainer toasts={ui.toasts} onDismiss={ui.dismissToast} />
      <IndexingReportModal
        isOpen={ui.reportResults.length > 0 || ui.pendingHwpFiles.length > 0}
        onClose={() => { ui.setReportResults([]); ui.setPendingHwpFiles([]); }}
        results={ui.pendingHwpFiles.length > 0 && ui.reportResults.length === 0
          ? [{ success: true, indexed_count: 0, failed_count: 0, hwp_files: ui.pendingHwpFiles } as AddFolderResult]
          : ui.reportResults
        }
        onReindex={async (convertedPaths) => {
          const watchedFolders = idx.status?.watched_folders ?? [];
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
          ui.showToast(`${indexedCount}개 HWPX 파일 인덱싱 완료`, "success");
          idx.refreshStatus();
        }}
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
    </div>
  );
}

export default App;
