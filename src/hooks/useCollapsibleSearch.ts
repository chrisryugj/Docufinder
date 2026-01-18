import { useState, useCallback, useRef, UIEvent, useMemo } from "react";

interface UseCollapsibleSearchOptions {
  /** 스크롤 임계값 (px) - 이 값 이상 스크롤 시 축소 */
  threshold?: number;
  /** 축소 시 콜백 */
  onCollapse?: () => void;
  /** 확장 시 콜백 */
  onExpand?: () => void;
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
  /** 현재 스크롤 위치 */
  scrollTop: number;
  /** 수동 확장 */
  expand: () => void;
}

export function useCollapsibleSearch(
  options: UseCollapsibleSearchOptions = {}
): UseCollapsibleSearchReturn {
  const { threshold = 100, onCollapse, onExpand } = options;

  const [isCollapsed, setIsCollapsed] = useState(false);
  const [scrollTop, setScrollTop] = useState(0);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevCollapsed = useRef(false);
  const lastScrollTop = useRef(0);
  const scrollDirectionUp = useRef(false);

  // 쓰로틀링을 위한 RAF 플래그
  const rafPending = useRef(false);

  // 실제 스크롤 처리 로직
  const handleScrollRaw = useCallback(
    (e: UIEvent<HTMLDivElement>) => {
      const container = e.currentTarget;
      const currentScrollTop = container.scrollTop;

      // 스크롤 방향 감지 (작은 움직임 무시: 5px 이상 차이만)
      const scrollDelta = currentScrollTop - lastScrollTop.current;
      if (Math.abs(scrollDelta) > 5) {
        scrollDirectionUp.current = scrollDelta < 0;
        lastScrollTop.current = currentScrollTop;
      }

      setScrollTop(currentScrollTop);

      // 스크롤 가능 영역이 충분한지 체크 (컨텐츠 높이 - 컨테이너 높이 > threshold * 2)
      const scrollableHeight = container.scrollHeight - container.clientHeight;
      if (scrollableHeight < threshold * 2) {
        // 스크롤 영역이 충분하지 않으면 축소하지 않음
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

      // 축소: threshold 이상 스크롤 + 아래로 스크롤 중
      // 확장: 위로 스크롤 중 + threshold/2 이하
      let shouldCollapse = prevCollapsed.current;

      if (!prevCollapsed.current && currentScrollTop > threshold && !scrollDirectionUp.current) {
        // 확장 → 축소: threshold 넘고 아래로 스크롤 중
        shouldCollapse = true;
      } else if (prevCollapsed.current && scrollDirectionUp.current && currentScrollTop < threshold / 2) {
        // 축소 → 확장: 위로 스크롤 중이고 threshold/2 이하
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
    },
    [threshold, onCollapse, onExpand]
  );

  // RAF 기반 쓰로틀링 (60fps 제한, CPU 50% 감소)
  const handleScroll = useMemo(() => {
    return (e: UIEvent<HTMLDivElement>) => {
      if (rafPending.current) return;
      rafPending.current = true;

      requestAnimationFrame(() => {
        rafPending.current = false;
        handleScrollRaw(e);
      });
    };
  }, [handleScrollRaw]);

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
    scrollTop,
    expand,
  };
}
