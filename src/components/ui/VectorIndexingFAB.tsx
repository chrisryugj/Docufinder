import { useState } from "react";
import { X, Brain } from "lucide-react";

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
 * - 원형 진행률 표시
 * - 호버 시 상세 정보
 * - 취소 버튼
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
  const radius = 20;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = circumference - (progress / 100) * circumference;

  // 현재 파일명 (경로에서 파일명만 추출)
  const fileName = currentFile
    ? currentFile.split(/[\\/]/).pop() || currentFile
    : null;

  return (
    <div
      className="fixed bottom-6 right-6 z-50"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      {/* 호버 시 상세 정보 */}
      {isHovered && (
        <div className="absolute bottom-full right-0 mb-2 w-64 p-3 bg-popover border border-border rounded-lg shadow-lg">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium">시맨틱 검색 준비 중</span>
            <button
              onClick={onCancel}
              className="p-1 hover:bg-muted rounded transition-colors"
              title="취소"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
          <div className="text-xs text-muted-foreground space-y-1">
            <div>
              진행률: {processedChunks.toLocaleString()} / {totalChunks.toLocaleString()} 청크
            </div>
            {fileName && (
              <div className="truncate" title={currentFile || undefined}>
                현재: {fileName}
              </div>
            )}
          </div>
        </div>
      )}

      {/* FAB 버튼 */}
      <div className="relative w-14 h-14 cursor-pointer">
        {/* 배경 원 */}
        <div className="absolute inset-0 rounded-full bg-primary/10 backdrop-blur-sm border border-primary/20" />

        {/* SVG 진행률 원 */}
        <svg className="absolute inset-0 w-full h-full -rotate-90">
          {/* 배경 트랙 */}
          <circle
            cx="28"
            cy="28"
            r={radius}
            fill="none"
            stroke="currentColor"
            strokeWidth="3"
            className="text-primary/20"
          />
          {/* 진행률 */}
          <circle
            cx="28"
            cy="28"
            r={radius}
            fill="none"
            stroke="currentColor"
            strokeWidth="3"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={strokeDashoffset}
            className="text-primary transition-all duration-300"
          />
        </svg>

        {/* 중앙 아이콘 + 퍼센트 */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <Brain className="w-4 h-4 text-primary animate-pulse" />
          <span className="text-[10px] font-medium text-primary">{progress}%</span>
        </div>
      </div>
    </div>
  );
}
