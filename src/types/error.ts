/**
 * API 에러 타입 정의
 *
 * 백엔드 ApiError enum과 동기화됨
 */

/** API 에러 코드 */
export type ApiErrorCode =
  // 파일 시스템
  | "PathNotFound"
  | "AccessDenied"
  | "InvalidPath"
  // 데이터베이스
  | "DatabaseConnection"
  | "DatabaseQuery"
  // 인덱싱
  | "IndexingFailed"
  | "IndexingCancelled"
  // 검색
  | "SearchFailed"
  | "EmbeddingFailed"
  | "VectorIndexEmpty"
  | "VectorIndexCorrupted"
  | "SemanticSearchDisabled"
  // 설정
  | "SettingsLoad"
  | "SettingsSave"
  // 내부
  | "LockFailed"
  | "TaskJoinError"
  | "ModelNotFound";

/** API 에러 객체 */
export interface ApiError {
  code: ApiErrorCode;
  message: string;
}

/**
 * 객체가 ApiError인지 확인
 */
export function isApiError(err: unknown): err is ApiError {
  return (
    typeof err === "object" &&
    err !== null &&
    "code" in err &&
    "message" in err &&
    typeof (err as ApiError).code === "string" &&
    typeof (err as ApiError).message === "string"
  );
}

/**
 * 에러에서 사용자 친화적 메시지 추출
 */
/** 내부 에러 코드에 대한 사용자 친화적 메시지 */
const SANITIZED_MESSAGES: Partial<Record<ApiErrorCode, string>> = {
  DatabaseConnection: "데이터베이스 연결에 실패했습니다",
  DatabaseQuery: "데이터베이스 처리 중 오류가 발생했습니다",
  LockFailed: "내부 처리 중 오류가 발생했습니다",
  TaskJoinError: "작업 처리 중 오류가 발생했습니다",
};

export function getErrorMessage(err: unknown): string {
  if (isApiError(err)) {
    return SANITIZED_MESSAGES[err.code] ?? err.message;
  }
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === "string") {
    return err;
  }
  return "알 수 없는 오류가 발생했습니다";
}

/**
 * 에러 코드별 카테고리
 */
export function getErrorCategory(code: ApiErrorCode): "filesystem" | "database" | "indexing" | "search" | "settings" | "internal" {
  switch (code) {
    case "PathNotFound":
    case "AccessDenied":
    case "InvalidPath":
      return "filesystem";
    case "DatabaseConnection":
    case "DatabaseQuery":
      return "database";
    case "IndexingFailed":
    case "IndexingCancelled":
      return "indexing";
    case "SearchFailed":
    case "EmbeddingFailed":
    case "VectorIndexEmpty":
    case "VectorIndexCorrupted":
    case "SemanticSearchDisabled":
      return "search";
    case "SettingsLoad":
    case "SettingsSave":
      return "settings";
    default:
      return "internal";
  }
}
