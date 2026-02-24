import { Component, ErrorInfo, ReactNode } from "react";
import { logToBackend } from "../utils/errorLogger";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

/**
 * 전역 에러 바운더리
 * 렌더링 에러 발생 시 앱 전체 크래시 방지
 */
export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("ErrorBoundary caught an error:", error, errorInfo);
    logToBackend(
      "error",
      error.message,
      error.stack || errorInfo.componentStack || undefined,
      "ErrorBoundary",
    );
  }

  handleReload = () => {
    window.location.reload();
  };

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      // 커스텀 fallback이 있으면 사용
      if (this.props.fallback) {
        return this.props.fallback;
      }

      // 기본 에러 UI
      return (
        <div className="min-h-screen flex items-center justify-center bg-[var(--bg-primary)]">
          <div className="max-w-md w-full mx-4 p-6 bg-[var(--color-bg-secondary)] rounded-lg border border-[var(--color-border)] text-center">
            <div className="text-5xl mb-4">⚠️</div>
            <h1 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
              오류가 발생했습니다
            </h1>
            <p className="text-[var(--color-text-secondary)] mb-4">
              예기치 않은 오류가 발생했습니다. 문제가 지속되면 앱을 다시 시작해 주세요.
            </p>
            {this.state.error && (
              <details className="text-left mb-4 p-3 bg-[var(--color-bg-tertiary)] rounded text-sm">
                <summary className="cursor-pointer text-[var(--color-text-secondary)] hover:text-[var(--text-primary)]">
                  오류 상세 정보
                </summary>
                <pre className="mt-2 text-xs text-red-500 whitespace-pre-wrap break-words">
                  {this.state.error.message}
                </pre>
              </details>
            )}
            <div className="flex gap-3 justify-center">
              <button
                onClick={this.handleReset}
                className="px-4 py-2 bg-[var(--color-bg-tertiary)] text-[var(--text-primary)] rounded hover:bg-[var(--color-bg-tertiary)] transition-colors"
              >
                다시 시도
              </button>
              <button
                onClick={this.handleReload}
                className="px-4 py-2 bg-[var(--color-accent)] text-white rounded hover:bg-[var(--color-accent-hover)] transition-colors"
              >
                앱 새로고침
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
