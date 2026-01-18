import { useRef, useState, useCallback, useEffect, UIEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Settings, ViewDensity } from "./types/settings";

// 색상 밝기 판단 헬퍼 (hex -> 밝은지 어두운지)
function isLightColor(hex: string): boolean {
  // hex -> rgb 변환
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) return true;

  const r = parseInt(result[1], 16);
  const g = parseInt(result[2], 16);
  const b = parseInt(result[3], 16);

  // 상대 밝기 계산 (YIQ 공식)
  const brightness = (r * 299 + g * 587 + b * 114) / 1000;
  return brightness > 128;
}

// Hooks
import { useSearch, useIndexStatus, useVectorIndexing, useKeyboardShortcuts, useRecentSearches, useExport, useToast, useTheme, useCollapsibleSearch } from "./hooks";

// Components
import { Header, StatusBar, ErrorBanner } from "./components/layout";
import { SearchBar, SearchFilters, SearchResultList, CompactSearchBar } from "./components/search";
import { Sidebar } from "./components/sidebar";
import { SettingsModal } from "./components/settings/SettingsModal";
import { HelpModal } from "./components/help/HelpModal";
import { ToastContainer } from "./components/ui/Toast";
import { VectorIndexingFAB } from "./components/ui/VectorIndexingFAB";

function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const compactSearchInputRef = useRef<HTMLInputElement>(null);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [minConfidence, setMinConfidence] = useState(0);
  const [viewDensity, setViewDensity] = useState<ViewDensity>("compact");

  // 스크롤 기반 검색 영역 축소
  const {
    isCollapsed,
    handleScroll,
    scrollToTop,
    scrollContainerRef,
    scrollTop,
    expand,
  } = useCollapsibleSearch({
    threshold: 200,  // 200px 이상 스크롤 시 축소 (깜박임 방지)
    onCollapse: () => searchInputRef.current?.blur(),
  });

  // 맨 위로 버튼 표시 여부
  const showScrollTop = scrollTop > 300;

  // 테마
  const { setTheme } = useTheme();

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
  } = useSearch({ debounceMs: 300, minConfidence });

  // 인덱스 상태
  const {
    status,
    isIndexing,
    progress,
    error: indexError,
    clearError: clearIndexError,
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
  } = useVectorIndexing();

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

  // 하이라이트 색상 적용 함수
  const applyHighlightColors = useCallback((settings: Settings) => {
    const root = document.documentElement;

    // 파일명 하이라이트 색상
    if (settings.highlight_filename_color) {
      root.style.setProperty("--color-highlight-filename-bg", settings.highlight_filename_color);
      // 텍스트 색상은 배경 밝기에 따라 자동 조정 (밝으면 어두운 글자, 어두우면 밝은 글자)
      const isLightBg = isLightColor(settings.highlight_filename_color);
      root.style.setProperty("--color-highlight-filename-text", isLightBg ? "#0f172a" : "#fef3c7");
    } else {
      root.style.removeProperty("--color-highlight-filename-bg");
      root.style.removeProperty("--color-highlight-filename-text");
    }

    // 문서 내용 하이라이트 색상
    if (settings.highlight_content_color) {
      root.style.setProperty("--color-highlight-bg", settings.highlight_content_color);
      const isLightBg = isLightColor(settings.highlight_content_color);
      root.style.setProperty("--color-highlight-text", isLightBg ? "#0f172a" : "#fef08a");
    } else {
      root.style.removeProperty("--color-highlight-bg");
      root.style.removeProperty("--color-highlight-text");
    }
  }, []);

  // 설정 로드 (검색 모드, 최소 신뢰도, 보기 밀도, 하이라이트 색상 적용)
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await invoke<Settings>("get_settings");
        setSearchMode(settings.search_mode ?? "hybrid");
        setMinConfidence(settings.min_confidence ?? 0);
        setViewDensity(settings.view_density ?? "compact");
        applyHighlightColors(settings);
      } catch (err) {
        console.warn("Failed to load settings:", err);
      }
    };

    loadSettings();
  }, [setSearchMode, applyHighlightColors]);

  // 초기 자동 포커스 제거 - 사용자가 클릭할 때만 포커스
  // (자동 포커스 시 Windows IME 팝업 문제 발생)

  // 윈도우 포커스 복귀 시 검색창 포커스 재설정 (IME 전환 안정화)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const resetSearchFocus = () => {
      if (settingsOpen) return;
      const input = searchInputRef.current;
      if (!input) return;

      const activeElement = document.activeElement;
      const isEditable =
        activeElement?.tagName === "INPUT" ||
        activeElement?.tagName === "TEXTAREA" ||
        (activeElement instanceof HTMLElement && activeElement.isContentEditable);

      if (isEditable && activeElement !== input) {
        return;
      }

      if (activeElement === input) {
        input.blur();
      }

      requestAnimationFrame(() => {
        input.focus();
      });
    };

    const setup = async () => {
      const window = getCurrentWindow();
      try {
        // 초기 로드 시에는 포커스 안 줌 (IME 팝업 위치 문제 방지)
        // 창 복귀 시에만 포커스 재설정
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

  useEffect(() => {
    if (searchMode !== "hybrid" && filters.keywordOnly) {
      setFilters({ ...filters, keywordOnly: false });
    }
  }, [searchMode, filters, setFilters]);

  // 파일 열기 (검색 결과 클릭 시 최근 검색에 저장)
  const handleOpenFile = useCallback(
    async (filePath: string, page?: number | null) => {
      // 검색 결과 클릭 시 최근 검색에 저장
      const trimmedQuery = query.trim();
      if (trimmedQuery.length >= 2) {
        addSearch(trimmedQuery);
      }

      const toastId = showToast("파일 여는 중...", "loading");
      try {
        await invoke("open_file", { path: filePath, page: page ?? null });
        updateToast(toastId, { message: "파일을 열었습니다", type: "success" });
      } catch (err) {
        console.error("Failed to open file:", err);
        updateToast(toastId, { message: "파일 열기 실패", type: "error" });
      }
    },
    [query, addSearch, showToast, updateToast]
  );

  // 경로 복사 (\\?\ 접두사 제거)
  const handleCopyPath = useCallback(async (path: string) => {
    try {
      const cleanPath = path.replace(/^\\\\\?\\/, "");
      await navigator.clipboard.writeText(cleanPath);
      showToast("경로가 복사되었습니다", "success");
    } catch (err) {
      console.error("Failed to copy path:", err);
      showToast("경로 복사 실패", "error");
    }
  }, [showToast]);

  // 폴더 열기 (\\?\ 접두사 제거)
  const handleOpenFolder = useCallback(async (folderPath: string) => {
    try {
      const cleanPath = folderPath.replace(/^\\\\\?\\/, "");
      await invoke("open_folder", { path: cleanPath });
    } catch (err) {
      console.error("Failed to open folder:", err);
      showToast("폴더 열기 실패", "error");
    }
  }, [showToast]);

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

  // 검색어 변경 (저장은 별도 로직에서 처리)
  const handleQueryChange = useCallback(
    (newQuery: string) => {
      setQuery(newQuery);
    },
    [setQuery]
  );

  // 검색 결과가 있고 3초 유지 시 최근 검색에 저장
  useEffect(() => {
    // 이전 타이머 취소
    if (searchTimerRef.current) {
      clearTimeout(searchTimerRef.current);
      searchTimerRef.current = null;
    }

    // 검색어 2자 이상 + 결과 있을 때만 저장 예약
    const trimmedQuery = query.trim();
    if (trimmedQuery.length >= 2 && filteredResults.length > 0) {
      searchTimerRef.current = setTimeout(() => {
        addSearch(trimmedQuery);
        searchTimerRef.current = null;
      }, 3000); // 3초 유지 시 저장
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
  if (prevResultsLength.current !== filteredResults.length) {
    prevResultsLength.current = filteredResults.length;
    if (selectedIndex >= filteredResults.length) {
      setSelectedIndex(filteredResults.length > 0 ? 0 : -1);
    }
  }

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
        onAddFolder={addFolder}
        onRemoveFolder={removeFolder}
        recentSearches={recentSearches}
        onSelectSearch={handleSelectSearch}
        onRemoveSearch={removeSearch}
        onClearSearches={clearSearches}
      />

      {/* 메인 콘텐츠 (사이드바 열림에 따라 전체 이동) */}
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
              searchMode={searchMode}
              onSearchModeChange={setSearchMode}
              isLoading={isLoading}
              status={status}
              resultCount={filteredResults.length}
              onExpand={handleExpand}
              onAddFolder={addFolder}
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

        {/* Expanded Header (스크롤 상단에서 표시) */}
        {!isCollapsed && (
          <div className="sticky top-0 z-20 bg-[var(--color-bg-primary)]/90 backdrop-blur-md border-b" style={{ borderColor: 'var(--color-border)' }}>
            <Header
              onAddFolder={addFolder}
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
          {/* Search Bar + Filters Area (스크롤 상단에서만 표시) */}
          {!isCollapsed && (
            <div className="px-4 pt-4 pb-2">
              <SearchBar
                ref={searchInputRef}
                query={query}
                onQueryChange={handleQueryChange}
                searchMode={searchMode}
                onSearchModeChange={setSearchMode}
                isLoading={isLoading}
                status={status}
                resultCount={filteredResults.length}
                searchTime={searchTime}
              />

              {/* 필터 바 (검색바 바로 아래) */}
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

              {/* 에러 메시지 */}
              {error && <div className="mt-3"><ErrorBanner message={error} onDismiss={clearError} /></div>}
            </div>
          )}

          {/* 에러 메시지 (컴팩트 모드) */}
          {isCollapsed && error && (
            <div className="px-6 pt-2"><ErrorBanner message={error} onDismiss={clearError} /></div>
          )}

          {/* Results Area */}
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
              />
            </div>
          </main>
        </div>

        {/* Status Bar (Fixed at bottom) */}
        <StatusBar status={status} progress={progress} onCancelIndexing={cancelIndexing} />
      </div>

      {/* Settings Modal */}
      <SettingsModal
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onThemeChange={setTheme}
        onSettingsSaved={(settings) => {
          setSearchMode(settings.search_mode ?? "hybrid");
          setMinConfidence(settings.min_confidence ?? 0);
          setViewDensity(settings.view_density ?? "compact");
          applyHighlightColors(settings);
        }}
      />

      {/* Help Modal */}
      <HelpModal isOpen={helpOpen} onClose={() => setHelpOpen(false)} />

      {/* Toast Container */}
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />

      {/* Vector Indexing FAB (2단계 백그라운드 인덱싱 진행률) */}
      {vectorStatus?.is_running && (
        <VectorIndexingFAB
          progress={vectorProgress}
          totalChunks={vectorStatus.total_chunks}
          processedChunks={vectorStatus.processed_chunks}
          currentFile={vectorStatus.current_file}
          onCancel={cancelVectorIndexing}
        />
      )}

      {/* Scroll to Top FAB */}
      {showScrollTop && !vectorStatus?.is_running && (
        <button
          onClick={scrollToTop}
          className="fixed bottom-20 right-6 w-10 h-10 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-105 z-40"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            border: "1px solid var(--color-border)",
            boxShadow: "0 2px 8px rgba(0,0,0,0.15)",
          }}
          aria-label="맨 위로 스크롤"
        >
          <svg
            className="w-5 h-5"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
            style={{ color: "var(--color-text-muted)" }}
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 15l7-7 7 7" />
          </svg>
        </button>
      )}
    </div>
  );
}

export default App;
