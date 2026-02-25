import { useState, useEffect, useRef } from "react";
import { invokeWithTimeout, IPC_TIMEOUT } from "../../utils/invokeWithTimeout";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";
import { InfoTooltip } from "../ui/Tooltip";
import { SettingsToggle } from "./SettingsToggle";
import { ColorPresetPicker } from "./ColorPresetPicker";
import type { Settings } from "../../types/settings";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onThemeChange?: (theme: Settings["theme"]) => void;
  onSettingsSaved?: (settings: Settings) => void;
  onClearData?: () => Promise<void>;
}

const SEARCH_MODE_OPTIONS = [
  { value: "keyword", label: "키워드 검색 (권장)" },
  { value: "hybrid", label: "하이브리드 (모델 필요)" },
  { value: "semantic", label: "의미 검색 (모델 필요)" },
  { value: "filename", label: "파일명 검색" },
];

const THEME_OPTIONS = [
  { value: "light", label: "라이트 모드" },
  { value: "dark", label: "다크 모드" },
  { value: "system", label: "시스템 설정" },
];

const MAX_RESULTS_OPTIONS = [
  { value: "20", label: "20개" },
  { value: "50", label: "50개 (기본)" },
  { value: "100", label: "100개" },
  { value: "200", label: "200개" },
  { value: "500", label: "500개" },
  { value: "1000", label: "1000개" },
];

const VIEW_DENSITY_OPTIONS = [
  { value: "normal", label: "기본 (넓게)" },
  { value: "compact", label: "컴팩트 (좁게)" },
];

const VECTOR_INDEXING_MODE_OPTIONS = [
  { value: "manual", label: "수동" },
  { value: "auto", label: "자동" },
];

const INDEXING_INTENSITY_OPTIONS = [
  { value: "fast", label: "빠르게 (CPU 최대)" },
  { value: "balanced", label: "균형 (권장)" },
  { value: "background", label: "백그라운드 (최소 부하)" },
];

const RESULTS_PER_PAGE_OPTIONS = [
  { value: "20", label: "20개" },
  { value: "50", label: "50개 (기본)" },
  { value: "100", label: "100개" },
  { value: "200", label: "200개" },
];

const MAX_FILE_SIZE_OPTIONS = [
  { value: "50", label: "50 MB" },
  { value: "100", label: "100 MB" },
  { value: "200", label: "200 MB (기본)" },
  { value: "500", label: "500 MB" },
  { value: "0", label: "제한 없음" },
];

const CONFIDENCE_STEP = 5;

// 하이라이트 색상 프리셋 (라이트/다크 모드 각각)
const HIGHLIGHT_COLOR_PRESETS = [
  { value: "", label: "기본", light: "#fde047", dark: "#854d0e" },
  { value: "#fbbf24", label: "앰버", light: "#fbbf24", dark: "#b45309" },
  { value: "#fb923c", label: "오렌지", light: "#fb923c", dark: "#c2410c" },
  { value: "#f87171", label: "레드", light: "#f87171", dark: "#b91c1c" },
  { value: "#c084fc", label: "퍼플", light: "#c084fc", dark: "#7c3aed" },
  { value: "#60a5fa", label: "블루", light: "#60a5fa", dark: "#1d4ed8" },
  { value: "#34d399", label: "그린", light: "#34d399", dark: "#059669" },
  { value: "#2dd4bf", label: "틸", light: "#2dd4bf", dark: "#0d9488" },
];

const SEMANTIC_UNLOCK_PASSWORD = "9812";

export function SettingsModal({ isOpen, onClose, onThemeChange, onSettingsSaved, onClearData }: SettingsModalProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const originalDataRootRef = useRef<string | undefined>(undefined);

  // 시맨틱 검색 비밀번호 모달
  const [showSemanticPassword, setShowSemanticPassword] = useState(false);
  const [semanticPasswordInput, setSemanticPasswordInput] = useState("");
  const [semanticPasswordError, setSemanticPasswordError] = useState<string | null>(null);
  const semanticPasswordInputRef = useRef<HTMLInputElement>(null);

  // 설정 로드 (useEffect 내부에 함수 정의하여 의존성 문제 해결)
  useEffect(() => {
    if (!isOpen) return;

    const loadSettings = async () => {
      setIsLoading(true);
      setError(null);
      try {
        const result = await invokeWithTimeout<Settings>("get_settings", undefined, IPC_TIMEOUT.SETTINGS);
        originalDataRootRef.current = result.data_root;
        setSettings(result);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(`설정을 불러올 수 없습니다: ${message}`);
      } finally {
        setIsLoading(false);
      }
    };

    loadSettings();
  }, [isOpen]);

  const saveSettings = async () => {
    if (!settings) return;

    setIsSaving(true);
    setError(null);
    try {
      await invokeWithTimeout("update_settings", { settings }, IPC_TIMEOUT.SETTINGS);
      onSettingsSaved?.(settings);

      // data_root 변경 시 재시작 안내
      if (settings.data_root !== originalDataRootRef.current) {
        await ask(
          "데이터 저장 경로가 변경되었습니다.\n변경 사항을 적용하려면 앱을 재시작해주세요.",
          { title: "재시작 필요", kind: "info", okLabel: "확인" }
        );
      }

      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(`설정 저장에 실패했습니다: ${message}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleChange = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    if (!settings) return;

    // 하이브리드/시맨틱 모드 선택 시 시맨틱 검색이 비활성화 상태면 비밀번호 요구
    if (
      key === "search_mode" &&
      (value === "hybrid" || value === "semantic") &&
      !(settings.semantic_search_enabled ?? false)
    ) {
      setShowSemanticPassword(true);
      setSemanticPasswordInput("");
      setSemanticPasswordError(null);
      // 비밀번호 확인 후 search_mode도 같이 변경하기 위해 pendingSearchMode로 처리
      setPendingSemanticAction(() => () => {
        setSettings((prev) => prev ? { ...prev, [key]: value, semantic_search_enabled: true } : prev);
      });
      return;
    }

    setSettings({ ...settings, [key]: value });

    // 테마 변경 시 즉시 적용
    if (key === "theme" && onThemeChange) {
      onThemeChange(value as Settings["theme"]);
    }
  };

  const [pendingSemanticAction, setPendingSemanticAction] = useState<(() => void) | null>(null);

  const handleSemanticToggle = (enabled: boolean) => {
    if (!settings) return;
    if (enabled) {
      setShowSemanticPassword(true);
      setSemanticPasswordInput("");
      setSemanticPasswordError(null);
      setPendingSemanticAction(() => () => {
        setSettings((prev) => prev ? { ...prev, semantic_search_enabled: true } : prev);
      });
    } else {
      setSettings({ ...settings, semantic_search_enabled: false });
    }
  };

  const handleSemanticPasswordConfirm = () => {
    if (semanticPasswordInput === SEMANTIC_UNLOCK_PASSWORD) {
      pendingSemanticAction?.();
      setShowSemanticPassword(false);
      setSemanticPasswordInput("");
      setSemanticPasswordError(null);
      setPendingSemanticAction(null);
    } else {
      setSemanticPasswordError("활성화 코드가 올바르지 않습니다.");
      semanticPasswordInputRef.current?.select();
    }
  };

  if (isLoading) {
    return (
      <Modal isOpen={isOpen} onClose={onClose} title="설정" size="lg">
        <div className="flex justify-center py-8">
          <div
            className="animate-spin rounded-full h-8 w-8 border-2"
            style={{
              borderColor: "var(--color-border)",
              borderTopColor: "var(--color-accent)",
            }}
          />
        </div>
      </Modal>
    );
  }

  return (
    <>
    <Modal isOpen={isOpen} onClose={onClose} title="설정" size="lg">
      {error && (
        <div
          className="mb-4 p-3 rounded-lg text-sm"
          style={{
            backgroundColor: "rgba(239, 68, 68, 0.1)",
            border: "1px solid rgba(239, 68, 68, 0.3)",
            color: "var(--color-error)",
          }}
        >
          {error}
        </div>
      )}

      {settings && (
        <div className="space-y-5">
          {/* 검색 모드 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              기본 검색 모드
            </label>
            <Dropdown
              options={SEARCH_MODE_OPTIONS}
              value={settings.search_mode}
              onChange={(value) => handleChange("search_mode", value as Settings["search_mode"])}
              placeholder="검색 모드 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              키워드: FTS5 전문 검색 / 하이브리드: 키워드 + 의미 검색 (시맨틱 활성화 필요)
            </p>
          </div>

          {/* 최대 결과 수 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              최대 검색 결과
            </label>
            <Dropdown
              options={MAX_RESULTS_OPTIONS}
              value={String(settings.max_results)}
              onChange={(value) => handleChange("max_results", parseInt(value))}
              placeholder="결과 수 선택"
            />
          </div>

          {/* 더 보기 단위 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              결과 표시 단위
            </label>
            <Dropdown
              options={RESULTS_PER_PAGE_OPTIONS}
              value={String(settings.results_per_page ?? 50)}
              onChange={(value) => handleChange("results_per_page", parseInt(value))}
              placeholder="단위 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              한 번에 표시할 검색 결과 수. "더 보기"를 눌러 추가 로드
            </p>
          </div>

          {/* 최소 신뢰도 */}
          <div>
            <label
              className="flex items-center text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              최소 신뢰도
              <InfoTooltip
                content={
                  <div className="space-y-2 py-1">
                    <div>
                      <strong style={{ color: "var(--color-text-primary)" }}>📊 점수 산정</strong>
                      <p className="mt-0.5">RRF(Reciprocal Rank Fusion) 방식으로 키워드 검색과 의미 검색 순위를 병합해 계산합니다.</p>
                    </div>
                    <div>
                      <strong style={{ color: "var(--color-text-primary)" }}>💡 추천 설정</strong>
                      <ul className="mt-0.5 space-y-0.5">
                        <li>• <strong>0%</strong>: 모든 결과 표시</li>
                        <li>• <strong>20-30%</strong>: 관련성 높은 결과 (권장)</li>
                        <li>• <strong>50%+</strong>: 매우 정확한 결과만</li>
                      </ul>
                    </div>
                    <p className="text-[10px]" style={{ color: "var(--color-text-muted)" }}>같은 문서도 페이지별로 점수가 다를 수 있습니다</p>
                  </div>
                }
                maxWidth={320}
              />
            </label>
            <div className="flex items-center gap-4">
              <input
                type="range"
                min={0}
                max={100}
                step={CONFIDENCE_STEP}
                value={settings.min_confidence}
                onChange={(e) => handleChange("min_confidence", Number(e.target.value))}
                className="flex-1 accent-blue-500"
                aria-label="최소 신뢰도 설정"
              />
              <div
                className="min-w-[48px] text-sm font-semibold text-right"
                style={{ color: "var(--color-text-primary)" }}
              >
                {settings.min_confidence}%
              </div>
            </div>
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              설정 값 미만의 결과는 표시하지 않습니다
            </p>
          </div>

          {/* 테마 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              테마
            </label>
            <Dropdown
              options={THEME_OPTIONS}
              value={settings.theme}
              onChange={(value) => handleChange("theme", value as Settings["theme"])}
              placeholder="테마 선택"
            />
          </div>

          {/* 결과 보기 밀도 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              검색 결과 보기
            </label>
            <Dropdown
              options={VIEW_DENSITY_OPTIONS}
              value={settings.view_density ?? "normal"}
              onChange={(value) => handleChange("view_density", value as Settings["view_density"])}
              placeholder="보기 모드 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              컴팩트: 더 많은 결과를 한 화면에 표시
            </p>
          </div>

          {/* 하위폴더 포함 */}
          <SettingsToggle
            label="하위폴더 포함"
            description="폴더 추가 시 하위폴더도 함께 인덱싱"
            checked={settings.include_subfolders ?? true}
            onChange={(v) => handleChange("include_subfolders", v)}
          />

          {/* 제외 디렉토리 */}
          <div>
            <label
              className="block text-xs font-medium mb-1.5"
              style={{ color: "var(--color-text-secondary)" }}
            >
              제외 디렉토리
            </label>
            <p className="text-xs mb-2" style={{ color: "var(--color-text-muted)" }}>
              인덱싱에서 제외할 폴더 이름 (줄바꿈 구분). 기본 제외: Windows, Program Files, AppData, node_modules 등
            </p>
            <textarea
              className="w-full rounded-md px-3 py-2 text-xs font-mono resize-y"
              style={{
                backgroundColor: "var(--color-bg-secondary)",
                color: "var(--color-text-primary)",
                border: "1px solid var(--color-border)",
                minHeight: "60px",
              }}
              value={(settings.exclude_dirs ?? []).join("\n")}
              onChange={(e) =>
                handleChange(
                  "exclude_dirs",
                  e.target.value
                    .split("\n")
                    .map((s) => s.trim())
                    .filter(Boolean) as unknown as Settings["exclude_dirs"]
                )
              }
              placeholder="추가 제외할 폴더명 입력..."
              rows={3}
            />
          </div>

          {/* 성능 설정 섹션 */}
          <div
            className="border-t pt-4"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3
              className="text-sm font-medium mb-3"
              style={{ color: "var(--color-text-primary)" }}
            >
              성능 설정
            </h3>
          </div>

          {/* 인덱싱 강도 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              인덱싱 강도
            </label>
            <Dropdown
              options={INDEXING_INTENSITY_OPTIONS}
              value={settings.indexing_intensity ?? "balanced"}
              onChange={(value) => handleChange("indexing_intensity", value as Settings["indexing_intensity"])}
              placeholder="강도 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              백그라운드: 다른 작업에 영향 최소화 (HDD 환경 권장)
            </p>
          </div>

          {/* 최대 파일 크기 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              최대 파일 크기
            </label>
            <Dropdown
              options={MAX_FILE_SIZE_OPTIONS}
              value={String(settings.max_file_size_mb ?? 200)}
              onChange={(value) => handleChange("max_file_size_mb", parseInt(value))}
              placeholder="크기 선택"
            />
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              설정 크기 초과 파일은 인덱싱 건너뜀 (0 = 제한 없음)
            </p>
          </div>

          {/* 고급 기능 섹션 */}
          <div className="border-t pt-4" style={{ borderColor: "var(--color-border)" }}>
            <h3 className="text-sm font-medium mb-0.5" style={{ color: "var(--color-text-primary)" }}>고급 기능</h3>
            <p className="text-xs mb-3" style={{ color: "var(--color-text-muted)" }}>
              AI 기반 시맨틱 검색. 활성화 시 ONNX 모델(약 500MB)을 다운로드합니다.
            </p>
          </div>
          <div className="rounded-lg p-3" style={{ backgroundColor: (settings.semantic_search_enabled ?? false) ? "rgba(245, 158, 11, 0.08)" : "var(--color-bg-secondary)", border: `1px solid ${(settings.semantic_search_enabled ?? false) ? "rgba(245, 158, 11, 0.3)" : "var(--color-border)"}` }}>
            <SettingsToggle
              label="시맨틱 검색"
              description="문서의 의미를 이해하여 유사한 내용을 찾아줍니다 (베타 테스트 후 공개)"
              checked={settings.semantic_search_enabled ?? false}
              onChange={handleSemanticToggle}
              activeColor="bg-amber-500"
            />
            {(settings.semantic_search_enabled ?? false) && (
              <div className="mt-3 space-y-3">
                <div className="flex items-start gap-2 p-2 rounded text-xs" style={{ backgroundColor: "rgba(245, 158, 11, 0.1)", color: "var(--color-text-secondary)" }}>
                  <span>메모리 약 1GB 추가 사용. 저사양 PC(4GB RAM)에서는 성능 저하가 발생할 수 있습니다.</span>
                </div>
                <div>
                  <label className="block text-xs font-medium mb-1.5" style={{ color: "var(--color-text-secondary)" }}>벡터 인덱싱 모드</label>
                  <Dropdown options={VECTOR_INDEXING_MODE_OPTIONS} value={settings.vector_indexing_mode ?? "manual"} onChange={(value) => handleChange("vector_indexing_mode", value as Settings["vector_indexing_mode"])} placeholder="모드 선택" />
                </div>
              </div>
            )}
          </div>

          {/* 시스템 트레이 섹션 구분선 */}
          <div
            className="border-t pt-4"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3
              className="text-sm font-medium mb-3"
              style={{ color: "var(--color-text-primary)" }}
            >
              시스템 트레이
            </h3>
          </div>

          {/* 윈도우 시작 시 자동 실행 */}
          <SettingsToggle
            label="Windows 시작 시 자동 실행"
            description="컴퓨터 부팅 시 자동으로 시작"
            checked={settings.auto_start ?? false}
            onChange={(v) => handleChange("auto_start", v)}
          />

          {/* 시작 시 최소화 */}
          <SettingsToggle
            label="시작 시 트레이로 최소화"
            description="앱 시작 시 트레이 아이콘으로 숨김"
            checked={settings.start_minimized ?? false}
            onChange={(v) => handleChange("start_minimized", v)}
          />

          {/* 하이라이트 색상 섹션 */}
          <div
            className="border-t pt-4"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3
              className="text-sm font-medium mb-3"
              style={{ color: "var(--color-text-primary)" }}
            >
              하이라이트 색상
            </h3>
          </div>

          {/* 파일명 하이라이트 색상 */}
          <ColorPresetPicker
            label="파일명 하이라이트"
            description="파일명 검색 결과에서 매칭된 글자 강조 색상"
            presets={HIGHLIGHT_COLOR_PRESETS}
            selectedValue={settings.highlight_filename_color}
            onChange={(v) => handleChange("highlight_filename_color", v)}
          />

          {/* 문서 내용 하이라이트 색상 */}
          <ColorPresetPicker
            label="문서 내용 하이라이트"
            description="문서 검색 결과에서 매칭된 키워드 강조 색상"
            presets={HIGHLIGHT_COLOR_PRESETS}
            selectedValue={settings.highlight_content_color}
            onChange={(v) => handleChange("highlight_content_color", v)}
          />

          {/* 데이터 관리 섹션 */}
          <div
            className="border-t pt-4"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3
              className="text-sm font-medium mb-3"
              style={{ color: "var(--color-text-primary)" }}
            >
              데이터 관리
            </h3>
          </div>

          {/* 데이터 저장 경로 */}
          <div>
            <label
              className="block text-sm font-medium mb-2"
              style={{ color: "var(--color-text-secondary)" }}
            >
              데이터 저장 경로
            </label>
            <div className="flex items-center gap-2">
              <div
                className="flex-1 px-3 py-2 rounded-lg text-sm truncate"
                style={{
                  backgroundColor: "var(--color-bg-primary)",
                  border: "1px solid var(--color-border)",
                  color: settings.data_root
                    ? "var(--color-text-primary)"
                    : "var(--color-text-muted)",
                }}
                title={settings.data_root || "기본 위치 (AppData)"}
              >
                {settings.data_root || "기본 위치 (AppData)"}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={async () => {
                  const selected = await open({
                    directory: true,
                    multiple: false,
                    title: "데이터 저장 폴더 선택",
                  });
                  if (selected) {
                    handleChange("data_root", selected as string);
                  }
                }}
              >
                변경
              </Button>
              {settings.data_root && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleChange("data_root", undefined)}
                >
                  초기화
                </Button>
              )}
            </div>
            <p className="mt-1.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
              DB와 벡터 인덱스 저장 위치. 변경 시 앱 재시작 필요
            </p>
          </div>

          {/* 로그 폴더 열기 */}
          <div className="flex items-center justify-between">
            <div>
              <label
                className="text-sm font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                로그 폴더
              </label>
              <p className="mt-0.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
                오류 추적용 로그 파일 위치 (7일 보존)
              </p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              onClick={async () => {
                try {
                  await invokeWithTimeout("open_log_dir", undefined, IPC_TIMEOUT.FILE_ACTION);
                } catch (err) {
                  const message = err instanceof Error ? err.message : String(err);
                  setError(`로그 폴더 열기 실패: ${message}`);
                }
              }}
            >
              폴더 열기
            </Button>
          </div>

          {/* 모든 데이터 초기화 */}
          <div className="flex items-center justify-between">
            <div>
              <label
                className="text-sm font-medium"
                style={{ color: "var(--color-text-secondary)" }}
              >
                모든 데이터 초기화
              </label>
              <p className="mt-0.5 text-xs" style={{ color: "var(--color-text-muted)" }}>
                인덱싱된 문서, 벡터 임베딩, 등록 폴더 모두 삭제
              </p>
            </div>
            <Button
              variant="danger"
              size="sm"
              isLoading={isClearing}
              disabled={isClearing}
              onClick={async () => {
                const confirmed = await ask(
                  "모든 인덱싱 데이터와 등록된 폴더가 삭제됩니다.\n원본 파일은 영향 없습니다.\n\n계속하시겠습니까?",
                  {
                    title: "데이터 초기화",
                    kind: "warning",
                    okLabel: "초기화",
                    cancelLabel: "취소",
                  }
                );

                if (confirmed && onClearData) {
                  setIsClearing(true);
                  try {
                    await onClearData();
                    onClose();
                  } catch (err) {
                    const message = err instanceof Error ? err.message : String(err);
                    setError(`초기화 실패: ${message}`);
                  } finally {
                    setIsClearing(false);
                  }
                }
              }}
            >
              초기화
            </Button>
          </div>

          {/* 버튼 */}
          <div
            className="flex justify-end gap-3 pt-4 border-t"
            style={{ borderColor: "var(--color-border)" }}
          >
            <Button variant="ghost" onClick={onClose}>
              취소
            </Button>
            <Button
              onClick={saveSettings}
              isLoading={isSaving}
              disabled={isSaving}
            >
              저장
            </Button>
          </div>
        </div>
      )}
    </Modal>

    {/* 시맨틱 검색 활성화 비밀번호 모달 */}
    {showSemanticPassword && (
      <div
        className="fixed inset-0 flex items-center justify-center z-[60]"
        style={{ backgroundColor: "rgba(0, 0, 0, 0.6)" }}
        onClick={(e) => {
          if (e.target === e.currentTarget) {
            setShowSemanticPassword(false);
            setSemanticPasswordInput("");
            setSemanticPasswordError(null);
          }
        }}
      >
        <div
          className="w-full max-w-sm mx-4 rounded-lg"
          style={{
            backgroundColor: "var(--color-bg-secondary)",
            boxShadow: "var(--shadow-lg)",
            border: "1px solid var(--color-border)",
          }}
        >
          <div
            className="px-5 py-4 border-b"
            style={{ borderColor: "var(--color-border)" }}
          >
            <h3 className="text-base font-semibold" style={{ color: "var(--color-text-primary)" }}>
              시맨틱 검색 활성화
            </h3>
          </div>
          <div className="px-5 py-4 space-y-3">
            <p className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              현재 <strong>베타 테스트 중</strong>인 기능입니다. 정식 출시 전 일부 사용자에게만 선별 제공됩니다.
            </p>
            <p className="text-sm" style={{ color: "var(--color-text-muted)" }}>
              활성화하려면 관리자에게 문의하여 활성화 코드를 받으세요.
            </p>
            <div>
              <input
                ref={semanticPasswordInputRef}
                type="password"
                className="w-full px-3 py-2 rounded-md text-sm"
                style={{
                  backgroundColor: "var(--color-bg-primary)",
                  color: "var(--color-text-primary)",
                  border: `1px solid ${semanticPasswordError ? "rgba(239,68,68,0.6)" : "var(--color-border)"}`,
                  outline: "none",
                }}
                placeholder="활성화 코드 입력"
                value={semanticPasswordInput}
                onChange={(e) => {
                  setSemanticPasswordInput(e.target.value);
                  setSemanticPasswordError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSemanticPasswordConfirm();
                  if (e.key === "Escape") {
                    setShowSemanticPassword(false);
                    setSemanticPasswordInput("");
                    setSemanticPasswordError(null);
                  }
                }}
                autoFocus
              />
              {semanticPasswordError && (
                <p className="mt-1 text-xs" style={{ color: "var(--color-error, #ef4444)" }}>
                  {semanticPasswordError}
                </p>
              )}
            </div>
          </div>
          <div
            className="flex justify-end gap-2 px-5 py-4 border-t"
            style={{ borderColor: "var(--color-border)" }}
          >
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setShowSemanticPassword(false);
                setSemanticPasswordInput("");
                setSemanticPasswordError(null);
              }}
            >
              취소
            </Button>
            <Button size="sm" onClick={handleSemanticPasswordConfirm}>
              활성화
            </Button>
          </div>
        </div>
      </div>
    )}
    </>
  );
}
