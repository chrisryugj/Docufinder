import { useEffect, useRef } from "react";
import { useIMEComposition } from "./useIMEComposition";

interface UseSearchInputOptions {
  query: string;
  onQueryChange: (query: string) => void;
  onCompositionStart?: () => void;
  onCompositionEnd?: (finalValue: string) => void;
  forwardedRef: React.ForwardedRef<HTMLInputElement>;
}

/**
 * SearchBar/CompactSearchBar 공통 입력 로직:
 * - IME 핸들링
 * - ref 병합 (forwardRef + innerRef)
 * - 외부 query prop ↔ input value 동기화
 */
export function useSearchInput({
  query,
  onQueryChange,
  onCompositionStart,
  onCompositionEnd,
  forwardedRef,
}: UseSearchInputOptions) {
  const innerRef = useRef<HTMLInputElement>(null);

  const { imeHandlers } = useIMEComposition({
    query,
    onQueryChange,
    onCompositionStart,
    onCompositionEnd,
    inputRef: innerRef,
  });

  // ref 병합 (외부 ref + 내부 innerRef)
  useEffect(() => {
    if (!forwardedRef) return;
    if (typeof forwardedRef === "function") {
      forwardedRef(innerRef.current);
    } else {
      forwardedRef.current = innerRef.current;
    }
    return () => {
      if (typeof forwardedRef === "function") {
        forwardedRef(null);
      } else if (forwardedRef) {
        forwardedRef.current = null;
      }
    };
  }, [forwardedRef]);

  // 외부 query prop → input value 동기화
  useEffect(() => {
    if (innerRef.current && innerRef.current.value !== query) {
      innerRef.current.value = query;
    }
  }, [query]);

  return { innerRef, imeHandlers };
}
