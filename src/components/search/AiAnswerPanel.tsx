import { memo, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import "katex/dist/katex.min.css";
import type { AiAnalysis } from "../../types/search";
import { FileIcon } from "../ui/FileIcon";
import { ResultContextMenu, useContextMenu } from "./ResultContextMenu";

interface Props {
  answer: string;
  isStreaming: boolean;
  analysis: AiAnalysis | null;
  error: string | null;
  onReset: () => void;
  currentQuestion?: string;
  onExampleClick?: (text: string) => void;
}

const EXAMPLE_CATEGORIES: { label: string; icon: string; examples: string[] }[] = [
  { label: "요약", icon: "📋", examples: ["인사규정 핵심 조항을 요약해줘", "회의록에서 결정된 사항만 정리해줘", "보고서의 주요 결론은 뭐야?"] },
  { label: "조건·규정", icon: "📜", examples: ["연차 사용 조건이 어떻게 되나요?", "계약 해지 시 위약금 조항은?", "재택근무 신청 자격 요건은?"] },
  { label: "수치·데이터", icon: "📊", examples: ["2026년 예산 총액은 얼마인가요?", "프로젝트별 투입 인원 현황은?", "매출 목표 달성률이 어떻게 돼?"] },
  { label: "일정·기한", icon: "📅", examples: ["계약 만료일이 언제야?", "분기별 제출 마감일 정리해줘", "2026년 주요 일정을 알려줘"] },
  { label: "절차·방법", icon: "📝", examples: ["출장비 정산 절차가 어떻게 돼?", "신규 입사자 등록 방법은?", "장비 반납 프로세스 알려줘"] },
  { label: "내용 확인", icon: "🔍", examples: ["보안 교육 이수 기준이 뭐야?", "납품 검수 기준을 알려줘", "개인정보 처리방침 내용은?"] },
];

function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
}

/** Windows extended-length path prefix (\\?\) 제거 */
function cleanPath(path: string): string {
  return path.replace(/^\\\\\?\\/, "");
}

/** 답변 끝의 [출처: 1, 3] 패턴에서 문서 번호(0-based) 추출 + 답변 텍스트 정리 */
function parseSourceRefs(text: string): { cleanText: string; refIndices: Set<number> } {
  const match = text.match(/\[출처:\s*([\d,\s]+)\]\s*$/);
  if (!match) return { cleanText: text, refIndices: new Set() };

  const indices = match[1]
    .split(",")
    .map((s) => parseInt(s.trim(), 10) - 1) // 1-based → 0-based
    .filter((n) => !isNaN(n) && n >= 0);

  return {
    cleanText: text.slice(0, match.index).trimEnd(),
    refIndices: new Set(indices),
  };
}

function AiAnswerPanel({ answer, isStreaming, analysis, error, onReset, currentQuestion }: Props) {
  const handleOpenFile = useCallback((path: string) => {
    invoke("open_file", { path }).catch(() => {});
  }, []);

  const handleOpenFolder = useCallback((path: string) => {
    invoke("open_folder", { path }).catch(() => {});
  }, []);

  // 에러 상태
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-12 px-6 text-center">
        <div className="w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center mb-3">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--color-error, #ef4444)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="15" y1="9" x2="9" y2="15" />
            <line x1="9" y1="9" x2="15" y2="15" />
          </svg>
        </div>
        {currentQuestion && (
          <p className="text-xs text-[var(--color-text-tertiary)] mb-2">
            질문: <span className="italic">"{currentQuestion}"</span>
          </p>
        )}
        <p className="text-sm text-[var(--color-text-secondary)] mb-3 max-w-md">{error}</p>
        <button onClick={onReset} className="text-xs text-[var(--color-accent)] hover:underline">
          초기화
        </button>
      </div>
    );
  }

  // 대기 상태 (아직 질문 안 함)
  if (!answer && !isStreaming && !analysis) {
    return (
      <div className="flex flex-col h-full px-4 sm:px-8 pt-2">
        {/* 검색 범위: 검색창 ScopeChip으로 이동 */}

        {/* 예시 질문 그리드 — 표시 전용 */}
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
          {EXAMPLE_CATEGORIES.map((cat) => (
            <div
              key={cat.label}
              className="flex flex-col gap-2.5 px-4 py-3.5 rounded-xl"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              <div className="flex items-center gap-2">
                <span className="text-base">{cat.icon}</span>
                <span className="text-xs font-semibold text-[var(--color-text-secondary)]">
                  {cat.label}
                </span>
              </div>
              <ul className="space-y-1.5">
                {cat.examples.map((ex) => (
                  <li key={ex} className="text-[13px] leading-snug text-[var(--color-text-muted)]">
                    "{ex}"
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {/* 하단 안내 */}
        <div className="mt-auto pb-4 flex items-center justify-center gap-4 text-xs text-[var(--color-text-tertiary)]">
          <span>Enter로 전송 · Shift+Enter로 줄바꿈</span>
          <span className="opacity-30">|</span>
          <span className="flex items-center gap-1.5 opacity-70">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--color-warning, #f59e0b)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
              <path d="M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            </svg>
            문서 내용이 외부 AI 서버로 전송됩니다
          </span>
        </div>
      </div>
    );
  }

  const { cleanText, refIndices } = !isStreaming ? parseSourceRefs(answer) : { cleanText: answer, refIndices: new Set<number>() };

  return (
    <div className="flex flex-col h-full overflow-y-auto px-2 py-2 gap-3">
      {/* 질문 영역 */}
      {currentQuestion && (
        <div
          className="flex items-start gap-2.5 px-3.5 py-3 rounded-lg"
          style={{ backgroundColor: "var(--color-bg-tertiary)" }}
        >
          <div
            className="w-5 h-5 rounded-full shrink-0 flex items-center justify-center mt-0.5"
            style={{ backgroundColor: "var(--color-text-muted)", opacity: 0.2 }}
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="var(--color-text-secondary)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
          </div>
          <p className="text-sm text-[var(--color-text-primary)] leading-relaxed flex-1" title={currentQuestion}>
            {currentQuestion}
          </p>
          <button
            onClick={onReset}
            className="shrink-0 p-1 rounded hover:bg-[var(--color-bg-secondary)] transition-colors"
            style={{ color: "var(--color-text-muted)" }}
            title="초기화"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      )}

      {/* 답변 영역 */}
      <div
        className="flex-1 rounded-lg px-3.5 py-3"
        style={{
          backgroundColor: "var(--color-bg-secondary)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* 답변 라벨 */}
        <div className="flex items-center gap-2 mb-2.5">
          <div
            className="w-5 h-5 rounded-full shrink-0 flex items-center justify-center"
            style={{ background: "linear-gradient(135deg, var(--color-accent-ai) 0%, var(--color-accent-ai-hover) 100%)" }}
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="white" stroke="none">
              <path d="M12 2l2.4 6.4L21 11l-6.6 2.4L12 21l-2.4-7.6L3 11l6.6-2.4L12 2z" />
            </svg>
          </div>
          <span className="text-[11px] font-medium" style={{ color: "var(--color-accent-ai)" }}>
            AI 문서 분석 결과
          </span>
          {isStreaming && (
            <span className="text-[10px] animate-pulse" style={{ color: "var(--color-accent-ai)" }}>분석 중...</span>
          )}
          {analysis && (
            <span className="text-[10px] text-[var(--color-text-tertiary)] ml-auto tabular-nums">
              {(analysis.processing_time_ms / 1000).toFixed(1)}초
            </span>
          )}
        </div>

        {/* 답변 본문 (마크다운 렌더링) */}
        <div className="text-sm text-[var(--color-text-primary)] leading-[1.8] break-words ai-answer-prose">
          {isStreaming ? (
            // 스트리밍 중: pre-wrap (마크다운 미완성 상태)
            <span className="whitespace-pre-wrap" aria-live="polite" role="status">
              {cleanText}
              <span
                className="inline-block w-1.5 h-3.5 rounded-sm animate-pulse ml-0.5 align-text-bottom"
                style={{ backgroundColor: "var(--color-accent-ai)" }}
              />
            </span>
          ) : (
            // 완료: 마크다운 렌더링 ($...$ 수식은 KaTeX 로)
            <ReactMarkdown
              remarkPlugins={[remarkGfm, remarkMath]}
              rehypePlugins={[rehypeKatex]}
            >
              {cleanText}
            </ReactMarkdown>
          )}
        </div>
      </div>

      {/* 참조 문서 */}
      {analysis && analysis.source_files.length > 0 && (
        <div className="space-y-1.5">
          <p className="text-[10px] font-medium text-[var(--color-text-tertiary)] px-1">
            참조 문서 {refIndices.size > 0 && <span className="font-normal">· 근거 {refIndices.size}건</span>}
          </p>
          {(() => {
            const hasRefs = refIndices.size > 0;
            const sorted = analysis.source_files
              .map((path, i) => ({ path, i, isRef: refIndices.has(i) }))
              .sort((a, b) => (a.isRef === b.isRef ? 0 : a.isRef ? -1 : 1));

            return (
              <div className="flex flex-col gap-0.5">
                {sorted.map(({ path, isRef }) => (
                  <SourceFileItem
                    key={path}
                    path={path}
                    isRef={isRef}
                    dimmed={hasRefs && !isRef}
                    onOpenFile={handleOpenFile}
                    onOpenFolder={handleOpenFolder}
                  />
                ))}
              </div>
            );
          })()}
        </div>
      )}

      {/* 하단 액션 바 */}
      {(analysis || error) && (
        <CopyableActionBar answer={answer} analysis={analysis} onReset={onReset} />

      )}
    </div>
  );
}

/** 참조 문서 아이템 — 우클릭 컨텍스트 메뉴 지원 */
function SourceFileItem({
  path,
  isRef,
  dimmed,
  onOpenFile,
  onOpenFolder,
}: {
  path: string;
  isRef: boolean;
  dimmed: boolean;
  onOpenFile: (path: string) => void;
  onOpenFolder: (path: string) => void;
}) {
  const name = basename(path);
  const { contextMenu, handleContextMenu, closeContextMenu } = useContextMenu();

  const handleCopyPath = useCallback((p: string) => {
    navigator.clipboard.writeText(cleanPath(p));
  }, []);

  const folderPath = cleanPath(path).replace(/\\/g, "/").split("/").slice(0, -1).join("/");

  return (
    <>
      <div
        data-context-menu
        className={`flex items-center gap-2.5 px-3 py-2 rounded-lg group cursor-pointer transition-all ${
          dimmed ? "opacity-40" : ""
        }`}
        style={{
          backgroundColor: isRef ? "var(--color-bg-secondary)" : "transparent",
          border: isRef ? "1px solid var(--color-border)" : "1px solid transparent",
        }}
        onClick={() => onOpenFile(path)}
        onContextMenu={handleContextMenu}
        title={cleanPath(path)}
      >
        <FileIcon fileName={name} size="sm" />
        <span className="text-[13px] text-[var(--color-text-secondary)] truncate flex-1">
          {name}
        </span>
        {isRef && (
          <span
            className="text-[9px] font-medium px-1.5 py-0.5 rounded shrink-0"
            style={{ backgroundColor: "var(--color-accent-ai-subtle)", color: "var(--color-accent-ai)" }}
          >
            근거
          </span>
        )}
        <button
          onClick={(e) => { e.stopPropagation(); onOpenFolder(path); }}
          className="text-[var(--color-text-tertiary)] hover:text-[var(--color-accent)] opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
          title="폴더 열기"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
        </button>
      </div>
      <ResultContextMenu
        filePath={cleanPath(path)}
        folderPath={folderPath}
        onOpenFile={() => onOpenFile(path)}
        onCopyPath={handleCopyPath}
        onOpenFolder={() => onOpenFolder(path)}
        contextMenu={contextMenu}
        closeContextMenu={closeContextMenu}
      />
    </>
  );
}

/** 하단 액션 바 — 새 질문 + 복사 버튼 */
function CopyableActionBar({ answer, analysis, onReset }: { answer: string; analysis: AiAnalysis | null; onReset: () => void }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(answer);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard API 미지원 시 fallback
      const textarea = document.createElement("textarea");
      textarea.value = answer;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [answer]);

  return (
    <div className="flex items-center justify-between pt-1">
      <div className="flex items-center gap-1">
        <button
          onClick={onReset}
          className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded-md transition-colors hover:bg-[var(--color-bg-tertiary)]"
          style={{ color: "var(--color-text-muted)" }}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="1 4 1 10 7 10" />
            <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
          </svg>
          새 질문
        </button>
        {answer && (
          <button
            onClick={handleCopy}
            className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium rounded-md transition-colors hover:bg-[var(--color-bg-tertiary)]"
            style={{ color: copied ? "var(--color-success)" : "var(--color-text-muted)" }}
            aria-label="AI 답변 복사"
          >
            {copied ? (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            ) : (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
              </svg>
            )}
            {copied ? "복사됨" : "복사"}
          </button>
        )}
      </div>
      {analysis && (
        <span className="text-[10px] text-[var(--color-text-tertiary)] tabular-nums">
          {analysis.model}
          {analysis.tokens_used && ` · ${analysis.tokens_used.total_tokens}t`}
        </span>
      )}
    </div>
  );
}

export default memo(AiAnswerPanel);
