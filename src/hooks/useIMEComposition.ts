import { useCallback, useRef } from "react";

interface UseIMECompositionOptions {
  query: string;
  onQueryChange: (value: string) => void;
  onCompositionStart?: () => void;
  onCompositionEnd?: (finalValue: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  /** Windows 한영전환 IME 초기화 (첫 포커스 시 blur→focus 리셋) */
  enableWindowsIMEInit?: boolean;
}

/**
 * IME 조합 상태 관리 훅
 * - onChange에서 isComposing 감지
 * - onCompositionStart/End 핸들링
 * - onBlur 시 미완료 조합 정리
 * - (선택) Windows IME 초기화
 */
export function useIMEComposition({
  query,
  onQueryChange,
  onCompositionStart: onCompStart,
  onCompositionEnd: onCompEnd,
  inputRef,
  enableWindowsIMEInit = false,
}: UseIMECompositionOptions) {
  const isComposingRef = useRef(false);
  const hasInitializedIME = useRef(false);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = e.target.value;
      const composing = (e.nativeEvent as InputEvent).isComposing === true;
      if (composing && !isComposingRef.current) {
        isComposingRef.current = true;
        onCompStart?.();
      } else if (!composing && isComposingRef.current) {
        isComposingRef.current = false;
        onCompEnd?.(value);
      }
      onQueryChange(value);
    },
    [onQueryChange, onCompStart, onCompEnd]
  );

  const handleCompositionStart = useCallback(() => {
    if (!isComposingRef.current) {
      isComposingRef.current = true;
      onCompStart?.();
    }
  }, [onCompStart]);

  const handleCompositionEnd = useCallback(
    (e: React.CompositionEvent<HTMLInputElement>) => {
      const finalValue = (e.target as HTMLInputElement).value;
      if (isComposingRef.current) {
        isComposingRef.current = false;
      }
      if (finalValue !== query) {
        onQueryChange(finalValue);
      }
      onCompEnd?.(finalValue);
    },
    [query, onQueryChange, onCompEnd]
  );

  const handleBlur = useCallback(() => {
    if (isComposingRef.current) {
      isComposingRef.current = false;
      const value = inputRef.current?.value ?? query;
      if (value !== query) {
        onQueryChange(value);
      }
      onCompEnd?.(value);
    }
  }, [query, onQueryChange, onCompEnd, inputRef]);

  // 첫 포커스 시 IME 초기화 (Windows 한영전환 문제 해결)
  const handleFocus = useCallback(() => {
    if (!enableWindowsIMEInit || hasInitializedIME.current) return;
    hasInitializedIME.current = true;

    const input = inputRef.current;
    if (!input) return;

    // blur 후 최소 딜레이로 다시 focus (Windows IME 리셋)
    input.blur();
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        input.focus();
      });
    });
  }, [enableWindowsIMEInit, inputRef]);

  return {
    isComposingRef,
    imeHandlers: {
      onChange: handleChange,
      onCompositionStart: handleCompositionStart,
      onCompositionEnd: handleCompositionEnd,
      onBlur: handleBlur,
      onFocus: handleFocus,
    },
  };
}
