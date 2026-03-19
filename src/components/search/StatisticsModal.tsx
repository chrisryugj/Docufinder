import { useEffect, useState, useCallback, useMemo, memo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Modal } from "../ui/Modal";
import type { DocumentStatistics, StatEntry, FileEntry } from "../../types/search";

interface StatisticsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onFilterByType?: (fileType: string) => void;
  onOpenFile?: (filePath: string, pageNumber?: number | null) => void;
}

/** 파일 크기 포맷 */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/** 타임스탬프 → 날짜 문자열 */
function formatDate(ts: number): string {
  const d = new Date(ts * 1000);
  return d.toLocaleDateString("ko-KR", { year: "numeric", month: "2-digit", day: "2-digit" });
}

/** 파일 유형 → 한글 레이블 */
const TYPE_LABELS: Record<string, string> = {
  txt: "텍스트", md: "마크다운", hwpx: "한글", hwp: "한글(구)",
  docx: "워드", doc: "워드(구)", pptx: "파워포인트", ppt: "파워포인트(구)",
  xlsx: "엑셀", xls: "엑셀(구)", pdf: "PDF",
};

/** 파일 유형별 차트 색상 */
const TYPE_COLORS: Record<string, string> = {
  hwpx: "#3b82f6", hwp: "#60a5fa",
  docx: "#8b5cf6", doc: "#a78bfa",
  pptx: "#f97316", ppt: "#fb923c",
  xlsx: "#22c55e", xls: "#4ade80",
  pdf: "#ef4444",
  txt: "#6b7280", md: "#9ca3af",
};
const DEFAULT_COLOR = "#a1a1aa";

/** 도넛 차트 (SVG) */
function DonutChart({ data, onSegmentClick }: { data: StatEntry[]; onSegmentClick?: (label: string) => void }) {
  const total = data.reduce((sum, d) => sum + d.count, 0);
  if (total === 0) return <p className="text-center text-sm" style={{ color: "var(--color-text-muted)" }}>데이터 없음</p>;

  const cx = 80, cy = 80, r = 60, strokeWidth = 24;
  const circumference = 2 * Math.PI * r;

  // 세그먼트 오프셋 사전 계산 (StrictMode double-render 안전)
  const segments = useMemo(() => {
    let acc = 0;
    return data.map((entry) => {
      const pct = entry.count / total;
      const dashLen = pct * circumference;
      const seg = { label: entry.label, dashLen, dashOffset: -acc, color: TYPE_COLORS[entry.label] || DEFAULT_COLOR };
      acc += dashLen;
      return seg;
    });
  }, [data, total, circumference]);

  return (
    <div className="flex items-center gap-4">
      <svg width="160" height="160" viewBox="0 0 160 160" className="shrink-0">
        {segments.map((seg) => (
            <circle
              key={seg.label}
              cx={cx} cy={cy} r={r}
              fill="none"
              stroke={seg.color}
              strokeWidth={strokeWidth}
              strokeDasharray={`${seg.dashLen} ${circumference - seg.dashLen}`}
              strokeDashoffset={seg.dashOffset}
              transform={`rotate(-90 ${cx} ${cy})`}
              className="cursor-pointer hover:opacity-80 transition-opacity"
              onClick={() => onSegmentClick?.(seg.label)}
            />
        ))}
        <text x={cx} y={cy - 6} textAnchor="middle" fill="var(--color-text-primary)" fontSize="18" fontWeight="bold">
          {total.toLocaleString()}
        </text>
        <text x={cx} y={cy + 12} textAnchor="middle" fill="var(--color-text-muted)" fontSize="10">
          총 문서
        </text>
      </svg>
      <div className="flex flex-col gap-1 min-w-0">
        {data.slice(0, 8).map((entry) => (
          <button
            key={entry.label}
            className="flex items-center gap-2 text-xs hover:opacity-80 text-left"
            onClick={() => onSegmentClick?.(entry.label)}
          >
            <span
              className="w-2.5 h-2.5 rounded-sm shrink-0"
              style={{ backgroundColor: TYPE_COLORS[entry.label] || DEFAULT_COLOR }}
            />
            <span className="truncate" style={{ color: "var(--color-text-secondary)" }}>
              {TYPE_LABELS[entry.label] || entry.label}
            </span>
            <span className="tabular-nums ml-auto font-medium" style={{ color: "var(--color-text-primary)" }}>
              {entry.count.toLocaleString()}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

/** 수평 바 차트 (SVG) */
function BarChart({ data, maxBars = 8 }: { data: StatEntry[]; maxBars?: number }) {
  const sliced = data.slice(0, maxBars);
  const max = Math.max(...sliced.map((d) => d.count), 1);
  if (sliced.length === 0) return <p className="text-center text-sm" style={{ color: "var(--color-text-muted)" }}>데이터 없음</p>;

  return (
    <div className="flex flex-col gap-1.5">
      {sliced.map((entry) => {
        const pct = (entry.count / max) * 100;
        return (
          <div key={entry.label} className="flex items-center gap-2">
            <span className="text-[11px] w-10 text-right tabular-nums shrink-0" style={{ color: "var(--color-text-secondary)" }}>
              {entry.label}
            </span>
            <div className="flex-1 h-4 rounded-sm overflow-hidden" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
              <div
                className="h-full rounded-sm transition-all"
                style={{ width: `${pct}%`, backgroundColor: "var(--color-accent)" }}
              />
            </div>
            <span className="text-[11px] w-8 tabular-nums shrink-0" style={{ color: "var(--color-text-muted)" }}>
              {entry.count.toLocaleString()}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** 파일 리스트 */
function FileList({
  files,
  type,
  onOpenFile,
}: {
  files: FileEntry[];
  type: "recent" | "largest";
  onOpenFile?: (path: string) => void;
}) {
  if (files.length === 0) return null;
  return (
    <div className="flex flex-col gap-0.5">
      {files.map((f, i) => (
        <button
          key={f.path}
          className="flex items-center gap-2 px-2 py-1 rounded text-left hover:bg-[var(--color-bg-tertiary)] transition-colors"
          onClick={() => onOpenFile?.(f.path)}
        >
          <span className="text-[10px] w-4 text-right tabular-nums" style={{ color: "var(--color-text-muted)" }}>
            {i + 1}
          </span>
          <span className="text-xs truncate flex-1" style={{ color: "var(--color-text-primary)" }}>
            {f.name}
          </span>
          <span className="text-[10px] tabular-nums shrink-0" style={{ color: "var(--color-text-muted)" }}>
            {type === "recent" ? formatDate(f.value) : formatSize(f.value)}
          </span>
        </button>
      ))}
    </div>
  );
}

/** 폴더별 분포 */
function FolderList({ folders }: { folders: StatEntry[] }) {
  if (folders.length === 0) return null;
  const max = Math.max(...folders.map((f) => f.count), 1);
  return (
    <div className="flex flex-col gap-1.5">
      {folders.map((f) => {
        const pct = (f.count / max) * 100;
        const folderName = f.label.split(/[/\\]/).filter(Boolean).pop() || f.label;
        return (
          <div key={f.label} className="flex items-center gap-2" title={f.label}>
            <span className="text-[11px] truncate w-24 shrink-0" style={{ color: "var(--color-text-secondary)" }}>
              {folderName}
            </span>
            <div className="flex-1 h-3.5 rounded-sm overflow-hidden" style={{ backgroundColor: "var(--color-bg-tertiary)" }}>
              <div className="h-full rounded-sm" style={{ width: `${pct}%`, backgroundColor: "var(--color-accent-secondary, var(--color-accent))" }} />
            </div>
            <span className="text-[10px] w-8 tabular-nums shrink-0" style={{ color: "var(--color-text-muted)" }}>
              {f.count.toLocaleString()}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** 섹션 래퍼 */
function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-2">
      <h3 className="text-xs font-semibold uppercase tracking-wider" style={{ color: "var(--color-text-muted)" }}>
        {title}
      </h3>
      {children}
    </div>
  );
}

export const StatisticsModal = memo(function StatisticsModal({
  isOpen,
  onClose,
  onFilterByType,
  onOpenFile,
}: StatisticsModalProps) {
  const [stats, setStats] = useState<DocumentStatistics | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    setLoading(true);
    invoke<DocumentStatistics>("get_document_statistics")
      .then(setStats)
      .catch(() => setStats(null))
      .finally(() => setLoading(false));
  }, [isOpen]);

  const handleSegmentClick = useCallback(
    (fileType: string) => {
      onFilterByType?.(fileType);
      onClose();
    },
    [onFilterByType, onClose]
  );

  const handleOpenFile = useCallback(
    (path: string) => {
      onOpenFile?.(path, null);
    },
    [onOpenFile]
  );

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="문서 통계" size="lg">
      {loading ? (
        <div className="flex items-center justify-center py-12">
          <div className="w-6 h-6 rounded-full animate-spin" style={{ border: "2px solid var(--color-border)", borderTopColor: "var(--color-accent)" }} />
        </div>
      ) : stats ? (
        <div className="space-y-6">
          {/* 요약 카드 */}
          <div className="grid grid-cols-3 gap-3">
            {[
              { label: "총 문서", value: stats.total_files.toLocaleString() },
              { label: "인덱싱 완료", value: stats.indexed_files.toLocaleString() },
              { label: "총 크기", value: formatSize(stats.total_size) },
            ].map((card) => (
              <div
                key={card.label}
                className="text-center px-3 py-2.5 rounded-lg"
                style={{ backgroundColor: "var(--color-bg-tertiary)" }}
              >
                <div className="text-lg font-bold tabular-nums" style={{ color: "var(--color-text-primary)" }}>
                  {card.value}
                </div>
                <div className="text-[10px] mt-0.5" style={{ color: "var(--color-text-muted)" }}>
                  {card.label}
                </div>
              </div>
            ))}
          </div>

          {/* 파일 유형 분포 (도넛) */}
          <Section title="파일 유형별 분포">
            <DonutChart data={stats.file_types} onSegmentClick={handleSegmentClick} />
          </Section>

          {/* 연도별 분포 (바) */}
          <Section title="연도별 문서 수">
            <BarChart data={stats.years} />
          </Section>

          {/* 폴더별 분포 */}
          {stats.folders.length > 0 && (
            <Section title="폴더별 문서 수">
              <FolderList folders={stats.folders} />
            </Section>
          )}

          {/* 최근/최대 파일 */}
          <div className="grid grid-cols-2 gap-4">
            <Section title="최근 수정된 문서">
              <FileList files={stats.recent_files} type="recent" onOpenFile={handleOpenFile} />
            </Section>
            <Section title="가장 큰 문서">
              <FileList files={stats.largest_files} type="largest" onOpenFile={handleOpenFile} />
            </Section>
          </div>
        </div>
      ) : (
        <p className="text-center py-8 text-sm" style={{ color: "var(--color-text-muted)" }}>
          통계를 불러올 수 없습니다.
        </p>
      )}
    </Modal>
  );
});
