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
}

const steps: Step[] = [
  {
    title: "폴더 추가",
    description: "검색할 폴더를 추가하세요. 사이드바 상단의 '+' 버튼을 클릭하거나, 폴더를 앱으로 드래그하면 됩니다. 추가된 폴더의 문서가 자동으로 인덱싱됩니다.",
    icon: (
      <svg className="w-16 h-16" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
      </svg>
    ),
  },
  {
    title: "문서 검색",
    description: "검색창에 찾고자 하는 내용을 입력하세요. 키워드 검색과 의미 기반 검색이 결합된 하이브리드 검색으로 관련 문서를 찾아드립니다.",
    icon: (
      <svg className="w-16 h-16" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
    ),
  },
  {
    title: "검색 모드 전환",
    description: "상단의 토글로 '하이브리드' 검색과 '파일명' 검색을 전환할 수 있습니다. 파일명만 빠르게 찾고 싶을 때는 파일명 모드를 사용하세요.",
    icon: (
      <svg className="w-16 h-16" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
      </svg>
    ),
  },
  {
    title: "시작할 준비가 되었습니다!",
    description: "이제 Anything을 사용할 준비가 완료되었습니다. 폴더를 추가하고 문서를 검색해보세요. 도움말은 우측 상단의 '?' 버튼에서 언제든 확인할 수 있습니다.",
    icon: (
      <svg className="w-16 h-16" fill="none" viewBox="0 0 24 24" stroke="currentColor">
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
      <div className="space-y-6">
        {/* 아이콘 및 설명 */}
        <div className="flex flex-col items-center text-center py-6">
          <div
            className="w-24 h-24 rounded-2xl flex items-center justify-center mb-6"
            style={{
              backgroundColor: "var(--color-accent-light)",
              color: "var(--color-accent)",
            }}
          >
            {step.icon}
          </div>
          <h3
            className="text-xl font-semibold mb-3"
            style={{ color: "var(--color-text-primary)" }}
          >
            {step.title}
          </h3>
          <p
            className="text-sm leading-relaxed max-w-md"
            style={{ color: "var(--color-text-secondary)" }}
          >
            {step.description}
          </p>
        </div>

        {/* 페이지 인디케이터 */}
        <div className="flex justify-center gap-2">
          {steps.map((_, index) => (
            <button
              key={index}
              onClick={() => setCurrentStep(index)}
              className="w-2.5 h-2.5 rounded-full transition-all"
              style={{
                backgroundColor:
                  index === currentStep
                    ? "var(--color-accent)"
                    : "var(--color-border)",
              }}
              aria-label={`${index + 1}단계로 이동`}
            />
          ))}
        </div>

        {/* 버튼 */}
        <div className="flex gap-3 pt-2">
          {currentStep > 0 ? (
            <Button variant="ghost" onClick={goToPrev} className="flex-1">
              이전
            </Button>
          ) : (
            <Button variant="ghost" onClick={onSkip} className="flex-1">
              건너뛰기
            </Button>
          )}
          <Button variant="primary" onClick={goToNext} className="flex-1">
            {isLastStep ? "시작하기" : "다음"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
