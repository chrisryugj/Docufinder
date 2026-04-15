//! Indexing DTOs - 인덱싱 관련 데이터 전송 객체

use serde::{Deserialize, Serialize};

/// 인덱스 상태 DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStatus {
    /// 전체 파일 수
    pub total_files: usize,
    /// 인덱싱된 파일 수
    pub indexed_files: usize,
    /// 감시 중인 폴더 목록
    pub watched_folders: Vec<String>,
    /// 벡터 인덱스 크기
    pub vectors_count: usize,
    /// 시맨틱 검색 가능 여부
    pub semantic_available: bool,
    /// 파일명 캐시가 상한(MAX_CACHE_ENTRIES)을 초과하여 잘렸는지 여부
    #[serde(default)]
    pub filename_cache_truncated: bool,
}

/// 폴더 추가 결과 DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct AddFolderResult {
    /// 성공 여부
    pub success: bool,
    /// 인덱싱된 파일 수
    pub indexed_count: usize,
    /// 실패한 파일 수
    pub failed_count: usize,
    /// 벡터 인덱스 수
    pub vectors_count: usize,
    /// 결과 메시지
    pub message: String,
    /// 에러 목록
    pub errors: Vec<String>,
    /// OCR로 인덱싱된 이미지 파일 수
    #[serde(default)]
    pub ocr_image_count: usize,
}

impl AddFolderResult {
    /// 성공 결과 생성
    pub fn success(indexed_count: usize, message: String) -> Self {
        Self {
            success: true,
            indexed_count,
            failed_count: 0,
            vectors_count: 0,
            message,
            errors: vec![],
            ocr_image_count: 0,
        }
    }

    /// 취소 결과 생성
    pub fn cancelled() -> Self {
        Self {
            success: false,
            indexed_count: 0,
            failed_count: 0,
            vectors_count: 0,
            message: "인덱싱이 취소되었습니다".to_string(),
            errors: vec![],
            ocr_image_count: 0,
        }
    }
}

/// 폴더 통계 DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct FolderStats {
    /// 전체 파일 수 (메타데이터 포함)
    pub file_count: usize,
    /// FTS 인덱싱된 문서 수
    pub indexed_count: usize,
    /// 마지막 인덱싱 시간 (Unix timestamp)
    pub last_indexed: Option<i64>,
}

/// 감시 폴더 정보 DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct WatchedFolderInfo {
    /// 폴더 경로
    pub path: String,
    /// 즐겨찾기 여부
    pub is_favorite: bool,
    /// 추가 시간 (Unix timestamp)
    pub added_at: Option<i64>,
    /// 인덱싱 상태 ("indexing" | "completed")
    pub indexing_status: String,
}
