import { useEffect, useState } from "react";
import { X, Plus, Minus, Edit3 } from "lucide-react";
import { invokeWithTimeout } from "../../utils/invokeWithTimeout";
import type { LineageDiffResponse } from "../../types/search";

interface Props {
  aPath: string;
  aName: string;
  bPath: string;
  bName: string;
  onClose: () => void;
}

/** 두 버전 간 청크 레벨 변경점을 보여주는 모달. */
export function VersionDiffModal({ aPath, aName, bPath, bName, onClose }: Props) {
  const [data, setData] = useState<LineageDiffResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await invokeWithTimeout<LineageDiffResponse>(
          "get_lineage_diff",
          { aPath, bPath },
          120_000,
        );
        if (!cancelled) setData(res);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [aPath, bPath]);

  const counts = data
    ? {
        added: data.changes.filter((c) => c.kind === "added").length,
        removed: data.changes.filter((c) => c.kind === "removed").length,
        modified: data.changes.filter((c) => c.kind === "modified").length,
      }
    : { added: 0, removed: 0, modified: 0 };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ backgroundColor: "rgba(0,0,0,0.5)" }}
      onClick={onClose}
    >
      <div
        className="rounded-lg shadow-2xl overflow-hidden flex flex-col"
        style={{
          backgroundColor: "var(--color-bg-elevated)",
          border: "1px solid var(--color-border)",
          maxWidth: 820,
          width: "100%",
          maxHeight: "85vh",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          className="flex items-start justify-between p-4 border-b"
          style={{ borderColor: "var(--color-border-subtle)" }}
        >
          <div className="flex-1 min-w-0">
            <h3 className="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
              버전 간 변경점
            </h3>
            <div className="mt-1 text-xs space-y-0.5" style={{ color: "var(--color-text-secondary)" }}>
              <div>
                <span className="opacity-75">A:</span> {aName}
              </div>
              <div>
                <span className="opacity-75">B:</span> {bName}
              </div>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded hover:opacity-70"
            aria-label="닫기"
          >
            <X className="w-4 h-4" style={{ color: "var(--color-text-secondary)" }} />
          </button>
        </div>

        {/* Summary */}
        {data && (
          <div
            className="px-4 py-2 text-xs flex items-center gap-4 border-b"
            style={{
              borderColor: "var(--color-border-subtle)",
              backgroundColor: "var(--color-bg-subtle)",
            }}
          >
            <span>
              <span className="font-semibold" style={{ color: "var(--color-text-primary)" }}>
                {data.unchanged_count}
              </span>{" "}
              <span style={{ color: "var(--color-text-muted)" }}>동일</span>
            </span>
            <span style={{ color: "var(--color-accent)" }}>
              <Edit3 className="w-3 h-3 inline mr-0.5" />
              {counts.modified} 변경
            </span>
            <span style={{ color: "var(--color-success, #34a853)" }}>
              <Plus className="w-3 h-3 inline mr-0.5" />
              {counts.added} 추가
            </span>
            <span style={{ color: "var(--color-danger, #ea4335)" }}>
              <Minus className="w-3 h-3 inline mr-0.5" />
              {counts.removed} 제거
            </span>
            <span className="ml-auto" style={{ color: "var(--color-text-muted)" }}>
              A: {data.a_total_chunks} · B: {data.b_total_chunks} 청크
            </span>
          </div>
        )}

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading && (
            <div className="text-sm" style={{ color: "var(--color-text-muted)" }}>
              두 파일의 청크 임베딩을 생성하고 비교 중... (최대 1분)
            </div>
          )}
          {error && (
            <div className="text-sm" style={{ color: "var(--color-danger, #ea4335)" }}>
              오류: {error}
            </div>
          )}
          {data && data.changes.length === 0 && (
            <div className="text-sm" style={{ color: "var(--color-text-muted)" }}>
              의미 있는 변경점이 발견되지 않음 (모든 청크가 95% 이상 동일).
            </div>
          )}
          {data && data.changes.length > 0 && (
            <div className="space-y-2">
              {data.changes.map((c, i) => (
                <DiffRow key={i} entry={c} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function DiffRow({ entry }: { entry: import("../../types/search").ChunkDiffEntry }) {
  const { kind, a_preview, b_preview, similarity, location_hint, page_number } = entry;
  const iconByKind = {
    added: <Plus className="w-3.5 h-3.5" />,
    removed: <Minus className="w-3.5 h-3.5" />,
    modified: <Edit3 className="w-3.5 h-3.5" />,
  };
  const colorByKind = {
    added: "var(--color-success, #34a853)",
    removed: "var(--color-danger, #ea4335)",
    modified: "var(--color-accent)",
  };
  const labelByKind = {
    added: "추가됨",
    removed: "제거됨",
    modified: "수정됨",
  };

  return (
    <div
      className="rounded-md p-2.5 text-xs"
      style={{
        border: "1px solid var(--color-border-subtle)",
        backgroundColor: "var(--color-bg-subtle)",
      }}
    >
      <div className="flex items-center gap-2 mb-1.5">
        <span style={{ color: colorByKind[kind] }}>{iconByKind[kind]}</span>
        <span className="font-semibold" style={{ color: colorByKind[kind] }}>
          {labelByKind[kind]}
        </span>
        {similarity !== null && (
          <span style={{ color: "var(--color-text-muted)" }}>
            유사도 {(similarity * 100).toFixed(0)}%
          </span>
        )}
        {(location_hint || page_number) && (
          <span className="ml-auto" style={{ color: "var(--color-text-muted)" }}>
            {location_hint ?? `p.${page_number}`}
          </span>
        )}
      </div>
      {a_preview && (
        <div className="mb-1 pl-1" style={{ borderLeft: `2px solid ${colorByKind.removed}` }}>
          <div className="text-[10px] font-semibold mb-0.5" style={{ color: colorByKind.removed }}>
            A
          </div>
          <div className="pl-2" style={{ color: "var(--color-text-secondary)" }}>
            {a_preview}
          </div>
        </div>
      )}
      {b_preview && (
        <div className="pl-1" style={{ borderLeft: `2px solid ${colorByKind.added}` }}>
          <div className="text-[10px] font-semibold mb-0.5" style={{ color: colorByKind.added }}>
            B
          </div>
          <div className="pl-2" style={{ color: "var(--color-text-secondary)" }}>
            {b_preview}
          </div>
        </div>
      )}
    </div>
  );
}
