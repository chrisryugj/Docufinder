import { useRef, useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Hooks
import { useSearch, useIndexStatus, useVectorIndexing, useKeyboardShortcuts, useRecentSearches, useExport, useToast, useTheme, useCollapsibleSearch } from "./hooks";
import { clearSearchCache } from "./hooks/useSearch";
import { useFirstRun } from "./hooks/useFirstRun";
import { useFileActions } from "./hooks/useFileActions";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAppEvents } from "./hooks/useAppEvents";
import { useUpdater } from "./hooks/useUpdater";
import { setupGlobalErrorHandlers } from "./utils/errorLogger";

// Components
import { Header, StatusBar, ErrorBanner, AppModals, FloatingUI } from "./components/layout";
import { SearchBar, SearchFilters, SearchResultList, CompactSearchBar } from "./components/search";
import { IndexingReportModal } from "./components/search/IndexingReportModal";
import { Sidebar } from "./components/sidebar";
import { ToastContainer } from "./components/ui/Toast";
import { UpdateBanner } from "./components/ui/UpdateBanner";
import { Modal } from "./components/ui/Modal";
import { Button } from "./components/ui/Button";
import type { Settings } from "./types/settings";
import type { AddFolderResult } from "./types/index";

function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const compactSearchInputRef = useRef<HTMLInputElement>(null);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState<number>(-1);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [reportResults, setReportResults] = useState<AddFolderResult[]>([]);
  const [showAutoIndexPrompt, setShowAutoIndexPrompt] = useState(false);
  const autoIndexPromptShownRef = useRef(false);

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
    setViewDensity,
    semanticEnabled,
    vectorIndexingMode,
    resultsPerPage,
    applySettings,
  } = useAppSettings({ setSearchMode });

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

  // Tauri 이벤트 리스너 (증분 인덱싱 + 모델 다운로드)
  useAppEvents({ query, invalidateSearch, refreshStatus, refreshVectorStatus, showToast, updateToast });

  // OTA 자동 업데이트
  const updater = useUpdater();

  // 내보내기 (토스트 연동)
  const { exportToCSV, copyToClipboard } = useExport({ showToast });

  // 에러 통합 (검색 + 인덱싱 + 벡터)
  const error = searchError || indexError || vectorError;
  const clearError = useCallback(() => {
    clearSearchError();
    clearIndexError();
    clearVectorError();
  }, [clearSearchError, clearIndexError, clearVectorError]);

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
      // 반환값으로 최신 상태 확인 (stale closure 방지)
      refreshVectorStatus().then((freshStatus) => {
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
            />

            {/* 벡터 인덱싱 상태 배너 */}
            {isVectorIndexing && (
              <div
                className="max-w-4xl mx-auto mt-2 px-3 py-2 rounded-lg flex items-center justify-between text-xs"
                style={{
                  backgroundColor: "var(--color-accent-subtle, rgba(59, 130, 246, 0.1))",
                  border: "1px solid var(--color-accent-border, rgba(59, 130, 246, 0.2))",
                  color: "var(--color-text-secondary)",
                }}
              >
                <div className="flex items-center gap-2">
                  <div className="animate-spin h-3 w-3 rounded-full" style={{ border: "1px solid var(--color-accent)", borderTopColor: "transparent" }} />
                  <span>벡터 인덱싱 중... ({vectorProgress}%) — 키워드 검색만 가능</span>
                </div>
                <button onClick={cancelVectorIndexing} className="font-medium" style={{ color: "var(--color-accent)" }}>취소</button>
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
                  watchedFolders={status?.watched_folders ?? []}
                />
              </div>
            )}

            {error && <div className="mt-3"><ErrorBanner message={error} onDismiss={clearError} /></div>}
          </div>
        )}

        {/* Scrollable Content Area — 검색 결과만 포함 (스크롤 점프 원천 차단) */}
        <div
          ref={scrollContainerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto overflow-x-hidden"
          style={{ overflowAnchor: "none" }}
        >
          {isCollapsed && error && (
            <div className="px-6 pt-2"><ErrorBanner message={error} onDismiss={clearError} /></div>
          )}

          <main className="px-6 pb-20">
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
                resultsPerPage={resultsPerPage}
                indexedFiles={status?.indexed_files ?? 0}
                indexedFolders={status?.watched_folders?.length ?? 0}
                recentSearches={recentSearches}
                onSelectSearch={handleSelectSearch}
                semanticEnabled={semanticEnabled}
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
        isOpen={reportResults.length > 0}
        onClose={() => setReportResults([])}
        results={reportResults}
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
              console.error("Re-indexing failed for", folder, err);
            }
          }
          showToast(`${indexedCount}개 HWPX 파일 인덱싱 완료`, "success");
          refreshStatus();
        }}
      />

      {/* 자동 인덱싱 안내 다이얼로그 */}
      <Modal
        isOpen={showAutoIndexPrompt}
        onClose={() => setShowAutoIndexPrompt(false)}
        title="문서 검색을 시작하세요"
        size="sm"
      >
        <div className="space-y-4">
          <p className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
            등록된 폴더가 없습니다. 어떻게 시작할까요?
          </p>
          <div className="space-y-2">
            <Button
              className="w-full justify-center"
              onClick={async () => {
                setShowAutoIndexPrompt(false);
                await autoIndexAllDrives();
              }}
            >
              전체 드라이브 인덱싱
            </Button>
            <p className="text-xs text-center" style={{ color: "var(--color-text-muted)" }}>
              모든 드라이브를 스캔합니다 (시스템 폴더 자동 제외)
            </p>
            <Button
              variant="ghost"
              className="w-full justify-center"
              onClick={async () => {
                setShowAutoIndexPrompt(false);
                await handleAddFolder();
              }}
            >
              폴더 직접 선택
            </Button>
            <div className="pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
              <button
                className="w-full text-center text-xs py-1.5"
                style={{ color: "var(--color-text-muted)" }}
                onClick={() => setShowAutoIndexPrompt(false)}
              >
                나중에 할게요
              </button>
            </div>
          </div>
        </div>
      </Modal>

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
