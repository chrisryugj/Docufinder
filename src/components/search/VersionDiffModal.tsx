import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { X, Plus, Minus, Edit3, Check, ArrowRight } from "lucide-react";
import { invokeWithTimeout } from "../../utils/invokeWithTimeout";
import { cleanPath } from "../../utils/cleanPath";
import type { LineageDiffResponse, ChunkDiffEntry } from "../../types/search";

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

  // ESC 닫기
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const modifiedRows = data?.changes.filter((c) => c.kind === "modified") ?? [];
  const addedRows = data?.changes.filter((c) => c.kind === "added") ?? [];
  const removedRows = data?.changes.filter((c) => c.kind === "removed") ?? [];
  const unchangedSamples = data?.changes.filter((c) => c.kind === "unchanged") ?? [];
  const hasRealChanges = modifiedRows.length + addedRows.length + removedRows.length > 0;

  const content = (
    <div
      className="fixed inset-0 flex items-center justify-center p-4"
      style={{ backgroundColor: "rgba(0,0,0,0.55)", zIndex: 1000 }}
      onClick={onClose}
    >
      <div
        className="rounded-lg shadow-2xl overflow-hidden flex flex-col"
        style={{
          backgroundColor: "var(--color-bg-elevated, var(--color-bg-primary))",
          backgroundImage:
            "linear-gradient(var(--color-bg-elevated, var(--color-bg-primary)), var(--color-bg-elevated, var(--color-bg-primary)))",
          border: "1px solid var(--color-border)",
          maxWidth: 880,
          width: "100%",
          maxHeight: "85vh",
          boxShadow: "0 20px 50px rgba(0,0,0,0.25), 0 4px 12px rgba(0,0,0,0.12)",
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
            <div
              className="mt-2 text-xs flex items-center gap-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              <FileLabel role="A" name={aName} path={aPath} />
              <ArrowRight
                className="w-4 h-4 flex-shrink-0"
                style={{ color: "var(--color-text-muted)" }}
              />
              <FileLabel role="B" name={bName} path={bPath} />
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded hover:opacity-70 flex-shrink-0"
            aria-label="닫기"
          >
            <X className="w-4 h-4" style={{ color: "var(--color-text-secondary)" }} />
          </button>
        </div>

        {/* Summary */}
        {data && (
          <div
            className="px-4 py-2.5 text-xs flex items-center gap-3 border-b flex-wrap"
            style={{
              borderColor: "var(--color-border-subtle)",
              backgroundColor: "var(--color-bg-subtle)",
            }}
          >
            <SummaryChip
              icon={<Edit3 className="w-3 h-3" />}
              count={modifiedRows.length}
              label="변경"
              color="var(--color-accent)"
            />
            <SummaryChip
              icon={<Plus className="w-3 h-3" />}
              count={addedRows.length}
              label="B에 추가"
              color="var(--color-success, #34a853)"
            />
            <SummaryChip
              icon={<Minus className="w-3 h-3" />}
              count={removedRows.length}
              label="B에서 제거"
              color="var(--color-danger, #ea4335)"
            />
            <SummaryChip
              icon={<Check className="w-3 h-3" />}
              count={data.unchanged_count}
              label="동일"
              color="var(--color-text-muted)"
            />
            <span className="ml-auto" style={{ color: "var(--color-text-muted)" }}>
              A {data.a_total_chunks}청크 · B {data.b_total_chunks}청크
            </span>
          </div>
        )}

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
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
          {data && !hasRealChanges && unchangedSamples.length === 0 && !loading && (
            <div className="text-sm" style={{ color: "var(--color-text-muted)" }}>
              비교 가능한 청크가 없습니다 (내용이 너무 짧거나 임베딩 실패).
            </div>
          )}

          {modifiedRows.length > 0 && (
            <Section title={`변경된 내용 (${modifiedRows.length})`} color="var(--color-accent)">
              {modifiedRows.map((c, i) => (
                <ModifiedRow key={`m-${i}`} entry={c} />
              ))}
            </Section>
          )}

          {addedRows.length > 0 && (
            <Section
              title={`B에 추가된 내용 (${addedRows.length})`}
              subtitle="원본(A)에 없는 내용"
              color="var(--color-success, #34a853)"
            >
              {addedRows.map((c, i) => (
                <OneSideRow key={`a-${i}`} entry={c} side="B" />
              ))}
            </Section>
          )}

          {removedRows.length > 0 && (
            <Section
              title={`B에서 제거된 내용 (${removedRows.length})`}
              subtitle="대상(B)에 없는 내용"
              color="var(--color-danger, #ea4335)"
            >
              {removedRows.map((c, i) => (
                <OneSideRow key={`r-${i}`} entry={c} side="A" />
              ))}
            </Section>
          )}

          {!hasRealChanges && unchangedSamples.length > 0 && (
            <Section
              title="모든 청크가 거의 동일"
              subtitle="95% 이상 유사 — 아래는 비교된 청크 샘플"
              color="var(--color-text-muted)"
            >
              {unchangedSamples.map((c, i) => (
                <UnchangedRow key={`u-${i}`} entry={c} />
              ))}
            </Section>
          )}

          {hasRealChanges && unchangedSamples.length > 0 && (
            <details>
              <summary
                className="cursor-pointer text-xs py-1.5 px-2 rounded inline-block"
                style={{
                  color: "var(--color-text-muted)",
                  backgroundColor: "var(--color-bg-subtle)",
                }}
              >
                동일 청크 {unchangedSamples.length}개 보기
              </summary>
              <div className="mt-2 space-y-2">
                {unchangedSamples.map((c, i) => (
                  <UnchangedRow key={`u-${i}`} entry={c} />
                ))}
              </div>
            </details>
          )}
        </div>
      </div>
    </div>
  );

  return createPortal(content, document.body);
}

function FileLabel({ role, name, path }: { role: "A" | "B"; name: string; path: string }) {
  const color = role === "A" ? "var(--color-danger, #ea4335)" : "var(--color-success, #34a853)";
  return (
    <div className="flex-1 min-w-0 flex items-center gap-1.5" title={path}>
      <span
        className="inline-block px-1.5 py-0.5 rounded text-[10px] font-bold flex-shrink-0"
        style={{ backgroundColor: "var(--color-bg-subtle)", color }}
      >
        {role}
      </span>
      <span className="truncate">
        <span className="font-medium">{name}</span>
        <span className="ml-1 opacity-60 text-[10px]">{cleanPath(path)}</span>
      </span>
    </div>
  );
}

function SummaryChip({
  icon,
  count,
  label,
  color,
}: {
  icon: React.ReactNode;
  count: number;
  label: string;
  color: string;
}) {
  return (
    <span className="inline-flex items-center gap-1" style={{ color }}>
      {icon}
      <span className="font-semibold">{count}</span>
      <span className="opacity-80">{label}</span>
    </span>
  );
}

function Section({
  title,
  subtitle,
  color,
  children,
}: {
  title: string;
  subtitle?: string;
  color: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div
        className="text-xs font-semibold mb-2 pb-1 border-b flex items-baseline gap-2"
        style={{ color, borderColor: `color-mix(in srgb, ${color} 30%, transparent)` }}
      >
        <span>{title}</span>
        {subtitle && (
          <span className="text-[10px] font-normal opacity-70">· {subtitle}</span>
        )}
      </div>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

function ChunkMeta({ entry }: { entry: ChunkDiffEntry }) {
  const { similarity, location_hint, page_number } = entry;
  const parts: string[] = [];
  if (location_hint) parts.push(location_hint);
  else if (page_number) parts.push(`p.${page_number}`);
  if (similarity !== null && similarity !== undefined) {
    parts.push(`유사도 ${(similarity * 100).toFixed(1)}%`);
  }
  if (parts.length === 0) return null;
  return (
    <div className="text-[10px] mb-1" style={{ color: "var(--color-text-muted)" }}>
      {parts.join(" · ")}
    </div>
  );
}

/** 양쪽 모두 존재 — 수정됨 */
function ModifiedRow({ entry }: { entry: ChunkDiffEntry }) {
  return (
    <div
      className="rounded-md p-2.5 text-xs"
      style={{
        border: "1px solid var(--color-border-subtle)",
        backgroundColor: "var(--color-bg-subtle)",
      }}
    >
      <ChunkMeta entry={entry} />
      <div className="grid grid-cols-2 gap-2">
        {entry.a_preview && (
          <div
            className="rounded p-2"
            style={{
              backgroundColor: "color-mix(in srgb, var(--color-danger, #ea4335) 8%, transparent)",
              borderLeft: "2px solid var(--color-danger, #ea4335)",
            }}
          >
            <div
              className="text-[10px] font-semibold mb-1"
              style={{ color: "var(--color-danger, #ea4335)" }}
            >
              A (원본)
            </div>
            <div style={{ color: "var(--color-text-secondary)" }}>{entry.a_preview}</div>
          </div>
        )}
        {entry.b_preview && (
          <div
            className="rounded p-2"
            style={{
              backgroundColor: "color-mix(in srgb, var(--color-success, #34a853) 8%, transparent)",
              borderLeft: "2px solid var(--color-success, #34a853)",
            }}
          >
            <div
              className="text-[10px] font-semibold mb-1"
              style={{ color: "var(--color-success, #34a853)" }}
            >
              B (대상)
            </div>
            <div style={{ color: "var(--color-text-secondary)" }}>{entry.b_preview}</div>
          </div>
        )}
      </div>
    </div>
  );
}

/** 한쪽에만 존재 — 추가/제거됨 */
function OneSideRow({ entry, side }: { entry: ChunkDiffEntry; side: "A" | "B" }) {
  const preview = side === "A" ? entry.a_preview : entry.b_preview;
  const color = side === "A" ? "var(--color-danger, #ea4335)" : "var(--color-success, #34a853)";
  if (!preview) return null;
  return (
    <div
      className="rounded-md p-2.5 text-xs"
      style={{
        border: "1px solid var(--color-border-subtle)",
        backgroundColor: `color-mix(in srgb, ${color} 5%, transparent)`,
        borderLeft: `3px solid ${color}`,
      }}
    >
      <ChunkMeta entry={entry} />
      <div style={{ color: "var(--color-text-secondary)" }}>{preview}</div>
    </div>
  );
}

/** 동일 — 한 번만 표시 */
function UnchangedRow({ entry }: { entry: ChunkDiffEntry }) {
  const { a_preview, b_preview, byte_identical } = entry;
  const preview = a_preview ?? b_preview;
  if (!preview) return null;
  return (
    <div
      className="rounded-md p-2.5 text-xs"
      style={{
        border: "1px dashed var(--color-border-subtle)",
        backgroundColor: "var(--color-bg-subtle)",
      }}
    >
      <div
        className="text-[10px] mb-1 flex items-center gap-1"
        style={{ color: "var(--color-text-muted)" }}
      >
        <Check className="w-3 h-3" />
        <span>{byte_identical ? "완전 동일" : "거의 동일"}</span>
        {entry.similarity !== null && entry.similarity !== undefined && (
          <span>· {(entry.similarity * 100).toFixed(1)}%</span>
        )}
      </div>
      <div style={{ color: "var(--color-text-secondary)" }}>{preview}</div>
    </div>
  );
}
