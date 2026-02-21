import { useRef, useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Hooks
import { useSearch, useIndexStatus, useVectorIndexing, useKeyboardShortcuts, useRecentSearches, useExport, useToast, useTheme, useCollapsibleSearch } from "./hooks";
import { clearSearchCache } from "./hooks/useSearch";
import { useFirstRun } from "./hooks/useFirstRun";
import { useFileActions } from "./hooks/useFileActions";
import { useAppSettings } from "./hooks/useAppSettings";

// Components
import { Header, StatusBar, ErrorBanner } from "./components/layout";
import { SearchBar, SearchFilters, SearchResultList, CompactSearchBar } from "./components/search";
import { Sidebar } from "./components/sidebar";
import { SettingsModal } from "./components/settings/SettingsModal";
import { HelpModal } from "./components/help/HelpModal";
import { ToastContainer } from "./components/ui/Toast";
import { VectorIndexingFAB } from "./components/ui/VectorIndexingFAB";
import { DisclaimerModal, OnboardingModal } from "./components/onboarding";

function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const compactSearchInputRef = useRef<HTMLInputElement>(null);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);

  // 스크롤 기반 검색 영역 축소
  const {
    isCollapsed,
    handleScroll,
    scrollToTop,
    scrollContainerRef,
    showScrollTopButton: showScrollTop,
    expand,
  } = useCollapsibleSearch({
    threshold: 200,
    onCollapse: () => searchInputRef.current?.blur(),
  });

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
  } = useSearch({ minConfidence: 0 });

  // 인덱스 상태
  const {
    status,
    isIndexing,
    progress,
    error: indexError,
    clearError: clearIndexError,
    refreshStatus,
    addFolder,
    removeFolder,
    cancelIndexing,
  } = useIndexStatus();

  // 최근 검색
  const {
    searches: recentSearches,
    addSearch,
    removeSearch,
    clearSearches,
  } = useRecentSearches();

  // 토스트 알림
  const { toasts, showToast, updateToast, dismissToast } = useToast();

  // 벡터 인덱싱 (2단계 백그라운드)
  const {
    status: vectorStatus,
    progress: vectorProgress,
    justCompleted: vectorJustCompleted,
    clearCompleted: clearVectorCompleted,
    cancel: cancelVectorIndexing,
    startManual: startVectorIndexing,
    isRunning: isVectorIndexing,
  } = useVectorIndexing();

  // 앱 설정 (minConfidence, viewDensity, semanticEnabled, 하이라이트 색상)
  const {
    minConfidence,
    viewDensity,
    setViewDensity,
    semanticEnabled,
    applySettings,
  } = useAppSettings({ setSearchMode });

  // 파일/폴더 액션 (열기, 복사, 추가, 제거)
  const {
    handleOpenFile,
    handleCopyPath,
    handleOpenFolder,
    handleAddFolder,
    handleRemoveFolder,
  } = useFileActions({
    query,
    addSearch,
    showToast,
    updateToast,
    addFolder,
    removeFolder,
    invalidateSearch,
  });

  // 벡터 인덱싱 완료 시 토스트
  useEffect(() => {
    if (vectorJustCompleted) {
      showToast("시맨틱 검색 준비 완료!", "success");
      clearVectorCompleted();
    }
  }, [vectorJustCompleted, showToast, clearVectorCompleted]);

  // 내보내기 (토스트 연동)
  const { exportToCSV, copyToClipboard } = useExport({ showToast });

  // 에러 통합
  const error = searchError || indexError;
  const clearError = useCallback(() => {
    clearSearchError();
    clearIndexError();
  }, [clearSearchError, clearIndexError]);

  // 윈도우 포커스 복귀 시 검색창 포커스 재설정
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let lastResetTime = 0;

    const resetSearchFocus = () => {
      if (settingsOpen) return;

      // 디바운스: 500ms 이내 중복 실행 방지 (IPC 이벤트 폭주 대응)
      const now = Date.now();
      if (now - lastResetTime < 500) return;
      lastResetTime = now;

      const input = searchInputRef.current;
      // DOM에 연결된 요소인지 확인 (unmount된 stale ref 방지)
      if (!input || !input.isConnected) return;

      const activeElement = document.activeElement;
      const isEditable =
        activeElement?.tagName === "INPUT" ||
        activeElement?.tagName === "TEXTAREA" ||
        (activeElement instanceof HTMLElement && activeElement.isContentEditable);

      if (isEditable && activeElement !== input) {
        return;
      }

      // 이미 검색창에 포커스 중이면 건너뜀
      if (activeElement === input) {
        return;
      }

      requestAnimationFrame(() => {
        if (input.isConnected) {
          input.focus();
        }
      });
    };

    const setup = async () => {
      const window = getCurrentWindow();
      try {
        unlisten = await window.onFocusChanged(({ payload }) => {
          if (payload) {
            resetSearchFocus();
          }
        });
      } catch (err) {
        console.warn("Failed to register focus handler:", err);
      }
    };

    setup();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [settingsOpen]);

  // searchMode 변경 시 keywordOnly 필터 리셋
  useEffect(() => {
    if (searchMode !== "hybrid" && filters.keywordOnly) {
      setFilters({ ...filters, keywordOnly: false });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchMode]);

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

  // 검색 결과가 있고 3초 유지 시 최근 검색에 저장
  useEffect(() => {
    if (searchTimerRef.current) {
      clearTimeout(searchTimerRef.current);
      searchTimerRef.current = null;
    }

    const trimmedQuery = query.trim();
    if (trimmedQuery.length >= 2 && filteredResults.length > 0) {
      searchTimerRef.current = setTimeout(() => {
        addSearch(trimmedQuery);
        searchTimerRef.current = null;
      }, 3000);
    }

    return () => {
      if (searchTimerRef.current) {
        clearTimeout(searchTimerRef.current);
      }
    };
  }, [query, filteredResults.length, addSearch]);

  // 키보드 단축키
  useKeyboardShortcuts(
    {
      onFocusSearch: () => {
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
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

  // 결과가 변경되면 선택 초기화
  const prevResultsLength = useRef(filteredResults.length);
  useEffect(() => {
    if (prevResultsLength.current !== filteredResults.length) {
      prevResultsLength.current = filteredResults.length;
      if (selectedIndex >= filteredResults.length) {
        setSelectedIndex(filteredResults.length > 0 ? 0 : -1);
      }
    }
  }, [filteredResults.length, selectedIndex]);

  // 검색 영역 확장 핸들러
  const handleExpand = useCallback(() => {
    expand();
    scrollToTop();
    setTimeout(() => searchInputRef.current?.focus(), 100);
  }, [expand, scrollToTop]);

  return (
    <div className="min-h-screen" style={{ backgroundColor: 'var(--color-bg-primary)', color: 'var(--color-text-primary)' }}>
      {/* 사이드바 */}
      <Sidebar
        isOpen={sidebarOpen}
        onToggle={toggleSidebar}
        watchedFolders={status?.watched_folders ?? []}
        onAddFolder={handleAddFolder}
        onRemoveFolder={handleRemoveFolder}
        isIndexing={isIndexing}
        onFoldersChange={refreshStatus}
        recentSearches={recentSearches}
        onSelectSearch={handleSelectSearch}
        onRemoveSearch={removeSearch}
        onClearSearches={clearSearches}
      />

      {/* 메인 콘텐츠 */}
      <div
        className={`flex flex-col h-screen transition-all duration-300 ease-in-out
          ${sidebarOpen ? "pl-[var(--sidebar-width)]" : "pl-0"}`}
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
              isIndexing={isIndexing}
              isSidebarOpen={sidebarOpen}
            />
          </div>
        )}

        {/* Scrollable Content Area */}
        <div
          ref={scrollContainerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto overflow-x-hidden"
        >
          {/* Search Bar + Filters Area */}
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
              />

              {/* 벡터 인덱싱 상태 배너 */}
              {isVectorIndexing && (
                <div
                  className="max-w-4xl mx-auto mt-2 px-3 py-2 rounded-lg flex items-center justify-between text-xs"
                  style={{
                    backgroundColor: "rgba(59, 130, 246, 0.1)",
                    border: "1px solid rgba(59, 130, 246, 0.2)",
                    color: "var(--color-text-secondary)",
                  }}
                >
                  <div className="flex items-center gap-2">
                    <div className="animate-spin h-3 w-3 border border-blue-400 border-t-transparent rounded-full" />
                    <span>벡터 인덱싱 중... ({vectorProgress}%) — 키워드 검색만 가능</span>
                  </div>
                  <button onClick={cancelVectorIndexing} className="text-blue-400 hover:text-blue-300 font-medium">취소</button>
                </div>
              )}

              {/* 필터 바 */}
              {query && (results.length > 0 || filenameResults.length > 0) && (
                <div className="max-w-4xl mx-auto mt-2 pb-3 border-b" style={{ borderColor: "var(--color-border)" }}>
                  <SearchFilters
                    filters={filters}
                    onFiltersChange={setFilters}
                    showRefineSearch={results.length > 0 || filenameResults.length > 0}
                    searchMode={searchMode}
                    refineQuery={refineQuery}
                    onRefineQueryChange={setRefineQuery}
                    onRefineQueryClear={clearRefine}
                  />
                </div>
              )}

              {error && <div className="mt-3"><ErrorBanner message={error} onDismiss={clearError} /></div>}
            </div>
          )}

          {isCollapsed && error && (
            <div className="px-6 pt-2"><ErrorBanner message={error} onDismiss={clearError} /></div>
          )}

          <main className="px-6 pb-20 transition-all duration-150">
            <div className="max-w-4xl mx-auto mt-4">
              <SearchResultList
                results={filteredResults}
                filenameResults={filters.excludeFilename ? [] : filenameResults}
                groupedResults={groupedResults}
                viewMode={viewMode}
                onViewModeChange={setViewMode}
                viewDensity={viewDensity}
                onViewDensityChange={setViewDensity}
                query={query}
                isLoading={isLoading}
                selectedIndex={selectedIndex}
                onOpenFile={handleOpenFile}
                onCopyPath={handleCopyPath}
                onOpenFolder={handleOpenFolder}
                onExportCSV={() => exportToCSV(filteredResults, query)}
                onCopyAll={() => copyToClipboard(filteredResults, query)}
                refineKeywords={refineQuery.trim() ? refineQuery.trim().split(/\s+/) : undefined}
                resultCount={filteredResults.length}
                totalResultCount={results.length}
                minConfidence={minConfidence}
                searchTime={searchTime}
                scrollContainerRef={scrollContainerRef}
              />
            </div>
          </main>
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

      <SettingsModal
        isOpen={settingsOpen}
        onClose={() => {
          setSettingsOpen(false);
          // Modal cleanup이 설정 버튼으로 포커스 복원한 후 검색창으로 이동
          setTimeout(() => searchInputRef.current?.focus(), 0);
        }}
        onThemeChange={setTheme}
        onSettingsSaved={(settings) => {
          applySettings(settings);
          if (!(settings.semantic_search_enabled ?? false) && isVectorIndexing) {
            cancelVectorIndexing();
          }
        }}
        onClearData={async () => {
          await invoke("clear_all_data");
          clearSearchCache();
          await refreshStatus();
        }}
      />

      <HelpModal isOpen={helpOpen} onClose={() => setHelpOpen(false)} />
      <DisclaimerModal isOpen={showDisclaimer} onAccept={acceptDisclaimer} onExit={exitApp} />
      <OnboardingModal isOpen={showOnboarding} onComplete={completeOnboarding} onSkip={skipOnboarding} />
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />

      {vectorStatus?.is_running && (
        <VectorIndexingFAB
          progress={vectorProgress}
          totalChunks={vectorStatus.total_chunks}
          processedChunks={vectorStatus.processed_chunks}
          currentFile={vectorStatus.current_file}
          onCancel={cancelVectorIndexing}
        />
      )}

      {showScrollTop && !vectorStatus?.is_running && (
        <button
          onClick={scrollToTop}
          className="fixed bottom-20 right-6 w-10 h-10 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-105 z-40"
          style={{ backgroundColor: "var(--color-bg-secondary)", border: "1px solid var(--color-border)", boxShadow: "0 2px 8px rgba(0,0,0,0.15)" }}
          aria-label="맨 위로 스크롤"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24" style={{ color: "var(--color-text-muted)" }}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 15l7-7 7 7" />
          </svg>
        </button>
      )}
    </div>
  );
}

export default App;
