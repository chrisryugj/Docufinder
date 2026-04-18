import { useState } from "react";
import { InfoTooltip } from "../../ui/Tooltip";
import { SettingsToggle } from "../SettingsToggle";
import { invokeWithTimeout } from "../../../utils/invokeWithTimeout";
import type { TabProps } from "./types";
import { CONFIDENCE_STEP } from "./types";

interface RebuildLineageResponse {
  files_updated: number;
  lineages_created: number;
  vector_split: number;
  reunited: number;
  elapsed_ms: number;
}

interface LineageHealthReport {
  total_lineages: number;
  multi_version_lineages: number;
  problem_lineages: Array<{
    canonical_name: string;
    file_count: number;
    status: string;
    issues: string[];
  }>;
  unassigned_files: number;
}

interface PruneResult {
  total_checked: number;
  pruned: number;
  elapsed_ms: number;
}

export function SearchTab({ settings, onChange }: TabProps) {
  const [rebuilding, setRebuilding] = useState(false);
  const [rebuildResult, setRebuildResult] = useState<string | null>(null);
  const [pruning, setPruning] = useState(false);

  async function handlePruneMissing() {
    if (pruning) return;
    setPruning(true);
    setRebuildResult(null);
    try {
      const res = await invokeWithTimeout<PruneResult>(
        "prune_missing_files",
        undefined,
        300_000,
      );
      setRebuildResult(
        res.pruned === 0
          ? `✅ 정리할 레코드 없음 (${res.total_checked.toLocaleString()}개 검사 · ${(res.elapsed_ms / 1000).toFixed(1)}s)`
          : `🧹 ${res.pruned.toLocaleString()}개 고아 레코드 삭제 · 전체 ${res.total_checked.toLocaleString()}개 · ${(res.elapsed_ms / 1000).toFixed(1)}s`,
      );
    } catch (e) {
      setRebuildResult(`정리 실패: ${e}`);
    } finally {
      setPruning(false);
    }
  }

  async function handleRebuildLineage() {
    if (rebuilding) return;
    setRebuilding(true);
    setRebuildResult(null);
    try {
      const res = await invokeWithTimeout<RebuildLineageResponse>(
        "rebuild_lineage",
        undefined,
        600_000,
      );
      const parts = [
        `${res.files_updated.toLocaleString()}개 파일`,
        `${res.lineages_created.toLocaleString()}개 그룹`,
      ];
      if (res.vector_split > 0) parts.push(`벡터 분리 ${res.vector_split}`);
      if (res.reunited > 0) parts.push(`폴더 병합 ${res.reunited}`);
      parts.push(`${(res.elapsed_ms / 1000).toFixed(1)}s`);
      setRebuildResult(parts.join(" · "));
    } catch (e) {
      setRebuildResult(`실패: ${e}`);
    } finally {
      setRebuilding(false);
    }
  }

  async function handleCheckHealth() {
    try {
      const res = await invokeWithTimeout<LineageHealthReport>(
        "get_lineage_health",
        { limit: 10 },
        30_000,
      );
      const summary = [
        `전체 ${res.total_lineages.toLocaleString()}개 문서`,
        `다중버전 ${res.multi_version_lineages}개`,
        `미분류 ${res.unassigned_files}개`,
      ].join(" · ");
      if (res.problem_lineages.length === 0) {
        setRebuildResult(`✅ ${summary} — 정리 필요 lineage 없음`);
      } else {
        const top = res.problem_lineages
          .slice(0, 3)
          .map((p) => `• ${p.canonical_name || "(미지정)"} (${p.file_count}개, ${p.issues.join(", ")})`)
          .join("\n");
        setRebuildResult(`⚠️ ${summary}\n정리 대상 ${res.problem_lineages.length}개:\n${top}`);
      }
    } catch (e) {
      setRebuildResult(`건강도 조회 실패: ${e}`);
    }
  }

  return (
    <div className="space-y-3">
      {/* 최소 신뢰도 */}
      <div>
        <label
          className="flex items-center text-sm font-medium mb-1"
          style={{ color: "var(--color-text-secondary)" }}
        >
          최소 신뢰도
          <InfoTooltip
            content={
              <div className="space-y-2 py-1">
                <div>
                  <strong style={{ color: "var(--color-text-primary)" }}>점수 산정</strong>
                  <p className="mt-0.5">RRF 방식으로 키워드·의미 검색 순위를 병합 계산</p>
                </div>
                <div>
                  <strong style={{ color: "var(--color-text-primary)" }}>추천</strong>
                  <ul className="mt-0.5 space-y-0.5">
                    <li>0%: 모든 결과 / 20-30%: 권장 / 50%+: 정확한 결과만</li>
                  </ul>
                </div>
              </div>
            }
            maxWidth={280}
          />
        </label>
        <div className="flex items-center gap-3">
          <input
            type="range"
            min={0}
            max={100}
            step={CONFIDENCE_STEP}
            value={settings.min_confidence}
            onChange={(e) => onChange("min_confidence", Number(e.target.value))}
            className="flex-1 accent-[var(--color-accent)]"
            aria-label="최소 신뢰도 설정"
          />
          <div
            className="min-w-[40px] text-sm font-semibold text-right"
            style={{ color: "var(--color-text-primary)" }}
          >
            {settings.min_confidence}%
          </div>
        </div>
      </div>

      {/* 하위폴더 포함 */}
      <SettingsToggle
        label="하위폴더 포함"
        description="폴더 추가 시 하위폴더도 함께 인덱싱"
        checked={settings.include_subfolders ?? true}
        onChange={(v) => onChange("include_subfolders", v)}
      />

      {/* 제외 디렉토리 */}
      <div>
        <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
          제외 디렉토리
          <span className="font-normal ml-1" style={{ color: "var(--color-text-muted)" }}>
            (줄바꿈 구분, 기본: Windows·Program Files·AppData 등)
          </span>
        </label>
        <textarea
          className="w-full rounded-md px-3 py-1.5 text-xs font-mono resize-y"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
            minHeight: "48px",
          }}
          value={(settings.exclude_dirs ?? []).join("\n")}
          onChange={(e) =>
            onChange(
              "exclude_dirs",
              e.target.value
                .split("\n")
                .map((s) => s.trim())
                .filter((s): s is string => Boolean(s))
            )
          }
          placeholder="추가 제외할 폴더명 입력..."
          rows={2}
        />
      </div>

      {/* OCR 설정 */}
      <div>
        <SettingsToggle
          label="OCR (이미지 텍스트 인식)"
          description="JPG, PNG, BMP, TIFF 이미지에서 텍스트 추출 (PaddleOCR, ~15MB 모델)"
          checked={settings.ocr_enabled ?? false}
          onChange={(v) => onChange("ocr_enabled", v)}
        />
      </div>

      {/* 문서 버전 그룹핑 (Document Lineage) */}
      <div>
        <SettingsToggle
          label="문서 버전 그룹핑"
          description="같은 문서의 여러 버전(최종/최최종/v2)을 검색 결과에서 대표 1개로 표시. 펼치면 모든 버전 확인 가능."
          checked={settings.group_versions ?? true}
          onChange={(v) => onChange("group_versions", v)}
        />
        <div className="mt-2 flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleRebuildLineage}
              disabled={rebuilding}
              className="px-2.5 py-1 rounded text-xs font-medium transition-opacity hover:opacity-80 disabled:opacity-50"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                color: "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              {rebuilding ? "재계산 중..." : "버전 그룹 재계산"}
            </button>
            <button
              type="button"
              onClick={handleCheckHealth}
              disabled={rebuilding}
              className="px-2.5 py-1 rounded text-xs font-medium transition-opacity hover:opacity-80 disabled:opacity-50"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                color: "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              건강도 확인
            </button>
            <button
              type="button"
              onClick={handlePruneMissing}
              disabled={rebuilding || pruning}
              title="디스크에 없는 파일의 DB 잔재 레코드를 삭제합니다"
              className="px-2.5 py-1 rounded text-xs font-medium transition-opacity hover:opacity-80 disabled:opacity-50"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                color: "var(--color-text-secondary)",
                border: "1px solid var(--color-border)",
              }}
            >
              {pruning ? "정리 중..." : "없는 파일 정리"}
            </button>
          </div>
          {rebuildResult && (
            <pre
              className="text-[11px] whitespace-pre-wrap font-sans"
              style={{ color: "var(--color-text-muted)" }}
            >
              {rebuildResult}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}
