import { useState, useCallback, useRef, UIEvent, useMemo } from "react";

interface UseCollapsibleSearchOptions {
  /** 스크롤 임계값 (px) - 이 값 이상 스크롤 시 축소 */
  threshold?: number;
  /** 축소 시 콜백 */
  onCollapse?: () => void;
  /** 확장 시 콜백 */
  onExpand?: () => void;
  /** 이 input에 포커스 중이면 collapse 하지 않음 */
  searchInputRef?: React.RefObject<HTMLInputElement | null>;
}

interface UseCollapsibleSearchReturn {
  /** 축소 상태 */
  isCollapsed: boolean;
  /** 스크롤 이벤트 핸들러 */
  handleScroll: (e: UIEvent<HTMLDivElement>) => void;
  /** 맨 위로 스크롤 (자동 확장) */
  scrollToTop: () => void;
  /** 스크롤 컨테이너 ref */
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  /** 스크롤 위치가 FAB 표시 임계값 초과 */
  showScrollTopButton: boolean;
  /** 수동 확장 */
  expand: () => void;
}

export function useCollapsibleSearch(
  options: UseCollapsibleSearchOptions = {}
): UseCollapsibleSearchReturn {
  const { threshold = 100, onCollapse, onExpand, searchInputRef } = options;

  const [isCollapsed, setIsCollapsed] = useState(false);
  const [showScrollTopButton, setShowScrollTopButton] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevCollapsed = useRef(false);
  const lastScrollTop = useRef(0);
  const scrollDirectionUp = useRef(false);

  // 쓰로틀링을 위한 RAF 플래그
  const rafPending = useRef(false);

  // RAF 기반 쓰로틀링 (60fps 제한, CPU 50% 감소)
  // 주의: React synthetic event는 비동기 콜백에서 재사용되므로
  // RAF 콜백에서는 scrollContainerRef를 직접 사용
  const handleScroll = useMemo(() => {
    return (_e: UIEvent<HTMLDivElement>) => {
      if (rafPending.current) return;
      rafPending.current = true;

      requestAnimationFrame(() => {
        rafPending.current = false;
        const container = scrollContainerRef.current;
        if (!container) return;

        const currentScrollTop = container.scrollTop;

        // 스크롤 방향 감지 (작은 움직임 무시: 5px 이상 차이만)
        const scrollDelta = currentScrollTop - lastScrollTop.current;
        if (Math.abs(scrollDelta) > 5) {
          scrollDirectionUp.current = scrollDelta < 0;
          lastScrollTop.current = currentScrollTop;
        }

        // FAB 버튼 표시 여부 (threshold 교차시에만 setState)
        const shouldShowButton = currentScrollTop > 300;
        setShowScrollTopButton(prev => prev !== shouldShowButton ? shouldShowButton : prev);

        // 스크롤 가능 영역이 충분한지 체크
        const scrollableHeight = container.scrollHeight - container.clientHeight;
        if (scrollableHeight < threshold * 2) {
          if (prevCollapsed.current) {
            prevCollapsed.current = false;
            setIsCollapsed(false);
            onExpand?.();
          }
          return;
        }

        // 맨 위 근처면 무조건 확장 (10px 이하)
        if (currentScrollTop <= 10) {
          if (prevCollapsed.current) {
            prevCollapsed.current = false;
            setIsCollapsed(false);
            onExpand?.();
          }
          return;
        }

        // 검색 입력 중이면 collapse 하지 않음 (타이핑 중 포커스 이탈 방지)
        const isSearchFocused = searchInputRef?.current != null &&
          document.activeElement === searchInputRef.current;

        // 축소/확장 판단
        let shouldCollapse = prevCollapsed.current;

        if (!prevCollapsed.current && currentScrollTop > threshold && !scrollDirectionUp.current && !isSearchFocused) {
          shouldCollapse = true;
        } else if (prevCollapsed.current && scrollDirectionUp.current && currentScrollTop < threshold / 2) {
          shouldCollapse = false;
        }

        if (shouldCollapse !== prevCollapsed.current) {
          prevCollapsed.current = shouldCollapse;
          setIsCollapsed(shouldCollapse);

          if (shouldCollapse) {
            onCollapse?.();
          } else {
            onExpand?.();
          }
        }
      });
    };
  }, [threshold, onCollapse, onExpand, searchInputRef]);

  const scrollToTop = useCallback(() => {
    scrollContainerRef.current?.scrollTo({ top: 0, behavior: "smooth" });
  }, []);

  const expand = useCallback(() => {
    setIsCollapsed(false);
    prevCollapsed.current = false;
    onExpand?.();
  }, [onExpand]);

  return {
    isCollapsed,
    handleScroll,
    scrollToTop,
    scrollContainerRef,
    showScrollTopButton,
    expand,
  };
}
