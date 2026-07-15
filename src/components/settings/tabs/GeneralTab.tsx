import { useState, useCallback } from "react";
import { Dropdown } from "../../ui/Dropdown";
import { SettingsToggle } from "../SettingsToggle";
import type { Settings } from "../../../types/settings";
import type { TabProps } from "./types";
import {
  THEME_OPTIONS,
  SEARCH_MODE_OPTIONS,
  MAX_RESULTS_OPTIONS,
  RESULTS_PER_PAGE_OPTIONS,
  VIEW_DENSITY_OPTIONS,
  OPEN_CLICK_OPTIONS,
  UI_ZOOM_OPTIONS,
} from "./types";

const ZOOM_STORAGE_KEY = "docufinder-ui-zoom";

function getStoredZoom(): string {
  try {
    return localStorage.getItem(ZOOM_STORAGE_KEY) || "1";
  } catch {
    return "1";
  }
}

export function GeneralTab({ settings, onChange }: TabProps) {
  const [uiZoom, setUiZoom] = useState(getStoredZoom);

  const handleZoomChange = useCallback((value: string) => {
    setUiZoom(value);
    localStorage.setItem(ZOOM_STORAGE_KEY, value);
    document.documentElement.style.zoom = value;
  }, []);

  return (
    <div className="space-y-3">
      {/* 테마 + 검색 모드 (2열) */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            테마
          </label>
          <Dropdown
            options={THEME_OPTIONS}
            value={settings.theme}
            onChange={(value) => onChange("theme", value as Settings["theme"])}
            placeholder="테마 선택"
          />
        </div>
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            기본 검색 모드
          </label>
          <Dropdown
            options={SEARCH_MODE_OPTIONS}
            value={settings.search_mode}
            onChange={(value) => onChange("search_mode", value as Settings["search_mode"])}
            placeholder="검색 모드 선택"
          />
        </div>
      </div>
      <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
        키워드: FTS5 전문 검색 / 파일명: 파일 이름으로 검색
      </p>

      {/* 최대 결과 + 표시 단위 (2열) */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            최대 검색 결과
          </label>
          <Dropdown
            options={MAX_RESULTS_OPTIONS}
            value={String(settings.max_results)}
            onChange={(value) => onChange("max_results", parseInt(value))}
            placeholder="결과 수 선택"
          />
        </div>
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            결과 표시 단위
          </label>
          <Dropdown
            options={RESULTS_PER_PAGE_OPTIONS}
            value={String(settings.results_per_page ?? 50)}
            onChange={(value) => onChange("results_per_page", parseInt(value))}
            placeholder="단위 선택"
          />
        </div>
      </div>
      <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
        한 번에 표시할 결과 수. "더 보기"를 눌러 추가 로드
      </p>

      {/* 결과 보기 밀도 + UI 줌 (2열) */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            검색 결과 보기
          </label>
          <Dropdown
            options={VIEW_DENSITY_OPTIONS}
            value={settings.view_density ?? "normal"}
            onChange={(value) => onChange("view_density", value as Settings["view_density"])}
            placeholder="보기 모드 선택"
          />
        </div>
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            UI 크기
          </label>
          <Dropdown
            options={UI_ZOOM_OPTIONS}
            value={uiZoom}
            onChange={handleZoomChange}
            placeholder="줌 선택"
          />
        </div>
      </div>
      <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
        UI 크기를 조절하면 글자와 인터페이스 전체가 확대/축소됩니다
      </p>

      {/* 파일 열기 방식 */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-sm font-medium mb-1" style={{ color: "var(--color-text-secondary)" }}>
            파일 열기 방식
          </label>
          <Dropdown
            options={OPEN_CLICK_OPTIONS}
            value={settings.open_on_single_click ? "single" : "double"}
            onChange={(value) => onChange("open_on_single_click", value === "single")}
            placeholder="열기 방식 선택"
          />
        </div>
      </div>
      <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
        {settings.open_on_single_click
          ? "검색 결과를 한 번 클릭하면 바로 파일이 열립니다"
          : "한 번 클릭은 선택·미리보기, 두 번 클릭(또는 Enter)으로 파일을 엽니다"}
      </p>

      {/* 저장 위치 표시 */}
      <SettingsToggle
        label="결과에 저장 위치 표시"
        description="검색 결과 카드에 폴더 경로와 액션 버튼 줄(경로 복사·위치 열기)을 표시합니다 (컴팩트 보기 포함). 경로 클릭 시 탐색기에서 열립니다. 끄면 우클릭 메뉴로 대체"
        checked={settings.show_result_path ?? true}
        onChange={(checked) => onChange("show_result_path", checked)}
      />

      {/* 수정 날짜/시간 표시 형식 */}
      <SettingsToggle
        label="수정 날짜·시간 표시"
        description="검색 결과에 파일 수정 시각을 날짜·시간(예: 2026. 07. 15. 14:30)으로 표시합니다. 끄면 상대시간(예: 3일 전)으로 표시하고, 정확한 날짜·시간은 마우스를 올리면 나타납니다."
        checked={settings.show_absolute_time ?? true}
        onChange={(checked) => onChange("show_absolute_time", checked)}
      />
    </div>
  );
}
