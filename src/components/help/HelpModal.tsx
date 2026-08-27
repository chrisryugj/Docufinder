import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { Play } from "lucide-react";
import { Modal } from "../ui/Modal";
import { SYSTEM_FOLDERS_HINT, HAS_DRIVES, MOD_KEY } from "../../utils/platform";

export type HelpSection = "start" | "search" | "filters" | "advanced" | "shortcuts" | "tips";

interface HelpModalProps {
  isOpen: boolean;
  onClose: () => void;
  onRestartTour?: () => void;
  /** 열릴 때 표시할 탭 (예: 검색창 ? 버튼 → "search") */
  initialSection?: HelpSection;
}

export function HelpModal({ isOpen, onClose, onRestartTour, initialSection = "start" }: HelpModalProps) {
  const [activeSection, setActiveSection] = useState<HelpSection>(initialSection);
  // 설치 버전 — 망분리(자동 업데이트 불가) 환경에서 배포본과 대조하는 용도 (#42)
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);

  // 열릴 때마다 요청된 탭으로 — 닫힌 상태에서 initialSection이 바뀌어도 반영되도록
  useEffect(() => {
    if (isOpen) setActiveSection(initialSection);
  }, [isOpen, initialSection]);

  const sections: { id: HelpSection; label: string }[] = [
    { id: "start", label: "시작하기" },
    { id: "search", label: "검색 모드" },
    { id: "filters", label: "필터/보기" },
    { id: "advanced", label: "고급 기능" },
    { id: "shortcuts", label: "단축키" },
    { id: "tips", label: "활용 팁" },
  ];

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Anything 사용 가이드">
      <div className="flex gap-4 min-h-[420px]">
        {/* 사이드 탭 */}
        <nav className="flex flex-col gap-0.5 w-28 flex-shrink-0 border-r pr-3" role="tablist" aria-label="도움말 탭" style={{ borderColor: "var(--color-border)" }}>
          {sections.map((section) => (
            <button
              key={section.id}
              id={`help-tab-${section.id}`}
              role="tab"
              aria-selected={activeSection === section.id}
              aria-controls={`help-panel-${section.id}`}
              onClick={() => setActiveSection(section.id)}
              className={`px-3 py-2 text-left text-sm rounded-lg transition-colors whitespace-nowrap ${
                activeSection === section.id ? "font-semibold" : ""
              }`}
              style={{
                backgroundColor: activeSection === section.id ? "var(--color-bg-tertiary)" : "transparent",
                color: activeSection === section.id ? "var(--color-text-primary)" : "var(--color-text-muted)",
              }}
            >
              {section.label}
            </button>
          ))}
          {appVersion && (
            <div
              className="mt-auto pt-3 px-3 text-[11px] tabular-nums"
              style={{ color: "var(--color-text-muted)" }}
              title="설치된 버전 — 새 버전은 GitHub Releases 의 setup.exe/dmg 로 설치합니다"
            >
              Anything v{appVersion}
            </div>
          )}
        </nav>

        {/* 콘텐츠 영역 */}
        <div className="flex-1 overflow-y-auto pr-1" style={{ maxHeight: "460px" }}>
          {activeSection === "start" && (
            <div role="tabpanel" id="help-panel-start" aria-labelledby="help-tab-start">
              {onRestartTour && (
                <button
                  onClick={() => {
                    onRestartTour();
                    onClose();
                  }}
                  className="mb-4 w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-sm font-semibold transition-all hover:shadow-md"
                  style={{
                    backgroundColor: "var(--color-accent)",
                    color: "#fff",
                  }}
                >
                  <Play className="w-4 h-4" />
                  기능 투어 다시 보기
                </button>
              )}
              <StartSection />
            </div>
          )}
          {activeSection === "search" && <div role="tabpanel" id="help-panel-search" aria-labelledby="help-tab-search"><SearchSection /></div>}
          {activeSection === "filters" && <div role="tabpanel" id="help-panel-filters" aria-labelledby="help-tab-filters"><FiltersSection /></div>}
          {activeSection === "advanced" && <div role="tabpanel" id="help-panel-advanced" aria-labelledby="help-tab-advanced"><AdvancedSection /></div>}
          {activeSection === "shortcuts" && <div role="tabpanel" id="help-panel-shortcuts" aria-labelledby="help-tab-shortcuts"><ShortcutsSection /></div>}
          {activeSection === "tips" && <div role="tabpanel" id="help-panel-tips" aria-labelledby="help-tab-tips"><TipsSection /></div>}
        </div>
      </div>
    </Modal>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="text-base font-bold mb-3" style={{ color: "var(--color-text-primary)" }}>
      {children}
    </h3>
  );
}

function SubTitle({ children }: { children: React.ReactNode }) {
  return (
    <h4 className="text-sm font-semibold mb-1.5 mt-4" style={{ color: "var(--color-text-primary)" }}>
      {children}
    </h4>
  );
}

function Paragraph({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-sm mb-3 leading-relaxed" style={{ color: "var(--color-text-secondary)", wordBreak: "keep-all" }}>
      {children}
    </p>
  );
}

function StepList({ steps }: { steps: string[] }) {
  return (
    <ol className="list-decimal list-inside space-y-2 mb-4">
      {steps.map((step, i) => (
        <li key={i} className="text-sm leading-relaxed" style={{ color: "var(--color-text-secondary)", wordBreak: "keep-all" }}>
          {step}
        </li>
      ))}
    </ol>
  );
}

function FeatureBox({ title, description }: { title: string; description: string }) {
  return (
    <div
      className="p-3 rounded-lg mb-2"
      style={{ backgroundColor: "var(--color-bg-tertiary)" }}
    >
      <div className="font-semibold text-sm mb-1" style={{ color: "var(--color-text-primary)" }}>
        {title}
      </div>
      <div className="text-sm leading-relaxed" style={{ color: "var(--color-text-muted)", wordBreak: "keep-all" }}>
        {description}
      </div>
    </div>
  );
}

function ShortcutRow({ keys, description }: { keys: string; description: string }) {
  return (
    <div className="flex items-center justify-between py-2.5 border-b" style={{ borderColor: "var(--color-border)" }}>
      <span className="text-sm" style={{ color: "var(--color-text-secondary)" }}>{description}</span>
      <kbd
        className="px-2.5 py-1 text-xs rounded font-mono"
        style={{
          backgroundColor: "var(--color-bg-tertiary)",
          color: "var(--color-text-primary)",
          border: "1px solid var(--color-border)",
        }}
      >
        {keys}
      </kbd>
    </div>
  );
}

function InfoBox({ children }: { children: React.ReactNode }) {
  return (
    <div
      className="mt-4 p-3 rounded-lg text-sm leading-relaxed"
      style={{
        backgroundColor: "var(--color-accent-bg)",
        color: "var(--color-accent)",
        wordBreak: "keep-all" as const,
      }}
    >
      {children}
    </div>
  );
}

// === 섹션 컴포넌트들 ===

function StartSection() {
  return (
    <div>
      <SectionTitle>Anything에 오신 것을 환영합니다!</SectionTitle>
      <Paragraph>
        Anything은 PC에 저장된 문서를 빠르게 찾아주는 검색 앱이에요. 한글(HWPX), 워드(DOCX), 파워포인트(PPTX), 엑셀(XLSX), PDF, TXT 파일은 물론 이미지(OCR)까지 검색할 수 있어요.
      </Paragraph>

      <SubTitle>처음 사용하신다면</SubTitle>
      <StepList
        steps={[
          "헤더의 '폴더 추가' 버튼을 클릭하세요",
          "검색하고 싶은 문서가 있는 폴더를 선택하세요",
          "잠시 기다리면 인덱싱(문서 분석)이 완료돼요",
          "검색창에 찾고 싶은 내용을 입력하면 끝!",
        ]}
      />

      {HAS_DRIVES && (
        <>
          <SubTitle>전체 PC 인덱싱</SubTitle>
          <Paragraph>
            설정 → 시스템 탭에서 '전체 드라이브 인덱싱'을 실행하면
            PC의 모든 드라이브를 한 번에 인덱싱할 수 있어요.
            시스템 폴더({SYSTEM_FOLDERS_HINT})는 자동으로 제외됩니다.
          </Paragraph>
        </>
      )}

      <InfoBox>
        폴더를 추가하면 파일 변경을 자동 감지해요. 새 파일이 추가되면 자동으로 검색 대상에 포함됩니다!
      </InfoBox>
    </div>
  );
}

function SearchSection() {
  return (
    <div>
      <SectionTitle>검색 모드</SectionTitle>
      <Paragraph>
        기본 검색 모드는 설정 → 일반에서 바꿀 수 있어요. 결과 화면의 '옵션'에서도 전환됩니다.
      </Paragraph>

      <FeatureBox
        title="키워드 (기본)"
        description="입력한 단어가 정확히 포함된 문서를 찾아요. '계약서'를 검색하면 '계약서'가 들어간 문서만 나와요."
      />
      <FeatureBox
        title="파일명"
        description="파일 이름으로만 검색해요. 내용은 보지 않고, 파일명에 검색어가 포함된 파일을 즉시 찾아요."
      />

      <SubTitle>검색 연산자 (키워드 검색)</SubTitle>
      <Paragraph>
        키워드 검색에서 아래 연산자를 입력하면 검색을 더 정밀하게 조절할 수 있어요. 입력 중 검색창 아래에 인식된 연산자가 칩으로 표시돼요.
      </Paragraph>
      <div className="space-y-0 mb-4">
        <ShortcutRow keys={'"정확 구문"'} description="따옴표 안 구문이 그대로 포함된 문서만" />
        <ShortcutRow keys="-제외어" description={'해당 단어 포함 문서 제외 (-"제외 구문"도 가능)'} />
        <ShortcutRow keys="ext:hwp,pdf" description="확장자 필터 (hwp는 hwpx까지 자동 확장)" />
        <ShortcutRow keys="path:2024예산" description="파일 경로에 해당 단어가 포함된 문서만" />
        <ShortcutRow keys="after:2024-06" description="이 날짜 이후 수정된 문서 (YYYY-MM-DD)" />
        <ShortcutRow keys="before:2025-01-31" description="이 날짜 이전 수정된 문서" />
        <ShortcutRow keys="~10" description="근접 검색: 단어들이 10단어 이내 (~만 쓰면 10)" />
      </div>
      <InfoBox>
        연산자는 조합할 수 있어요. 예: ext:hwp "감사 결과" -초안 path:2024 — 키워드 모드에서만 동작해요.
      </InfoBox>

      <SubTitle>자연어 검색 (스마트)</SubTitle>
      <Paragraph>
        검색바 상단의 토글에서 '스마트' 모드로 전환하면 일상 언어로 검색할 수 있어요. "지난주 예산 관련 한글 문서 찾아줘"처럼 입력하고 Enter를 누르면, 검색어·파일 형식·날짜 범위를 자동으로 분석해서 결과를 보여줘요.
      </Paragraph>

      <SubTitle>Anything에게 질문 (AI 답변)</SubTitle>
      <Paragraph>
        같은 토글에서 'Anything' 모드를 선택하면 문서에 대해 질문할 수 있어요. "연차 사용 조건이 어떻게 되나요?"처럼 입력하고 Enter를 누르면 관련 문서를 찾아 내용을 근거로 답변해줘요. 설정 → AI 탭에서 API 키를 등록해야 동작해요.
      </Paragraph>

      <SubTitle>검색어 자동완성</SubTitle>
      <Paragraph>
        검색창에 2글자 이상 입력하면 이전 검색 기록과 문서 내 어휘를 기반으로 검색어를 제안해요. 화살표 키로 선택하거나 클릭하면 바로 적용돼요.
      </Paragraph>

      <SubTitle>결과내검색</SubTitle>
      <Paragraph>
        검색 결과가 너무 많을 때, 필터 영역의 '결과내검색' 입력창에 추가 키워드를 입력하면 현재 결과에서 더 좁혀서 찾을 수 있어요.
      </Paragraph>
    </div>
  );
}

function FiltersSection() {
  return (
    <div>
      <SectionTitle>필터와 보기 방식</SectionTitle>
      <Paragraph>
        검색 결과가 나온 후 필터를 사용하면 원하는 문서를 더 빨리 찾을 수 있어요.
      </Paragraph>

      <SubTitle>파일 형식 필터</SubTitle>
      <Paragraph>
        HWPX, DOCX, PPTX, XLSX, PDF, TXT 중 원하는 형식만 선택해서 볼 수 있어요.
      </Paragraph>

      <SubTitle>검색 범위</SubTitle>
      <Paragraph>
        특정 폴더 내에서만 검색하고 싶을 때, 검색 범위를 지정할 수 있어요.
      </Paragraph>

      <SubTitle>보기 방식</SubTitle>
      <FeatureBox
        title="일반 보기"
        description="검색 결과를 하나씩 보여줘요. 각 결과의 내용 미리보기를 자세히 볼 수 있어요."
      />
      <FeatureBox
        title="그룹 보기"
        description="같은 파일의 여러 결과를 묶어서 보여줘요. 한 파일에서 여러 곳이 검색됐을 때 유용해요."
      />

      <SubTitle>결과 밀도</SubTitle>
      <Paragraph>
        설정에서 '기본' 또는 '컴팩트' 모드를 선택할 수 있어요.
        컴팩트 모드는 한 화면에 더 많은 결과를 보여줘요.
      </Paragraph>
    </div>
  );
}

function ShortcutsSection() {
  return (
    <div>
      <SectionTitle>키보드 단축키</SectionTitle>
      <Paragraph>
        마우스 없이 키보드만으로도 빠르게 사용할 수 있어요.
      </Paragraph>

      <div className="space-y-0">
        <ShortcutRow keys={`${MOD_KEY} + K`} description="명령 팔레트 열기" />
        <ShortcutRow keys="/" description="검색창으로 이동" />
        <ShortcutRow keys={`${MOD_KEY} + B`} description="사이드바 열기/닫기" />
        <ShortcutRow keys="↑ / ↓" description="결과 목록 이동" />
        <ShortcutRow keys="Enter" description="선택한 파일 열기" />
        <ShortcutRow keys={`${MOD_KEY} + F`} description="미리보기에서 문서 내 찾기" />
        <ShortcutRow keys={`${MOD_KEY} + Shift + C`} description="선택한 파일 경로 복사" />
        <ShortcutRow keys="Esc" description="선택 해제 / 검색어 지우기" />
      </div>

      <InfoBox>
        검색창에서 바로 화살표 키를 누르면 결과 목록으로 이동해요!
      </InfoBox>
    </div>
  );
}

function AdvancedSection() {
  return (
    <div>
      <SectionTitle>고급 기능</SectionTitle>
      <Paragraph>
        헤더 우측 아이콘 버튼들로 고급 기능에 접근할 수 있어요.
      </Paragraph>

      <SubTitle>중복 문서 탐지</SubTitle>
      <FeatureBox
        title="정확 중복"
        description="SHA-256 해시로 파일 내용이 완전히 동일한 문서를 찾아요. 이름이 달라도 내용이 같으면 탐지됩니다."
      />
      <FeatureBox
        title="유사 중복"
        description="AI 벡터 분석으로 내용이 비슷한 문서를 찾아요 (90% 이상 유사도). 시맨틱 검색이 활성화되어 있어야 해요."
      />

      <SubTitle>만료 문서 스캔</SubTitle>
      <Paragraph>
        문서 내 '만료', '유효기간', '계약기간', '기한' 등의 키워드 주변 날짜를 자동 추출해요.
        만료됨 / 7일 이내 / 30일 이내 / 여유 4단계로 분류하여 긴급도를 한눈에 볼 수 있어요.
      </Paragraph>

      <SubTitle>문서 요약</SubTitle>
      <Paragraph>
        검색 결과를 클릭하면 미리보기 패널에서 '요약' 버튼으로 문서 핵심 내용을 자동 추출해요.
        요약은 설정한 AI 제공자(Gemini 또는 OpenAI 호환 서버)를 호출하므로 API 키가 필요해요.
      </Paragraph>

      <SubTitle>북마크</SubTitle>
      <Paragraph>
        미리보기 패널에서 중요한 문서를 북마크하고 메모를 남길 수 있어요.
        사이드바의 북마크 탭에서 저장한 문서를 바로 열 수 있어요.
      </Paragraph>

      <SubTitle>문서 통계</SubTitle>
      <Paragraph>
        헤더의 차트 아이콘을 클릭하면 인덱싱된 문서의 파일 형식별 분포,
        폴더별 문서 수, 총 용량 등 통계를 확인할 수 있어요.
      </Paragraph>

      <SubTitle>법령 참조 링크</SubTitle>
      <Paragraph>
        문서 미리보기에서 '민법 제750조', '근로기준법 제54조' 같은 법령 표현을
        자동 감지하여 law.go.kr 링크로 변환해줘요. 클릭하면 바로 법령 원문을 볼 수 있어요.
      </Paragraph>

      <SubTitle>유사 문서 찾기</SubTitle>
      <Paragraph>
        검색 결과를 우클릭 → '유사 문서 찾기'로 선택한 문서와 비슷한 내용의 다른 문서를 벡터 유사도 기반으로 찾아줘요.
      </Paragraph>

      <SubTitle>문서 비교</SubTitle>
      <Paragraph>
        검색 결과를 우클릭 → '비교 대상으로 선택'으로 기준 문서를 정한 뒤, 다른 결과를 우클릭하면 '○○와 비교' 메뉴가 생겨요. 두 문서의 달라진 부분을 나란히 보여줘서 버전이 여러 개인 문서를 정리할 때 유용해요.
      </Paragraph>

      <SubTitle>문서 크게 보기 (팝업 뷰어)</SubTitle>
      <Paragraph>
        미리보기 패널의 '문서 크게 보기' 버튼을 누르면 원본 서식 그대로 전체화면 팝업으로 볼 수 있어요. 휠은 스크롤, {MOD_KEY}+휠(트랙패드 핀치)은 커서 위치 기준 확대·축소, 더블클릭은 확대 토글이에요. 너비 맞춤 ↔ 페이지 맞춤 버튼도 있어요. PDF는 페이지 맨 위·아래에서 휠을 계속 굴리면 이전/다음 페이지로 넘어가요 (미리보기·팝업 공통).
      </Paragraph>

      <SubTitle>원본 레이아웃 보기</SubTitle>
      <Paragraph>
        미리보기 상단에서 '원본 레이아웃'(단축키 2)으로 전환하면 문서를 원본 조판 모습 그대로 볼 수 있어요. HWPX와 PDF는 물론 .hwp 파일도 한컴오피스 설치 없이 원본 레이아웃으로 렌더돼요. 텍스트 보기(단축키 1)와 자유롭게 오갈 수 있고, 찾기·크게 보기도 그대로 동작해요.
      </Paragraph>

      <SubTitle>OCR (이미지 텍스트 인식)</SubTitle>
      <Paragraph>
        설정 → 검색 탭에서 OCR을 켜면 PNG, JPG, BMP, TIFF 등 이미지 파일과 스캔된 PDF의 텍스트를 인식해서 검색할 수 있어요. PaddleOCR 기반으로 오프라인에서 동작해요. 켜는 시점에 이미 인덱싱된 이미지·스캔 PDF가 있으면 재인덱싱할지 물어봐요.
      </Paragraph>

      <SubTitle>PDF 수식 OCR</SubTitle>
      <Paragraph>
        설정 → 검색 탭에서 'PDF 수식 OCR'을 켜면 PDF 속 수학 수식을 LaTeX로 추출해 검색하고, 미리보기에서 수식 모양 그대로 렌더해줘요. 최초 1회 모델(약 155MB)을 자동 다운로드해요.
      </Paragraph>

      <SubTitle>망분리(폐쇄망) 환경</SubTitle>
      <Paragraph>
        OCR에 필요한 파일은 설치본에 모두 들어 있어서 인터넷 없이도 동작해요. 네트워크 시도 자체를 막고 싶으면 시스템 환경변수 'DOCUFINDER_OFFLINE'을 '1'로 설정하세요 — 평소에는 설정할 필요 없어요. 이 경우 '레이아웃 분석'(온라인 전용 선택 기능)만 꺼지고 OCR과 검색은 그대로예요.
      </Paragraph>

      <SubTitle>자동 문서 분류</SubTitle>
      <Paragraph>
        인덱싱된 문서를 내용 기반으로 자동 분류해요. 검색 결과에서 문서 유형 태그를 확인할 수 있어요.
      </Paragraph>

      <SubTitle>결과 내보내기</SubTitle>
      <Paragraph>
        검색 결과 상단 메뉴에서 CSV, XLSX(엑셀)로 내보내거나 클립보드에 복사할 수 있어요. ZIP 패키지로 원본 파일을 묶어서 내보내는 것도 가능해요.
      </Paragraph>

      <InfoBox>
        중복 탐지와 만료 스캔은 인덱싱된 문서를 대상으로 분석해요. 먼저 폴더를 추가하고 인덱싱을 완료해주세요!
      </InfoBox>
    </div>
  );
}

function TipsSection() {
  return (
    <div>
      <SectionTitle>활용 팁</SectionTitle>

      <div className="space-y-2">
        <FeatureBox
          title="파일 열기 방식"
          description="기본은 한 번 클릭 = 미리보기, 두 번 클릭(또는 Enter) = 파일 열기예요. 설정 → 일반 → '파일 열기 방식'에서 '한 번 클릭'으로 바꿀 수 있어요."
        />
        <FeatureBox
          title="즐겨찾기 폴더"
          description="자주 검색하는 폴더는 사이드바에서 우클릭 → 즐겨찾기로 등록하세요. 목록 상단에 고정됩니다."
        />
        <FeatureBox
          title="신뢰도 점수"
          description="검색 결과 옆의 퍼센트(%)는 검색어와의 관련도예요. 높을수록 더 정확한 결과입니다."
        />
        <FeatureBox
          title="실시간 파일 감시"
          description="등록한 폴더의 파일이 추가/수정/삭제되면 자동으로 반영돼요. 따로 다시 인덱싱할 필요 없어요."
        />
        <FeatureBox
          title="테마 변경"
          description="설정 → 일반 탭에서 라이트, 다크, 시스템 설정 중 원하는 테마를 선택하세요."
        />
        <FeatureBox
          title="스마트 폴더 (검색 저장)"
          description="자주 쓰는 검색 조건은 사이드바의 '현재 검색을 스마트 폴더로 저장' 버튼으로 저장하세요. 클릭 한 번으로 같은 검색을 다시 실행해요."
        />
        <FeatureBox
          title="드래그로 꺼내기"
          description="검색 결과 파일을 그대로 끌어서 폴더, 메일, 메신저 등 다른 앱에 놓을 수 있어요. 탐색기를 거칠 필요가 없어요."
        />
        <FeatureBox
          title="결과 내보내기"
          description="검색 결과 상단 메뉴에서 CSV, XLSX(엑셀)로 내보내거나 클립보드에 복사할 수 있어요. ZIP 패키지로 원본 파일을 묶을 수도 있어요."
        />
        <FeatureBox
          title="우클릭 메뉴"
          description="검색 결과를 우클릭하면 파일 열기, 경로 복사, 폴더 열기 등 추가 옵션을 사용할 수 있어요."
        />
      </div>
    </div>
  );
}
