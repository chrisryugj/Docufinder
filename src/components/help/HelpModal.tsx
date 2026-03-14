import { useState } from "react";
import { Modal } from "../ui/Modal";

interface HelpModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type HelpSection = "start" | "search" | "filters" | "shortcuts" | "tips";

export function HelpModal({ isOpen, onClose }: HelpModalProps) {
  const [activeSection, setActiveSection] = useState<HelpSection>("start");

  const sections: { id: HelpSection; label: string }[] = [
    { id: "start", label: "시작하기" },
    { id: "search", label: "검색 모드" },
    { id: "filters", label: "필터/보기" },
    { id: "shortcuts", label: "단축키" },
    { id: "tips", label: "활용 팁" },
  ];

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Anything 사용 가이드">
      <div className="flex gap-4 min-h-[420px]">
        {/* 사이드 탭 */}
        <nav className="flex flex-col gap-0.5 w-28 flex-shrink-0 border-r pr-3" style={{ borderColor: "var(--color-border)" }}>
          {sections.map((section) => (
            <button
              key={section.id}
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
        </nav>

        {/* 콘텐츠 영역 */}
        <div className="flex-1 overflow-y-auto pr-1" style={{ maxHeight: "460px" }}>
          {activeSection === "start" && <StartSection />}
          {activeSection === "search" && <SearchSection />}
          {activeSection === "filters" && <FiltersSection />}
          {activeSection === "shortcuts" && <ShortcutsSection />}
          {activeSection === "tips" && <TipsSection />}
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
    <p className="text-sm mb-3 leading-relaxed" style={{ color: "var(--color-text-secondary)" }}>
      {children}
    </p>
  );
}

function StepList({ steps }: { steps: string[] }) {
  return (
    <ol className="list-decimal list-inside space-y-2 mb-4">
      {steps.map((step, i) => (
        <li key={i} className="text-sm leading-relaxed" style={{ color: "var(--color-text-secondary)" }}>
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
      <div className="text-sm leading-relaxed" style={{ color: "var(--color-text-muted)" }}>
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
        Anything은 PC에 저장된 문서를 빠르게 찾아주는 검색 앱이에요.
        한글(HWPX), 워드(DOCX), 엑셀(XLSX), PDF, TXT 파일을 모두 검색할 수 있어요.
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

      <SubTitle>전체 PC 인덱싱</SubTitle>
      <Paragraph>
        설정 → 시스템 탭에서 '전체 드라이브 인덱싱'을 실행하면
        PC의 모든 드라이브를 한 번에 인덱싱할 수 있어요.
        시스템 폴더(Windows, Program Files 등)는 자동으로 제외됩니다.
      </Paragraph>

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
        검색바 우측의 드롭다운에서 상황에 맞는 검색 모드를 선택하세요.
      </Paragraph>

      <FeatureBox
        title="키워드 (기본)"
        description="입력한 단어가 정확히 포함된 문서를 찾아요. '계약서'를 검색하면 '계약서'가 들어간 문서만 나와요."
      />
      <FeatureBox
        title="하이브리드 (추천)"
        description="키워드 + 의미 검색을 함께 사용해요. 가장 정확한 결과를 얻을 수 있어요. 시맨틱 검색 활성화 + 모델 다운로드가 필요해요."
      />
      <FeatureBox
        title="시맨틱"
        description="비슷한 의미의 내용도 찾아줘요. '휴가'를 검색하면 '연차', '휴식' 관련 문서도 함께 나와요."
      />
      <FeatureBox
        title="파일명"
        description="파일 이름으로만 검색해요. 내용은 보지 않고, 파일명에 검색어가 포함된 파일을 즉시 찾아요."
      />

      <SubTitle>결과내검색</SubTitle>
      <Paragraph>
        검색 결과가 너무 많을 때, 필터 영역의 '결과내검색' 입력창에
        추가 키워드를 입력하면 현재 결과에서 더 좁혀서 찾을 수 있어요.
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
        HWPX, DOCX, XLSX, PDF, TXT 중 원하는 형식만 선택해서 볼 수 있어요.
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
        <ShortcutRow keys="Ctrl + K" description="검색창으로 이동" />
        <ShortcutRow keys="Ctrl + B" description="사이드바 열기/닫기" />
        <ShortcutRow keys="↑ / ↓" description="결과 목록 이동" />
        <ShortcutRow keys="Enter" description="선택한 파일 열기" />
        <ShortcutRow keys="Ctrl + C" description="선택한 파일 경로 복사" />
        <ShortcutRow keys="Esc" description="선택 해제 / 검색어 지우기" />
      </div>

      <InfoBox>
        검색창에서 바로 화살표 키를 누르면 결과 목록으로 이동해요!
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
          title="결과 내보내기"
          description="검색 결과 상단 메뉴에서 CSV로 내보내거나 클립보드에 복사할 수 있어요. 보고서 작성에 유용!"
        />
        <FeatureBox
          title="우클릭 메뉴"
          description="검색 결과를 우클릭하면 파일 열기, 경로 복사, 폴더 열기 등 추가 옵션을 사용할 수 있어요."
        />
      </div>
    </div>
  );
}
