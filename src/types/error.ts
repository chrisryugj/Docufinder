/**
 * API 에러 타입 정의
 *
 * 백엔드 ApiError enum과 동기화됨
 */

/** API 에러 코드 (src-tauri/src/error.rs ApiError 변형과 1:1) */
export type ApiErrorCode =
  // 입력 검증
  | "Validation"
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
  | "PasswordProtected"
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
  | "ModelNotFound"
  // AI·외부 명령
  | "AiError"
  | "CommandFailed";

/** API 에러 객체. 백엔드는 serde(tag="code", content="message") 라서 message 는 변형 안의
 *  상세값만 온다 (PathNotFound 는 경로만). 값이 없는 변형(VectorIndexEmpty 등)은 message 가 없다 */
export interface ApiError {
  code: ApiErrorCode;
  message?: string;
}

/**
 * 객체가 ApiError인지 확인
 */
export function isApiError(err: unknown): err is ApiError {
  if (typeof err !== "object" || err === null || !("code" in err)) return false;
  const { code, message } = err as { code: unknown; message?: unknown };
  return typeof code === "string" && (message === undefined || typeof message === "string");
}

function withDetail(summary: string, detail?: string): string {
  const d = detail?.trim();
  return d ? `${summary}: ${d}` : summary;
}

/** 코드별 사용자 문구. 상세값이 문장인 코드(Validation·PasswordProtected·AiError)는 그대로 쓴다 */
const USER_MESSAGES: Record<ApiErrorCode, (detail?: string) => string> = {
  Validation: (d) => d?.trim() || "입력한 값을 확인해 주세요",
  PathNotFound: (d) => withDetail("파일이나 폴더를 찾을 수 없습니다", d),
  AccessDenied: (d) => withDetail("접근 권한이 없습니다", d),
  InvalidPath: (d) => withDetail("경로가 올바르지 않습니다", d),
  // DB 상세는 스키마 노출 방지로 백엔드가 이미 숨긴다
  DatabaseConnection: () => "검색 데이터베이스를 열지 못했습니다. 앱을 다시 시작해 보세요",
  DatabaseQuery: () => "검색 데이터베이스 처리 중 오류가 발생했습니다",
  IndexingFailed: (d) => withDetail("문서를 읽지 못했습니다", d),
  IndexingCancelled: () => "문서 읽기를 취소했습니다",
  PasswordProtected: (d) => d?.trim() || "암호가 걸린 문서입니다. 암호를 입력해 주세요",
  SearchFailed: (d) => withDetail("검색하지 못했습니다", d),
  EmbeddingFailed: (d) => withDetail("AI 검색용 문서 분석에 실패했습니다", d),
  VectorIndexEmpty: () => "AI 검색에 쓸 문서 분석이 아직 없습니다. 분석이 끝난 뒤 다시 시도해 주세요",
  VectorIndexCorrupted: () => "AI 검색 데이터가 손상됐습니다. 문서를 다시 읽혀 주세요",
  SemanticSearchDisabled: () => "AI 검색 모델이 설치되지 않았습니다",
  SettingsLoad: (d) => withDetail("설정을 불러오지 못했습니다", d),
  SettingsSave: (d) => withDetail("설정을 저장하지 못했습니다", d),
  LockFailed: () => "내부 처리 중 오류가 발생했습니다. 잠시 뒤 다시 시도해 주세요",
  TaskJoinError: () => "작업 처리 중 오류가 발생했습니다. 잠시 뒤 다시 시도해 주세요",
  ModelNotFound: () => "필요한 모델 파일을 찾을 수 없습니다. 설정에서 모델을 다시 받아 주세요",
  AiError: (d) => d?.trim() || "AI 응답을 받지 못했습니다",
  CommandFailed: (d) => withDetail("문서 변환 도구를 실행하지 못했습니다", d),
};

/**
 * 에러에서 사용자 친화적 메시지 추출
 */
export function getErrorMessage(err: unknown): string {
  if (isApiError(err)) {
    const toMessage = USER_MESSAGES[err.code];
    return toMessage ? toMessage(err.message) : err.message?.trim() || "알 수 없는 오류가 발생했습니다";
  }
  if (err instanceof Error) {
    if (err.name === "IpcTimeoutError") return "응답이 너무 오래 걸려 중단했습니다. 잠시 뒤 다시 시도해 주세요";
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
