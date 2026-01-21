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
        }
    }
}

/// 폴더 통계 DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct FolderStats {
    /// 파일 수
    pub file_count: usize,
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
}

/// FTS 인덱싱 진행률 DTO
#[derive(Debug, Clone, Serialize)]
pub struct FtsIndexingProgress {
    /// 진행 단계 (scanning, indexing, completed, error, cancelled)
    pub phase: String,
    /// 전체 파일 수
    pub total_files: usize,
    /// 처리된 파일 수
    pub processed_files: usize,
    /// 현재 처리 중인 파일
    pub current_file: Option<String>,
    /// 폴더 경로
    pub folder_path: String,
    /// 에러 메시지
    pub error: Option<String>,
}

/// 벡터 인덱싱 진행률 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct VectorIndexingProgress {
    /// 전체 청크 수
    pub total_chunks: usize,
    /// 처리된 청크 수
    pub processed_chunks: usize,
    /// 현재 처리 중인 파일
    pub current_file: Option<String>,
    /// 완료 여부
    pub is_complete: bool,
}

/// 벡터 인덱싱 상태 DTO
#[derive(Debug, Clone, Serialize, Default)]
pub struct VectorIndexingStatus {
    /// 실행 중 여부
    pub is_running: bool,
    /// 전체 청크 수
    pub total_chunks: usize,
    /// 처리된 청크 수
    pub processed_chunks: usize,
}
