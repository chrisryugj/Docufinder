import { useState, useEffect, useCallback } from "react";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";

interface OnboardingModalProps {
  isOpen: boolean;
  onComplete: () => void;
  onSkip: () => void;
}

interface Step {
  title: string;
  description: string;
  icon: React.ReactNode;
  tip?: string;
}

const steps: Step[] = [
  {
    title: "폴더 추가하기",
    description:
      "검색할 폴더를 추가하세요.\n헤더의 '폴더 추가' 버튼 또는 사이드바에서 추가할 수 있어요.\n추가한 폴더의 문서가 자동으로 분석됩니다.",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
      </svg>
    ),
    tip: "전체 PC를 한 번에 인덱싱할 수도 있어요!",
  },
  {
    title: "문서 검색하기",
    description:
      "검색창에 찾고 싶은 내용을 입력하세요.\n예: '2024년 예산 집행현황', '민원처리 규정'\n\nHWPX, DOCX, XLSX, PDF, TXT 문서를 모두 검색할 수 있어요.",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
    ),
  },
  {
    title: "검색 모드 활용",
    description:
      "검색바 우측의 모드 전환으로 상황에 맞는 검색을 선택하세요.\n\n• 키워드: 정확한 단어 매칭\n• 하이브리드: 키워드 + 의미 검색 (추천)\n• 파일명: 파일 이름으로만 검색",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
      </svg>
    ),
    tip: "시맨틱 검색은 설정에서 활성화할 수 있어요",
  },
  {
    title: "준비 완료!",
    description:
      "폴더를 추가하면 파일 변경을 자동 감지해요.\n새 파일이 추가되면 자동으로 검색 대상에 포함됩니다.\n\n도움말은 헤더의 '?' 버튼에서 언제든 확인하세요.",
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
];

export function OnboardingModal({ isOpen, onComplete, onSkip }: OnboardingModalProps) {
  const [currentStep, setCurrentStep] = useState(0);

  const goToNext = useCallback(() => {
    if (currentStep < steps.length - 1) {
      setCurrentStep((prev) => prev + 1);
    } else {
      onComplete();
    }
  }, [currentStep, onComplete]);

  const goToPrev = useCallback(() => {
    if (currentStep > 0) {
      setCurrentStep((prev) => prev - 1);
    }
  }, [currentStep]);

  // 키보드 네비게이션
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) return;
      if (e.key === "ArrowRight") {
        goToNext();
      } else if (e.key === "ArrowLeft") {
        goToPrev();
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, goToNext, goToPrev]);

  const step = steps[currentStep];
  const isLastStep = currentStep === steps.length - 1;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onSkip}
      title="Anything 시작하기"
      size="lg"
      closable={true}
    >
      <div className="space-y-5">
        {/* 아이콘 및 설명 */}
        <div className="flex flex-col items-center text-center py-4">
          <div
            className="w-16 h-16 rounded-xl flex items-center justify-center mb-4"
            style={{
              backgroundColor: "var(--color-accent-light)",
              color: "var(--color-accent)",
            }}
          >
            {step.icon}
          </div>
          <h3
            className="text-lg font-bold mb-3"
            style={{ color: "var(--color-text-primary)", letterSpacing: "-0.01em" }}
          >
            {step.title}
          </h3>
          <p
            className="text-sm leading-relaxed max-w-sm whitespace-pre-line"
            style={{ color: "var(--color-text-secondary)" }}
          >
            {step.description}
          </p>
          {step.tip && (
            <div
              className="mt-3 px-3 py-2 rounded-lg text-xs"
              style={{
                backgroundColor: "var(--color-accent-bg)",
                color: "var(--color-accent)",
              }}
            >
              {step.tip}
            </div>
          )}
        </div>

        {/* 페이지 인디케이터 */}
        <div className="flex justify-center gap-1.5">
          {steps.map((_, index) => (
            <button
              key={index}
              onClick={() => setCurrentStep(index)}
              className="h-1.5 rounded-full transition-all"
              style={{
                backgroundColor:
                  index === currentStep
                    ? "var(--color-accent)"
                    : "var(--color-border)",
                width: index === currentStep ? "16px" : "6px",
              }}
              aria-label={`${index + 1}단계로 이동`}
            />
          ))}
        </div>

        {/* 단계 카운터 */}
        <p className="text-center text-xs" style={{ color: "var(--color-text-muted)" }}>
          {currentStep + 1} / {steps.length}
        </p>

        {/* 버튼 */}
        <div className="flex gap-2 pt-1">
          {currentStep > 0 ? (
            <Button variant="ghost" size="sm" onClick={goToPrev} className="flex-1">
              이전
            </Button>
          ) : (
            <Button variant="ghost" size="sm" onClick={onSkip} className="flex-1">
              건너뛰기
            </Button>
          )}
          <Button variant="primary" size="sm" onClick={goToNext} className="flex-1">
            {isLastStep ? "시작하기" : "다음"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
