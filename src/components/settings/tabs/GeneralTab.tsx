import { Dropdown } from "../../ui/Dropdown";
import type { Settings } from "../../../types/settings";
import type { TabProps } from "./types";
import {
  THEME_OPTIONS,
  SEARCH_MODE_OPTIONS,
  MAX_RESULTS_OPTIONS,
  RESULTS_PER_PAGE_OPTIONS,
  VIEW_DENSITY_OPTIONS,
} from "./types";

export function GeneralTab({ settings, onChange }: TabProps) {
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

      {/* 결과 보기 밀도 */}
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
    </div>
  );
}
