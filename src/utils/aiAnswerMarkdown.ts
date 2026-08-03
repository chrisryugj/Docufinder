import { cleanPath } from "./cleanPath";
import type { AiAnalysis } from "../types/search";

/** 경로에서 파일명만 추출 */
function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
}

/** AI 답변을 나중에 다시 찾아볼 수 있는 마크다운 문서로 조립.
 *  본문의 [출처N] 표기는 그대로 두고, 아래 참조 문서 번호와 대응시킨다. */
export function buildAiAnswerMarkdown(
  question: string | undefined,
  answer: string,
  analysis: AiAnalysis | null,
): string {
  const lines: string[] = [`# ${question?.trim() || "AI 문서 분석 결과"}`, ""];

  const meta = [new Date().toLocaleString("ko-KR")];
  if (analysis?.model) meta.push(analysis.model);
  lines.push(`> Anything AI 문서 분석 · ${meta.join(" · ")}`, "", answer.trim(), "");

  const files = analysis?.source_files ?? [];
  if (files.length > 0) {
    lines.push("## 참조 문서", "");
    files.forEach((path, i) => {
      const hint = analysis?.sources?.[i]?.location_hint;
      lines.push(`${i + 1}. **${basename(path)}**${hint ? ` — ${hint}` : ""}`);
      lines.push(`   \`${cleanPath(path)}\``);
    });
    lines.push("");
  }

  return lines.join("\n");
}

/** 질문을 파일명으로 쓸 수 있게 정리 (경로 구분자·예약문자 제거) */
export function toSafeFileStem(question: string | undefined): string {
  const stem = (question ?? "").replace(/[<>:"/\\|?*\n\r\t]+/g, "_").trim();
  return stem.slice(0, 40) || "AI_문서분석";
}

/** 검색 범위 폴더를 저장 다이얼로그 기본 위치로 삼는다.
 *  범위 지정 없이(전체 검색) 질의했으면 파일명만 넘겨 OS 기본 위치를 쓴다. */
export function toDefaultSavePath(
  scope: string | null | undefined,
  fileName: string,
): string {
  const dir = scope ? cleanPath(scope).replace(/[/\\]+$/, "") : "";
  if (!dir) return fileName;
  return `${dir}${dir.includes("\\") ? "\\" : "/"}${fileName}`;
}
