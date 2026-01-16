//! 앱 전역 상수 정의

/// 지원하는 파일 확장자 목록
pub const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "md", "hwpx", "docx", "xlsx", "xls", "pdf"];

/// FTS snippet 컨텍스트 토큰 수 (검색 결과 하이라이트 주변 문자 수)
pub const SNIPPET_CONTEXT_TOKENS: i32 = 32;

/// 청크 최대 문자 수 (인덱싱 시 문서 분할 단위)
pub const CHUNK_MAX_CHARS: usize = 1000;

/// 청크 오버랩 문자 수
pub const CHUNK_OVERLAP_CHARS: usize = 200;
