import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, FileText, FolderOpen, Loader2, Search } from "lucide-react";
import { Modal } from "../ui/Modal";
import { Badge } from "../ui/Badge";
import { FileIcon } from "../ui/FileIcon";

interface DuplicateFile {
  file_path: string;
  file_name: string;
  file_type: string;
  size: number;
  modified_at: number | null;
}

interface DuplicateGroup {
  files: DuplicateFile[];
  duplicate_type: "exact" | "similar";
  similarity: number;
}

interface DuplicateResponse {
  exact_groups: DuplicateGroup[];
  similar_groups: DuplicateGroup[];
  scan_time_ms: number;
  total_files_scanned: number;
}

interface DuplicateFinderModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
  showToast: (msg: string, type: "success" | "error" | "info") => string;
}

export function DuplicateFinderModal({
  isOpen,
  onClose,
  onOpenFile,
  onOpenFolder,
  showToast,
}: DuplicateFinderModalProps) {
  const [result, setResult] = useState<DuplicateResponse | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [tab, setTab] = useState<"exact" | "similar">("exact");
  const [folders, setFolders] = useState<{ path: string }[]>([]);
  const [selectedFolder, setSelectedFolder] = useState<string>("");

  useEffect(() => {
    if (isOpen) {
      setResult(null);
      setSelectedFolder("");
      setTab("exact");
      invoke<{ path: string }[]>("get_folders_with_info")
        .then(setFolders)
        .catch(() => {});
    }
  }, [isOpen]);

  const handleScan = useCallback(async () => {
    setIsScanning(true);
    setResult(null);
    try {
      const res = await invoke<DuplicateResponse>("find_duplicates", {
        folderPath: selectedFolder || null,
      });
      setResult(res);
      const total = res.exact_groups.length + res.similar_groups.length;
      if (total === 0) {
        showToast("중복 문서가 발견되지 않았습니다", "info");
      } else {
        showToast(`${total}개 중복 그룹 발견 (${res.scan_time_ms}ms)`, "success");
      }
    } catch (e: unknown) {
      const msg = e && typeof e === "object" && "message" in e ? (e as { message: string }).message : String(e);
      showToast(`중복 탐지 실패: ${msg}`, "error");
    } finally {
      setIsScanning(false);
    }
  }, [showToast, selectedFolder]);

  const groups = result
    ? tab === "exact"
      ? result.exact_groups
      : result.similar_groups
    : [];

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDate = (ts: number | null) => {
    if (!ts) return "-";
    return new Date(ts * 1000).toLocaleDateString("ko-KR");
  };

  const copyPath = async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      showToast("경로가 복사되었습니다", "info");
    } catch {
      showToast("클립보드 복사 실패", "error");
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="중복 문서 탐지" size="lg">
      <div className="space-y-4">
        {/* 스캔 버튼 */}
        {!result && !isScanning && (
          <div className="text-center py-8">
            <div
              className="w-16 h-16 mx-auto mb-4 rounded-2xl flex items-center justify-center"
              style={{ backgroundColor: "var(--color-bg-tertiary)" }}
            >
              <Search className="w-8 h-8 opacity-60" style={{ color: "var(--color-text-muted)" }} />
            </div>
            <p className="text-sm mb-4" style={{ color: "var(--color-text-secondary)" }}>
              인덱싱된 문서에서 정확한 중복과 유사한 내용의 문서를 찾습니다
            </p>
            {folders.length > 0 && (
              <div className="flex items-center justify-center gap-2 mb-4">
                <FolderOpen className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
                <select
                  value={selectedFolder}
                  onChange={(e) => setSelectedFolder(e.target.value)}
                  className="px-3 py-1.5 text-sm rounded-lg border"
                  style={{
                    backgroundColor: "var(--color-bg-secondary)",
                    borderColor: "var(--color-border)",
                    color: "var(--color-text-primary)",
                  }}
                >
                  <option value="">전체 폴더</option>
                  {folders.map((f) => (
                    <option key={f.path} value={f.path}>
                      {f.path.replace(/\\/g, "/").split("/").slice(-2).join("/")}
                    </option>
                  ))}
                </select>
              </div>
            )}
            <button
              onClick={handleScan}
              className="px-4 py-2 rounded-lg text-sm font-medium text-white"
              style={{ backgroundColor: "var(--color-accent)" }}
            >
              중복 탐지 시작
            </button>
          </div>
        )}

        {/* 로딩 */}
        {isScanning && (
          <div className="text-center py-12">
            <Loader2
              className="w-8 h-8 mx-auto mb-3 animate-spin"
              style={{ color: "var(--color-accent)" }}
            />
            <p className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              문서 비교 중... (파일 해시 계산 + 벡터 유사도 분석)
            </p>
          </div>
        )}

        {/* 결과 */}
        {result && !isScanning && (
          <>
            {/* 요약 */}
            <div
              className="flex items-center gap-3 px-3 py-2 rounded-lg text-xs"
              style={{ backgroundColor: "var(--color-bg-tertiary)" }}
            >
              <span>
                스캔: <strong>{result.total_files_scanned}</strong>개 파일
              </span>
              <span>
                정확 중복: <strong>{result.exact_groups.length}</strong>그룹
              </span>
              <span>
                유사 중복: <strong>{result.similar_groups.length}</strong>그룹
              </span>
              <span className="ml-auto" style={{ color: "var(--color-text-muted)" }}>
                {result.scan_time_ms}ms
              </span>
              <button
                onClick={handleScan}
                className="px-2 py-0.5 text-xs rounded border"
                style={{ borderColor: "var(--color-border)" }}
              >
                재스캔
              </button>
            </div>

            {/* 탭 */}
            <div className="flex gap-1 border-b" style={{ borderColor: "var(--color-border)" }}>
              <TabButton
                active={tab === "exact"}
                onClick={() => setTab("exact")}
                count={result.exact_groups.length}
              >
                정확 중복
              </TabButton>
              <TabButton
                active={tab === "similar"}
                onClick={() => setTab("similar")}
                count={result.similar_groups.length}
              >
                유사 중복
              </TabButton>
            </div>

            {/* 그룹 목록 */}
            <div className="max-h-[400px] overflow-y-auto space-y-3">
              {groups.length === 0 ? (
                <p
                  className="text-center py-8 text-sm"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {tab === "exact"
                    ? "정확히 같은 파일이 없습니다"
                    : "유사한 내용의 문서가 없습니다"}
                </p>
              ) : (
                groups.map((group, gi) => (
                  <div
                    key={gi}
                    className="border rounded-lg overflow-hidden"
                    style={{ borderColor: "var(--color-border)" }}
                  >
                    {/* 그룹 헤더 */}
                    <div
                      className="flex items-center gap-2 px-3 py-2 text-xs"
                      style={{ backgroundColor: "var(--color-bg-tertiary)" }}
                    >
                      <Badge
                        variant={group.duplicate_type === "exact" ? "danger" : "warning"}
                      >
                        {group.duplicate_type === "exact"
                          ? "동일"
                          : `${(group.similarity * 100).toFixed(0)}% 유사`}
                      </Badge>
                      <span style={{ color: "var(--color-text-secondary)" }}>
                        {group.files.length}개 파일
                      </span>
                      {group.files[0] && (
                        <span
                          className="ml-auto"
                          style={{ color: "var(--color-text-muted)" }}
                        >
                          {formatSize(group.files[0].size)}
                        </span>
                      )}
                    </div>

                    {/* 파일 목록 */}
                    {group.files.map((file, fi) => (
                      <div
                        key={fi}
                        className="flex items-center gap-2 px-3 py-2 text-xs border-t hover:bg-[var(--color-bg-tertiary)] transition-colors"
                        style={{ borderColor: "var(--color-border)" }}
                      >
                        <FileIcon
                          fileName={file.file_name}
                          className="w-4 h-4 flex-shrink-0"
                        />
                        <button
                          className="font-medium truncate text-left hover:underline"
                          style={{ color: "var(--color-text-primary)" }}
                          onClick={() => onOpenFile(file.file_path)}
                          title={file.file_path}
                        >
                          {file.file_name}
                        </button>
                        <span
                          className="truncate flex-1 text-right"
                          style={{ color: "var(--color-text-muted)", maxWidth: "200px" }}
                          title={file.file_path}
                        >
                          {file.file_path
                            .replace(/\\/g, "/")
                            .split("/")
                            .slice(-3, -1)
                            .join("/")}
                        </span>
                        <span style={{ color: "var(--color-text-muted)" }}>
                          {formatDate(file.modified_at)}
                        </span>
                        <button
                          onClick={() => copyPath(file.file_path)}
                          className="p-1 rounded hover:bg-[var(--color-bg-secondary)]"
                          title="경로 복사"
                        >
                          <Copy className="w-3 h-3" />
                        </button>
                        <button
                          onClick={() => onOpenFolder(file.file_path.replace(/[/\\][^/\\]+$/, ""))}
                          className="p-1 rounded hover:bg-[var(--color-bg-secondary)]"
                          title="폴더 열기"
                        >
                          <FileText className="w-3 h-3" />
                        </button>
                      </div>
                    ))}
                  </div>
                ))
              )}
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}

function TabButton({
  active,
  onClick,
  count,
  children,
}: {
  active: boolean;
  onClick: () => void;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className="px-3 py-2 text-xs font-medium border-b-2 transition-colors"
      style={{
        borderColor: active ? "var(--color-accent)" : "transparent",
        color: active ? "var(--color-accent)" : "var(--color-text-muted)",
      }}
    >
      {children} ({count})
    </button>
  );
}
