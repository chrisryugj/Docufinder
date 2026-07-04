import { useState, useEffect, useCallback } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { InfoTooltip } from "../../ui/Tooltip";
import { Button } from "../../ui/Button";
import { SettingsToggle } from "../SettingsToggle";
import { invokeWithTimeout } from "../../../utils/invokeWithTimeout";
import type { TabProps } from "./types";
import { CONFIDENCE_STEP } from "./types";
import { SYSTEM_FOLDERS_HINT } from "../../../utils/platform";

interface FormulaModelInfo {
  name: string;
  filename: string;
  sizeMb: number;
  exists: boolean;
  verified: boolean;
  path: string;
  invalidReason?: string | null;
}

interface FormulaModelsStatus {
  modelsDir: string;
  allReady: boolean;
  models: FormulaModelInfo[];
}

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

interface OcrReindexCandidates {
  count: number;
  folders: string[];
}

export function SearchTab({ settings, onChange }: TabProps) {
  const [rebuilding, setRebuilding] = useState(false);
  const [rebuildResult, setRebuildResult] = useState<string | null>(null);
  const [pruning, setPruning] = useState(false);

  // ────────── OCR 재인덱싱 프롬프트 ──────────
  // OCR 를 새로 켜면 이미 인덱싱된 이미지·스캔 PDF 는 재인덱싱해야 인식된다.
  const [ocrReindex, setOcrReindex] = useState<OcrReindexCandidates | null>(null);
  const [ocrReindexing, setOcrReindexing] = useState(false);
  const [ocrReindexMsg, setOcrReindexMsg] = useState<string | null>(null);

  const checkOcrCandidates = useCallback(async () => {
    try {
      const res = await invoke<OcrReindexCandidates>("count_ocr_reindex_candidates");
      setOcrReindexMsg(null);
      setOcrReindex(res.count > 0 ? res : null);
    } catch {
      // 조회 실패 시 프롬프트 없이 조용히 넘어간다(핵심 흐름 아님).
      setOcrReindex(null);
    }
  }, []);

  const handleOcrReindex = useCallback(async () => {
    if (!ocrReindex || ocrReindexing) return;
    setOcrReindexing(true);
    try {
      // 후보 폴더만 순차 재인덱싱(동시 실행은 감시 pause/resume 충돌 → 순차).
      for (const path of ocrReindex.folders) {
        await invoke("reindex_folder", { path });
      }
      setOcrReindexMsg(`✅ ${ocrReindex.folders.length}개 폴더 재인덱싱 완료 — 스캔 PDF·이미지가 OCR로 인식됩니다.`);
      setOcrReindex(null);
    } catch (e) {
      setOcrReindexMsg(`재인덱싱 실패: ${e}`);
    } finally {
      setOcrReindexing(false);
    }
  }, [ocrReindex, ocrReindexing]);

  // ────────── 수식 OCR 모델 상태 ──────────
  const [formulaStatus, setFormulaStatus] = useState<FormulaModelsStatus | null>(null);
  const [formulaChecking, setFormulaChecking] = useState(false);
  const [formulaDownloading, setFormulaDownloading] = useState(false);
  const [formulaProgress, setFormulaProgress] = useState<string | null>(null);
  const [formulaError, setFormulaError] = useState<string | null>(null);

  const checkFormulaStatus = useCallback(async () => {
    setFormulaChecking(true);
    setFormulaError(null);
    try {
      const s = await invoke<FormulaModelsStatus>("get_formula_models_status");
      setFormulaStatus(s);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setFormulaError(msg);
    } finally {
      setFormulaChecking(false);
    }
  }, []);

  useEffect(() => {
    if (settings.formula_ocr_enabled && !formulaStatus && !formulaChecking) {
      void checkFormulaStatus();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.formula_ocr_enabled]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    listen<string>("formula-model-progress", (ev) => {
      setFormulaProgress(ev.payload);
    })
      .then((fn) => {
        // 등록 완료 전 unmount(설정 탭 전환)면 cleanup이 no-op — 즉시 해제해 고아 방지
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const handleDownloadFormula = useCallback(async () => {
    setFormulaDownloading(true);
    setFormulaError(null);
    setFormulaProgress("다운로드 시작…");
    try {
      await invoke("download_formula_models");
      await checkFormulaStatus();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setFormulaError(msg);
    } finally {
      setFormulaDownloading(false);
    }
  }, [checkFormulaStatus]);

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
            (줄바꿈 구분, 기본: {SYSTEM_FOLDERS_HINT})
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
          description="이미지 파일(JPG, PNG, BMP, TIFF)과 스캔 PDF(이미지 기반)에서 텍스트 추출 (PaddleOCR, ~15MB 모델)"
          checked={settings.ocr_enabled ?? false}
          onChange={(v) => {
            onChange("ocr_enabled", v);
            if (v) void checkOcrCandidates();
            else {
              setOcrReindex(null);
              setOcrReindexMsg(null);
            }
          }}
        />
        {/* OCR 를 새로 켰을 때: 이미 인덱싱된 이미지·PDF 는 재인덱싱해야 인식됨 → 후보 폴더만 재인덱싱 프롬프트 */}
        {ocrReindex && (
          <div
            className="mt-2 p-2.5 rounded-md text-xs leading-relaxed"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              border: "1px solid var(--color-border)",
              color: "var(--color-text-secondary)",
            }}
          >
            <p>
              이미 인덱싱된{" "}
              <strong style={{ color: "var(--color-text-primary)" }}>
                이미지·PDF {ocrReindex.count.toLocaleString()}개
              </strong>
              는 재인덱싱해야 OCR로 인식돼요. (폴더 {ocrReindex.folders.length}개 · 폴더 전체 재처리)
            </p>
            <div className="mt-2 flex items-center gap-2">
              <Button
                variant="primary"
                size="sm"
                onClick={handleOcrReindex}
                isLoading={ocrReindexing}
                disabled={ocrReindexing}
              >
                지금 재인덱싱
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setOcrReindex(null)}
                disabled={ocrReindexing}
              >
                나중에
              </Button>
            </div>
          </div>
        )}
        {ocrReindexMsg && (
          <p className="mt-2 text-xs" style={{ color: "var(--color-text-muted)" }}>
            {ocrReindexMsg}
          </p>
        )}
        {/* 레이아웃 분석 — OCR 켰을 때만 노출되는 하위 실험 옵션 */}
        {(settings.ocr_enabled ?? false) && (
          <div className="mt-2 pl-3 border-l-2" style={{ borderColor: "var(--color-border)" }}>
            <SettingsToggle
              label="레이아웃 분석 (실험적)"
              description="스캔 PDF·이미지에서 제목·표·머리글/바닥글을 구분해 본문 노이즈(머리글·바닥글·페이지번호)를 제거 · PP-DocLayout ~22MB 모델 · 재시작 후 적용"
              checked={settings.ocr_layout_enabled ?? false}
              onChange={(v) => onChange("ocr_layout_enabled", v)}
            />
          </div>
        )}
      </div>

      {/* PDF 수식 OCR */}
      <div className="border-t pt-3" style={{ borderColor: "var(--color-border)" }}>
        <SettingsToggle
          label="PDF 수식 OCR"
          description="PDF 페이지에서 수식(Pix2Text ONNX)을 LaTeX로 자동 추출 → 검색/미리보기에 KaTeX 렌더 · 모델 ~155MB (최초 1회 자동 다운로드)"
          checked={settings.formula_ocr_enabled ?? false}
          onChange={(v) => {
            onChange("formula_ocr_enabled", v);
            if (v) void checkFormulaStatus();
          }}
        />
        {/* ⚠️ 강조된 경고 배너 */}
        <div
          className="mt-2 p-2.5 rounded-md text-xs leading-relaxed"
          style={{
            backgroundColor: "rgba(239, 68, 68, 0.08)",
            border: "1px solid var(--color-error, #ef4444)",
            color: "var(--color-error, #ef4444)",
          }}
        >
          <strong>⚠️ 주의:</strong> PDF 인덱싱 속도가 <strong>수 배 ~ 수십 배</strong> 느려집니다.
          페이지마다 수식 영역 검출 + ONNX 추론이 필요해 CPU 부하가 큽니다.
          대량 PDF 폴더라면 비활성화를 권장합니다.
        </div>
        {settings.formula_ocr_enabled && (
          <div
            className="mt-2 p-3 rounded text-xs"
            style={{
              backgroundColor: "var(--color-bg-secondary)",
              color: "var(--color-text-secondary)",
            }}
          >
            <div className="flex items-center justify-between mb-2">
              <span className="font-medium" style={{ color: "var(--color-text-primary)" }}>
                모델 상태
              </span>
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={checkFormulaStatus}
                  isLoading={formulaChecking}
                  disabled={formulaChecking || formulaDownloading}
                >
                  확인
                </Button>
                {formulaStatus && !formulaStatus.allReady && (
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={handleDownloadFormula}
                    isLoading={formulaDownloading}
                    disabled={formulaDownloading}
                  >
                    다운로드
                  </Button>
                )}
              </div>
            </div>
            {formulaError && (
              <p className="mb-2" style={{ color: "var(--color-error)" }}>오류: {formulaError}</p>
            )}
            {formulaProgress && formulaDownloading && (
              <p className="mb-2" style={{ color: "var(--color-text-muted)" }}>{formulaProgress}</p>
            )}
            {formulaStatus ? (
              <ul className="space-y-1">
                {formulaStatus.models.map((m) => (
                  <li key={m.filename} className="flex items-center justify-between">
                    <span>{m.name} ({m.sizeMb}MB)</span>
                    <span
                      style={{
                        color: m.verified
                          ? "var(--color-success)"
                          : m.exists
                            ? "var(--color-warning)"
                            : "var(--color-text-muted)",
                      }}
                    >
                      {m.verified ? "✓ 준비됨" : m.exists ? "⚠ SHA 불일치" : "— 없음"}
                    </span>
                  </li>
                ))}
                <li className="pt-1 mt-1 border-t" style={{ borderColor: "var(--color-border)", color: "var(--color-text-muted)" }}>
                  저장 위치: {formulaStatus.modelsDir}
                </li>
              </ul>
            ) : (
              <p style={{ color: "var(--color-text-muted)" }}>
                상태 확인 필요 — "확인" 버튼을 눌러주세요.
              </p>
            )}
          </div>
        )}
      </div>

      {/* 문서 버전 그룹핑 (Document Lineage) */}
      <div>
        <SettingsToggle
          label="문서 버전 그룹핑"
          description="같은 문서의 여러 버전(최종/최최종/v2)을 검색 결과에서 대표 1개로 표시. 펼치면 모든 버전 확인 가능."
          checked={settings.group_versions ?? true}
          onChange={(v) => onChange("group_versions", v)}
        />
        {/* 유지보수 도구 — 평소엔 접어 두고 필요 시에만 노출 (progressive disclosure) */}
        <details className="mt-2 group">
          <summary
            className="cursor-pointer text-xs select-none transition-colors hover:text-[var(--color-text-secondary)] list-none flex items-center gap-1"
            style={{ color: "var(--color-text-muted)" }}
          >
            <svg
              width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor"
              strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"
              className="transition-transform duration-150 group-open:rotate-90"
            >
              <polyline points="9 18 15 12 9 6" />
            </svg>
            고급 유지보수 도구
          </summary>
          <div className="mt-2 flex flex-col gap-2 pl-1">
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
        </details>
      </div>
    </div>
  );
}
