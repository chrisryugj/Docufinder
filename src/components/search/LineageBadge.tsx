import { useState, useEffect, useRef, useCallback } from "react";
import { createPortal } from "react-dom";
import { FileText, Crown, Clock, GitCompare } from "lucide-react";
import { invokeWithTimeout } from "../../utils/invokeWithTimeout";
import { formatRelativeTime } from "../../utils/formatRelativeTime";
import type { LineageVersion } from "../../types/search";
import { VersionDiffModal } from "./VersionDiffModal";

interface LineageBadgeProps {
  lineageId: string;
  versionCount: number;
  versionLabel?: string;
  currentFilePath: string;
  onOpenFile: (path: string) => void;
}

/** 검색 결과 카드에 표시되는 버전 뱃지.
 * 클릭 시 드롭다운으로 같은 lineage의 모든 버전 목록 표시. */
export function LineageBadge({
  lineageId,
  versionCount,
  versionLabel,
  currentFilePath,
  onOpenFile,
}: LineageBadgeProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [versions, setVersions] = useState<LineageVersion[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number }>({ top: 0, left: 0 });
  const [diffTarget, setDiffTarget] = useState<LineageVersion | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const currentVersion = versions?.find((v) => v.file_path === currentFilePath);

  const fetchVersions = useCallback(async () => {
    if (versions !== null || loading) return;
    setLoading(true);
    try {
      const result = await invokeWithTimeout<LineageVersion[]>(
        "get_lineage_versions",
        { lineageId },
        10_000,
      );
      setVersions(result);
    } catch (e) {
      console.error("lineage versions fetch failed:", e);
      setVersions([]);
    } finally {
      setLoading(false);
    }
  }, [lineageId, versions, loading]);

  const toggleOpen = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!isOpen && triggerRef.current) {
        const rect = triggerRef.current.getBoundingClientRect();
        setPos({ top: rect.bottom + 4, left: rect.left });
        fetchVersions();
      }
      setIsOpen((v) => !v);
    },
    [isOpen, fetchVersions],
  );

  // 외부 클릭 시 닫기
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        triggerRef.current &&
        !triggerRef.current.contains(target) &&
        popoverRef.current &&
        !popoverRef.current.contains(target)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [isOpen]);

  if (versionCount < 2) return null;

  return (
    <>
      <button
        ref={triggerRef}
        onClick={toggleOpen}
        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-medium transition-colors hover:opacity-80"
        style={{
          backgroundColor: "var(--color-bg-hover)",
          color: "var(--color-text-secondary)",
          border: "1px solid var(--color-border-subtle)",
        }}
        title={`같은 문서의 다른 버전 ${versionCount - 1}개 더 있음`}
        aria-label={`버전 ${versionCount}개 보기`}
      >
        <FileText className="w-3 h-3" strokeWidth={2} />
        {versionLabel && <span>{versionLabel}</span>}
        <span className="opacity-75">· 버전 {versionCount}개</span>
      </button>

      {isOpen &&
        createPortal(
          <div
            ref={popoverRef}
            className="fixed z-50 rounded-lg shadow-lg overflow-hidden"
            style={{
              top: pos.top,
              left: pos.left,
              minWidth: 320,
              maxWidth: 480,
              maxHeight: 360,
              backgroundColor: "var(--color-bg-elevated)",
              border: "1px solid var(--color-border)",
            }}
          >
            <div
              className="px-3 py-2 text-xs font-semibold border-b"
              style={{
                backgroundColor: "var(--color-bg-subtle)",
                color: "var(--color-text-secondary)",
                borderColor: "var(--color-border-subtle)",
              }}
            >
              문서 버전 ({versionCount}개)
            </div>
            <div className="overflow-y-auto" style={{ maxHeight: 320 }}>
              {loading && (
                <div className="p-3 text-xs" style={{ color: "var(--color-text-muted)" }}>
                  불러오는 중...
                </div>
              )}
              {!loading && versions && versions.length === 0 && (
                <div className="p-3 text-xs" style={{ color: "var(--color-text-muted)" }}>
                  버전 정보 없음
                </div>
              )}
              {!loading &&
                versions &&
                versions.map((v) => {
                  const isCurrent = v.file_path === currentFilePath;
                  const isCanonical = v.lineage_role === "canonical";
                  return (
                    <div
                      key={v.file_path}
                      className="flex items-stretch gap-0 transition-colors"
                      style={{
                        backgroundColor: isCurrent ? "var(--color-bg-hover)" : "transparent",
                        borderBottom: "1px solid var(--color-border-subtle)",
                      }}
                    >
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onOpenFile(v.file_path);
                          setIsOpen(false);
                        }}
                        className="flex-1 text-left px-3 py-2 flex items-start gap-2 hover:brightness-110"
                      >
                      {isCanonical ? (
                        <Crown
                          className="w-3.5 h-3.5 mt-0.5 flex-shrink-0"
                          style={{ color: "var(--color-accent-warm)" }}
                        />
                      ) : (
                        <Clock
                          className="w-3.5 h-3.5 mt-0.5 flex-shrink-0"
                          style={{ color: "var(--color-text-muted)" }}
                        />
                      )}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-1.5">
                          {v.version_label && (
                            <span
                              className="text-[10px] px-1 rounded font-medium"
                              style={{
                                backgroundColor: isCanonical
                                  ? "var(--color-accent-warm-bg)"
                                  : "var(--color-bg-subtle)",
                                color: isCanonical
                                  ? "var(--color-accent-warm)"
                                  : "var(--color-text-secondary)",
                              }}
                            >
                              {v.version_label}
                            </span>
                          )}
                          <span
                            className="text-xs font-medium truncate"
                            style={{ color: "var(--color-text-primary)" }}
                          >
                            {v.file_name}
                          </span>
                        </div>
                        {v.modified_at && (
                          <div
                            className="text-[10px] mt-0.5"
                            style={{ color: "var(--color-text-muted)" }}
                          >
                            {formatRelativeTime(v.modified_at)}
                          </div>
                        )}
                      </div>
                      </button>
                      {!isCurrent && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setDiffTarget(v);
                            setIsOpen(false);
                          }}
                          className="px-2 flex items-center hover:brightness-110"
                          style={{
                            borderLeft: "1px solid var(--color-border-subtle)",
                            color: "var(--color-text-muted)",
                          }}
                          title="현재 파일과 비교"
                          aria-label="현재 파일과 변경점 비교"
                        >
                          <GitCompare className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>
                  );
                })}
            </div>
          </div>,
          document.body,
        )}

      {diffTarget && (
        <VersionDiffModal
          aPath={currentFilePath}
          aName={currentVersion?.file_name ?? currentFilePath.split(/[\\/]/).pop() ?? "현재"}
          bPath={diffTarget.file_path}
          bName={diffTarget.file_name}
          onClose={() => setDiffTarget(null)}
        />
      )}
    </>
  );
}
