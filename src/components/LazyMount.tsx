import { Suspense, useState, type ReactNode } from "react";

interface LazyMountProps {
  /** 모달 열림 여부. 처음 true가 되면 그 뒤로 계속 마운트를 유지한다. */
  active: boolean;
  /** lazy 청크 로딩 중 표시할 폴백 (기본 null). */
  fallback?: ReactNode;
  children: ReactNode;
}

/**
 * lazy 모달용 마운트 래퍼.
 * active가 처음 true가 되기 전까지 children을 마운트하지 않아 lazy 청크 로딩을 지연한다.
 * 한 번 열린 뒤에는 계속 마운트 상태를 유지하므로, isOpen=false로 닫힐 때
 * 모달 내부의 exit 애니메이션이 보존된다(즉시 언마운트되지 않음).
 */
export function LazyMount({ active, fallback = null, children }: LazyMountProps) {
  const [mounted, setMounted] = useState(active);
  // "이전 렌더 정보 저장" 패턴 — 조건부 setState라 즉시 수렴(추가 커밋 없음)
  if (active && !mounted) setMounted(true);
  if (!mounted) return null;
  return <Suspense fallback={fallback}>{children}</Suspense>;
}
