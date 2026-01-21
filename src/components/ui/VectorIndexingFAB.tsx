import { useState } from "react";
import { X, Sparkles } from "lucide-react";

interface VectorIndexingFABProps {
  /** 진행률 (0-100) */
  progress: number;
  /** 전체 청크 수 */
  totalChunks: number;
  /** 처리된 청크 수 */
  processedChunks: number;
  /** 현재 파일명 */
  currentFile: string | null;
  /** 취소 콜백 */
  onCancel: () => void;
}

/**
 * 벡터 인덱싱 진행률 FAB (Floating Action Button)
 * - 글래스모피즘 디자인
 * - 원형 진행률 + 글로우 효과
 * - 호버 시 상세 정보
 */
export function VectorIndexingFAB({
  progress,
  totalChunks,
  processedChunks,
  currentFile,
  onCancel,
}: VectorIndexingFABProps) {
  const [isHovered, setIsHovered] = useState(false);

  // SVG 원형 진행률 계산
  const radius = 22;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = circumference - (progress / 100) * circumference;

  // 현재 파일명 (경로에서 파일명만 추출)
  const fileName = currentFile
    ? currentFile.split(/[\\/]/).pop() || currentFile
    : null;

  return (
    <div
      className="fixed bottom-20 right-5 z-50"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* 호버 시 상세 정보 패널 */}
      <div
        className={`
          absolute bottom-full right-0 mb-3 w-56
          transition-all duration-200 ease-out origin-bottom-right
          ${isHovered
            ? "opacity-100 scale-100 translate-y-0"
            : "opacity-0 scale-95 translate-y-2 pointer-events-none"
          }
        `}
      >
        <div
          className="p-3 rounded-xl border shadow-xl backdrop-blur-xl"
          style={{
            background: "linear-gradient(135deg, rgba(var(--color-primary-rgb, 99, 102, 241), 0.08), rgba(var(--color-primary-rgb, 99, 102, 241), 0.02))",
            borderColor: "rgba(var(--color-primary-rgb, 99, 102, 241), 0.2)",
          }}
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
              시맨틱 인덱싱
            </span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onCancel();
              }}
              className="p-1.5 rounded-lg transition-all hover:bg-red-500/20 hover:text-red-400 group"
              title="취소"
            >
              <X className="w-3.5 h-3.5 transition-transform group-hover:rotate-90" />
            </button>
          </div>

          <div className="space-y-1.5">
            <div className="flex justify-between text-xs" style={{ color: "var(--color-text-secondary)" }}>
              <span>진행률</span>
              <span className="font-mono">{processedChunks.toLocaleString()} / {totalChunks.toLocaleString()}</span>
            </div>

            {/* 미니 진행률 바 */}
            <div
              className="h-1 rounded-full overflow-hidden"
              style={{ background: "rgba(var(--color-primary-rgb, 99, 102, 241), 0.15)" }}
            >
              <div
                className="h-full rounded-full transition-all duration-500 ease-out"
                style={{
                  width: `${progress}%`,
                  background: "linear-gradient(90deg, var(--color-primary), var(--color-primary-light, #818cf8))",
                }}
              />
            </div>

            {fileName && (
              <div
                className="text-xs truncate pt-1"
                style={{ color: "var(--color-text-muted)" }}
                title={currentFile || undefined}
              >
                {fileName}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* FAB 버튼 */}
      <div
        className={`
          relative w-14 h-14 cursor-pointer
          transition-transform duration-200 ease-out
          ${isHovered ? "scale-110" : "scale-100"}
        `}
      >
        {/* 글로우 효과 */}
        <div
          className="absolute -inset-1 rounded-full blur-lg opacity-40 animate-pulse"
          style={{
            background: "linear-gradient(135deg, var(--color-primary), var(--color-primary-light, #818cf8))"
          }}
        />

        {/* 메인 배경 */}
        <div
          className="absolute inset-0 rounded-full backdrop-blur-xl border shadow-lg"
          style={{
            background: "linear-gradient(135deg, rgba(var(--color-primary-rgb, 99, 102, 241), 0.15), rgba(255, 255, 255, 0.05))",
            borderColor: "rgba(var(--color-primary-rgb, 99, 102, 241), 0.3)",
          }}
        />

        {/* SVG 진행률 원 */}
        <svg className="absolute inset-0 w-full h-full -rotate-90">
          {/* 배경 트랙 */}
          <circle
            cx="28"
            cy="28"
            r={radius}
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            className="text-primary/15"
          />
          {/* 진행률 */}
          <circle
            cx="28"
            cy="28"
            r={radius}
            fill="none"
            stroke="url(#progressGradient)"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={strokeDashoffset}
            className="transition-all duration-500 ease-out"
            style={{
              filter: "drop-shadow(0 0 4px var(--color-primary))",
            }}
          />
          {/* 그라데이션 정의 */}
          <defs>
            <linearGradient id="progressGradient" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stopColor="var(--color-primary)" />
              <stop offset="100%" stopColor="var(--color-primary-light, #818cf8)" />
            </linearGradient>
          </defs>
        </svg>

        {/* 중앙 콘텐츠 */}
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-0.5">
          <Sparkles
            className="w-4 h-4 animate-pulse"
            style={{ color: "var(--color-primary)" }}
          />
          <span
            className="text-[10px] font-semibold tabular-nums"
            style={{ color: "var(--color-primary)" }}
          >
            {progress}%
          </span>
        </div>
      </div>
    </div>
  );
}
