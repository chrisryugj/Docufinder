import type { TourStep } from "./OnboardingTour";

/**
 * Docufinder 기능 투어 스텝 정의
 *
 * 각 단계는 앱의 핵심 기능을 **친절한 어투**로 안내한다.
 * data-tour 속성으로 타겟을 명시하며, 해당 요소가 DOM에 없으면
 * 화면 중앙 메시지로 폴백된다.
 */
export const DOCUFINDER_TOUR_STEPS: TourStep[] = [
  {
    selector: null,
    title: "Docufinder에 오신 것을 환영합니다 👋",
    body: (
      <div className="space-y-2">
        <p>
          Docufinder는 내 컴퓨터 안의 문서를 <strong>단어 하나만 넣어도</strong> 찾아주는
          검색기입니다.
        </p>
        <p className="text-[12px] opacity-80">
          약 1분이면 핵심 기능을 모두 둘러볼 수 있어요. 언제든 <kbd>ESC</kbd> 키로
          닫을 수 있습니다.
        </p>
      </div>
    ),
    placement: "auto",
  },
  {
    selector: '[data-tour="search-bar"]',
    title: "1. 검색은 여기서 — 제목과 내용 모두",
    body: (
      <div className="space-y-2">
        <p>
          파일명, 문서 본문, PDF·HWPX·DOCX·XLSX 속 글자까지 한 번에 검색됩니다.
        </p>
        <p className="text-[12px] opacity-80">
          예: <code>보고서</code>, <code>2024 예산</code>, <code>근로기준법 제5조</code>
        </p>
        <p className="text-[12px] opacity-80">
          검색 후 파일 형식·수정 기간·검색 범위로 결과를 더 좁힐 수 있어요.
        </p>
      </div>
    ),
    placement: "bottom",
    padding: 6,
  },
  {
    selector: '[data-tour="sidebar-folders"]',
    title: "2. 검색할 폴더를 추가하세요",
    body: (
      <div className="space-y-2">
        <p>
          <strong>+ 버튼</strong>으로 폴더를 추가하면 자동으로 인덱싱이 시작됩니다.
          추가된 폴더는 항상 최신 상태로 유지돼요 (파일 변경 자동 감지).
        </p>
        <p className="text-[12px] opacity-80">
          드라이브 루트(C:\, D:\ 등)도 추가 가능 — 시스템 폴더는 자동 제외됩니다.
        </p>
      </div>
    ),
    placement: "right",
    padding: 6,
  },
  {
    selector: '[data-tour="settings-button"]',
    title: "3. 전체 드라이브 인덱싱은 여기서",
    body: (
      <div className="space-y-2">
        <p>
          설정 ▸ <strong>시스템</strong> 탭의 <em>"전체 드라이브 인덱싱"</em> 버튼으로
          컴퓨터의 모든 드라이브를 한 번에 인덱싱할 수 있어요.
        </p>
        <p className="text-[12px] opacity-80">
          각 드라이브의 진행 상태는 사이드바 상단에 실시간으로 표시됩니다 —
          중간에 멈춘 것처럼 보여도 "DB 저장 중", "캐시 정리 중" 단계가
          순서대로 표시되니 안심하고 기다리시면 돼요.
        </p>
      </div>
    ),
    placement: "bottom",
  },
  {
    selector: '[data-tour="help-button"]',
    title: "4. 투어는 언제든 다시 볼 수 있어요",
    body: (
      <div className="space-y-2">
        <p>
          도움말 버튼을 누르면 단축키 목록과 <strong>"기능 투어 다시 보기"</strong>
          옵션을 찾을 수 있습니다.
        </p>
        <p className="text-[12px] opacity-80">
          이제 첫 번째 폴더를 추가하고 검색을 시작해보세요! 🚀
        </p>
      </div>
    ),
    placement: "bottom",
  },
];

export const DOCUFINDER_TOUR_STORAGE_KEY = "docufinder-feature-tour-v1";
