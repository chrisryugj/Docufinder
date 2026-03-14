import { useCallback, useEffect } from "react";

// Hooks (기존 로직 그대로 재사용)
import { useSearch, useIndexStatus, useVectorIndexing, useToast, useRecentSearches, useTheme } from "./hooks";
import { useFirstRun } from "./hooks/useFirstRun";
import { useFileActions } from "./hooks/useFileActions";
import { useAppSettings } from "./hooks/useAppSettings";
import { setupGlobalErrorHandlers } from "./utils/errorLogger";

import { ToastContainer } from "./components/ui/Toast";

// V2 전용 컴포넌트 임포트
import { HeaderV2 } from "./components/v2/layout/HeaderV2";
import { SidebarV2 } from "./components/v2/layout/SidebarV2";
import { SearchBarV2 } from "./components/v2/search/SearchBarV2";
import { SearchResultListV2 } from "./components/v2/search/SearchResultListV2";

export default function AppV2() {
  // 테마 관리 (설정 모달 전환 등에서 사용)
  useTheme();

  // 첫 실행
  useFirstRun();

  // 검색 상태
  const {
    query,
    setQuery,
    filteredResults,
    searchTime,
    isLoading,
    searchMode,
    setSearchMode,
    setComposing,
    invalidate: invalidateSearch,
  } = useSearch({ minConfidence: 0 });

  // 인덱스 상태
  const {
    status,
    isIndexing,
    addFolder,
    addFolderByPath,
    removeFolder,
  } = useIndexStatus();

  // 최근 검색 (현재 V2 미구현)
  const {
    addSearch,
  } = useRecentSearches();

  // 토스트 알림
  const { toasts, showToast, updateToast, dismissToast } = useToast();

  // 벡터 인덱싱
  const {
    refreshStatus: refreshVectorStatus,
  } = useVectorIndexing();

  // 앱 설정
  // V2 진행 중 설정 로직은 추후 연결
  useAppSettings({ setSearchMode });

  // 파일/폴더 액션
  const {
    handleOpenFile,
    handleOpenFolder,
    handleAddFolder: rawHandleAddFolder,
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

  const handleAddFolder = useCallback(async () => {
    const results = await rawHandleAddFolder();
    // V2에서는 에러/HWP 처리를 모달 말고 토스트로 나중에 빼거나 생략할 수 있음
    return results;
  }, [rawHandleAddFolder]);

  useEffect(() => {
    setupGlobalErrorHandlers();
  }, []);

  return (
    <div className="v2-theme min-h-screen bg-[#F8FAFC] dark:bg-[var(--color-bg-primary)] text-slate-800 dark:text-slate-200 font-sans selection:bg-blue-500/30 overflow-hidden flex flex-col">
      {/* V2 메인 레이아웃 (추후 분리 예정) */}
      <div className="flex flex-1 h-full overflow-hidden">
        
        {/* V2 사이드바 적용 */}
        <SidebarV2 />

        {/* 메인 뷰 */}
        <main className="flex-1 flex flex-col relative z-0 overflow-hidden bg-slate-50/30 dark:bg-transparent">
          {/* 상단 앱 헤더 */}
          <HeaderV2 
            onAddFolder={handleAddFolder}
            onOpenSettings={() => {}}
            onOpenHelp={() => {}} 
            isIndexing={isIndexing}
          />

          <div className="flex-1 overflow-y-auto px-6 lg:px-12 py-8 scroll-smooth" style={{ scrollbarGutter: 'stable' }}>
            <div className="max-w-4xl mx-auto w-full">
              
              <div className="mb-10 text-center animate-slide-up" style={{ animationDelay: '0.1s' }}>
                <h2 className="text-[2.5rem] font-bold mb-3 tracking-tighter text-slate-800 dark:text-slate-100 v2-font-heading">내 PC의 모든 지식을 한 곳에서</h2>
                <p className="text-lg text-slate-500 dark:text-slate-400 font-medium">문서 형식에 상관없이, 단 0.1초 만에 필요한 내용을 찾아냅니다.</p>
              </div>
              
              <div className="animate-slide-up" style={{ animationDelay: '0.2s' }}>
                <SearchBarV2 
                  query={query}
                  onQueryChange={setQuery}
                  onCompositionStart={() => setComposing(true)}
                  onCompositionEnd={(finalValue) => setComposing(false, finalValue)}
                  isLoading={isLoading}
                  status={status}
                  resultCount={filteredResults.length}
                  searchTime={searchTime || 0}
                  searchMode={searchMode as 'keyword' | 'semantic' | 'hybrid'}
                  onSearchModeChange={setSearchMode}
                />
              </div>

              {/* 검색 결과 리스트 */}
              <div className="animate-slide-up" style={{ animationDelay: '0.3s' }}>
                <SearchResultListV2 
                  results={filteredResults}
                  selectedIndex={-1}
                  onOpenFile={handleOpenFile}
                  onOpenFolder={handleOpenFolder}
                />
              </div>

            </div>
          </div>
        </main>
      </div>
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
