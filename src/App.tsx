import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface SearchResult {
  file_path: string;
  file_name: string;
  chunk_index: number;
  content_preview: string;
  full_content: string;
  score: number;
  highlight_ranges: [number, number][];
  page_number: number | null;
  start_offset: number;
  /** 위치 힌트 (XLSX: "Sheet1!행1-50", PDF: "페이지 3", HWPX: "섹션 2" 등) */
  location_hint: string | null;
}

// 하이라이트 적용된 텍스트 렌더링
function HighlightedText({
  text,
  ranges,
}: {
  text: string;
  ranges: [number, number][];
}) {
  if (!ranges || ranges.length === 0) {
    return <>{text}</>;
  }

  // 범위 정렬 (시작 위치 기준)
  const sortedRanges = [...ranges].sort((a, b) => a[0] - b[0]);
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;

  sortedRanges.forEach(([start, end], i) => {
    // 하이라이트 전 텍스트
    if (start > lastIndex) {
      parts.push(text.slice(lastIndex, start));
    }
    // 하이라이트 텍스트
    parts.push(
      <mark
        key={i}
        className="bg-yellow-500/30 text-yellow-200 rounded px-0.5"
      >
        {text.slice(start, end)}
      </mark>
    );
    lastIndex = end;
  });

  // 마지막 남은 텍스트
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return <>{parts}</>;
}

interface SearchResponse {
  results: SearchResult[];
  total_count: number;
  search_time_ms: number;
}

interface IndexStatus {
  total_files: number;
  indexed_files: number;
  watched_folders: string[];
  vectors_count: number;
  semantic_available: boolean;
}

interface AddFolderResult {
  success: boolean;
  indexed_count: number;
  failed_count: number;
  message: string;
}

type SearchMode = "keyword" | "semantic" | "hybrid";

const SEARCH_MODES: { value: SearchMode; label: string; desc: string }[] = [
  { value: "keyword", label: "키워드", desc: "FTS5 전문검색" },
  { value: "hybrid", label: "하이브리드", desc: "키워드 + AI 통합" },
  { value: "semantic", label: "시맨틱", desc: "AI 의미 검색" },
];

function App() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searchTime, setSearchTime] = useState<number | null>(null);
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isIndexing, setIsIndexing] = useState(false);
  const [searchMode, setSearchMode] = useState<SearchMode>("keyword");
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 인덱스 상태 조회
  const fetchStatus = useCallback(async () => {
    try {
      const result = await invoke<IndexStatus>("get_index_status");
      setStatus(result);
    } catch (error) {
      console.error("Failed to get status:", error);
    }
  }, []);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  // 폴더 선택 및 인덱싱
  const handleAddFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "인덱싱할 폴더 선택",
      });

      if (selected) {
        setIsIndexing(true);
        setError(null);
        const result = await invoke<AddFolderResult>("add_folder", {
          path: selected,
        });
        console.log("Indexing result:", result);
        await fetchStatus();
        setIsIndexing(false);
      }
    } catch (err) {
      console.error("Failed to add folder:", err);
      const message = err instanceof Error ? err.message : String(err);
      setError(`폴더 추가 실패: ${message}`);
      setIsIndexing(false);
    }
  };

  // 검색 실행
  const handleSearch = async (searchQuery: string, mode: SearchMode) => {
    if (!searchQuery.trim()) {
      setResults([]);
      setSearchTime(null);
      return;
    }

    setIsLoading(true);
    setError(null);
    try {
      const commandMap: Record<SearchMode, string> = {
        keyword: "search_keyword",
        semantic: "search_semantic",
        hybrid: "search_hybrid",
      };
      const response = await invoke<SearchResponse>(commandMap[mode], {
        query: searchQuery,
      });
      setResults(response.results);
      setSearchTime(response.search_time_ms);
    } catch (err) {
      console.error("Search failed:", err);
      const message = err instanceof Error ? err.message : String(err);
      setError(`검색 실패: ${message}`);
      setResults([]);
      setSearchTime(null);
    }
    setIsLoading(false);
  };

  // 디바운스 검색
  useEffect(() => {
    const timer = setTimeout(() => {
      handleSearch(query, searchMode);
    }, 300);

    return () => clearTimeout(timer);
  }, [query, searchMode]);

  // 파일 열기 (페이지 지정 가능)
  const handleOpenFile = async (filePath: string, page?: number | null) => {
    try {
      await invoke("open_file", { path: filePath, page: page ?? null });
    } catch (error) {
      console.error("Failed to open file:", error);
    }
  };

  return (
    <div className="min-h-screen bg-gray-900 text-white flex flex-col">
      {/* Header */}
      <header className="border-b border-gray-800 px-6 py-4 flex justify-between items-center">
        <div>
          <h1 className="text-2xl font-bold">DocuFinder</h1>
          <p className="text-gray-400 text-sm">로컬 문서 검색 시스템</p>
        </div>
        <button
          onClick={handleAddFolder}
          disabled={isIndexing}
          className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600
                     rounded-lg text-sm font-medium transition-colors"
        >
          {isIndexing ? "인덱싱 중..." : "폴더 추가"}
        </button>
      </header>

      {/* Search Bar */}
      <div className="p-6 border-b border-gray-800">
        <div className="max-w-2xl mx-auto">
          <div className="relative">
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="검색어를 입력하세요..."
              className="w-full px-4 py-3 pl-12 bg-gray-800 border border-gray-700 rounded-lg
                         focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent
                         text-white placeholder-gray-500"
            />
            <svg
              className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-500"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
            {isLoading && (
              <div className="absolute right-4 top-1/2 -translate-y-1/2">
                <div className="w-5 h-5 border-2 border-gray-500 border-t-blue-500 rounded-full animate-spin" />
              </div>
            )}
          </div>
          {/* 검색 모드 선택 */}
          <div className="flex gap-2 mt-3">
            {SEARCH_MODES.map((mode) => {
              const needsSemantic = mode.value !== "keyword";
              const disabled = needsSemantic && !status?.semantic_available;
              return (
                <button
                  key={mode.value}
                  onClick={() => !disabled && setSearchMode(mode.value)}
                  disabled={disabled}
                  className={`px-3 py-1.5 text-sm rounded-lg transition-colors ${
                    searchMode === mode.value
                      ? "bg-blue-600 text-white"
                      : disabled
                        ? "bg-gray-800 text-gray-600 cursor-not-allowed"
                        : "bg-gray-800 text-gray-400 hover:bg-gray-700"
                  }`}
                  title={disabled ? "모델 파일 필요" : mode.desc}
                >
                  {mode.label}
                </button>
              );
            })}
            {searchTime !== null && results.length > 0 && (
              <span className="ml-auto text-gray-500 text-sm self-center">
                {results.length}개 결과 ({searchTime}ms)
              </span>
            )}
          </div>
          {/* 에러 메시지 */}
          {error && (
            <div className="mt-3 p-3 bg-red-900/30 border border-red-600/50 rounded-lg flex items-center justify-between">
              <span className="text-red-300 text-sm">{error}</span>
              <button
                onClick={() => setError(null)}
                className="text-red-400 hover:text-red-300 ml-2"
              >
                ✕
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Results Area */}
      <main className="flex-1 px-6 py-4 overflow-auto">
        <div className="max-w-4xl mx-auto">
          {results.length > 0 ? (
            <div className="space-y-3">
              {results.map((result, index) => {
                const isExpanded = expandedIndex === index;
                return (
                  <div
                    key={`${result.file_path}-${result.chunk_index}-${index}`}
                    className="bg-gray-800 rounded-lg p-4 border border-gray-700 hover:border-gray-600 transition-colors"
                  >
                    {/* 헤더 */}
                    <div className="flex items-start justify-between mb-2">
                      <div
                        className="flex items-center gap-2 cursor-pointer hover:text-blue-400"
                        onClick={() => handleOpenFile(result.file_path, result.page_number)}
                        title={result.page_number ? `${result.page_number}페이지로 열기` : "파일 열기"}
                      >
                        <svg
                          className="w-5 h-5 text-blue-400 flex-shrink-0"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                          />
                        </svg>
                        <span className="font-medium text-white">
                          {result.file_name}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 text-xs">
                        {result.location_hint ? (
                          <span className="bg-green-600/30 text-green-300 px-2 py-0.5 rounded font-medium">
                            {result.location_hint}
                          </span>
                        ) : result.page_number ? (
                          <span className="bg-blue-600/30 text-blue-300 px-2 py-0.5 rounded font-medium">
                            {result.page_number}페이지
                          </span>
                        ) : (
                          <span className="bg-gray-700 text-gray-400 px-2 py-0.5 rounded">
                            섹션 {result.chunk_index + 1}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* 내용 */}
                    <div
                      className="cursor-pointer"
                      onClick={() => setExpandedIndex(isExpanded ? null : index)}
                    >
                      <p className="text-gray-300 text-sm leading-relaxed">
                        <HighlightedText
                          text={isExpanded ? result.full_content : result.content_preview}
                          ranges={result.highlight_ranges}
                        />
                      </p>
                      {!isExpanded && result.full_content.length > result.content_preview.length && (
                        <button className="text-blue-400 text-xs mt-1 hover:underline">
                          더보기 ▼
                        </button>
                      )}
                      {isExpanded && (
                        <button className="text-blue-400 text-xs mt-1 hover:underline">
                          접기 ▲
                        </button>
                      )}
                    </div>

                    {/* 경로 */}
                    <p className="text-gray-500 text-xs mt-2 truncate">
                      {result.file_path}
                    </p>
                  </div>
                );
              })}
            </div>
          ) : query.trim() && !isLoading ? (
            <div className="text-center text-gray-500 py-12">
              <svg
                className="w-16 h-16 mx-auto mb-4 opacity-50"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <p>검색 결과가 없습니다</p>
            </div>
          ) : (
            <div className="text-center text-gray-500 py-12">
              <svg
                className="w-16 h-16 mx-auto mb-4 opacity-50"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
              <p>폴더를 선택하고 검색을 시작하세요</p>
            </div>
          )}
        </div>
      </main>

      {/* Status Bar */}
      <footer className="border-t border-gray-700 bg-gray-800 px-4 py-2">
        <div className="flex justify-between text-sm text-gray-400">
          <span>인덱싱된 문서: {status?.total_files ?? 0}개</span>
          <span>
            {status?.watched_folders.length
              ? `폴더: ${status.watched_folders.length}개`
              : "폴더를 추가하세요"}
          </span>
        </div>
      </footer>
    </div>
  );
}

export default App;
