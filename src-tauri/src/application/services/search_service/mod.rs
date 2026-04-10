//! SearchService - 검색 비즈니스 로직
//!
//! 다양한 검색 모드 (keyword, semantic, hybrid, filename)를 처리하고
//! 결과를 정규화된 DTO로 반환합니다.

pub(crate) mod helpers;
mod hybrid;
mod keyword;
mod semantic;
mod smart;

use crate::application::dto::search::{SearchMode, SearchQuery, SearchResponse};
use crate::application::errors::{AppError, AppResult};
use crate::db;
use crate::search::filename_cache::FilenameCache;
use crate::tokenizer::TextTokenizer;
use std::path::PathBuf;
use std::sync::Arc;

/// 검색 서비스
pub struct SearchService {
    db_path: PathBuf,
    pub(super) embedder: Option<Arc<crate::embedder::Embedder>>,
    pub(super) vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
    pub(super) tokenizer: Option<Arc<dyn TextTokenizer>>,
    pub(super) filename_cache: Option<Arc<FilenameCache>>,
}

impl SearchService {
    /// 새 SearchService 생성
    pub fn new(
        db_path: PathBuf,
        embedder: Option<Arc<crate::embedder::Embedder>>,
        vector_index: Option<Arc<crate::search::vector::VectorIndex>>,
        tokenizer: Option<Arc<dyn TextTokenizer>>,
        filename_cache: Option<Arc<FilenameCache>>,
    ) -> Self {
        Self {
            db_path,
            embedder,
            vector_index,
            tokenizer,
            filename_cache,
        }
    }

    /// 검색 실행 (모드에 따라 분기)
    pub async fn search(&self, query: SearchQuery) -> AppResult<SearchResponse> {
        if query.query.trim().is_empty() {
            return Ok(SearchResponse::empty(self.mode_to_string(query.mode)));
        }

        match query.mode {
            SearchMode::Keyword => {
                self.search_keyword(&query.query, query.max_results, None)
                    .await
            }
            SearchMode::Semantic => {
                self.search_semantic(&query.query, query.max_results, None)
                    .await
            }
            SearchMode::Hybrid => {
                self.search_hybrid(&query.query, query.max_results, None)
                    .await
            }
            SearchMode::Filename => {
                self.search_filename(&query.query, query.max_results, None)
                    .await
            }
        }
    }

    // ── Private Helpers ──────────────────────────────────

    pub(super) fn get_connection(&self) -> AppResult<db::PooledConnection> {
        db::get_connection(&self.db_path)
            .map_err(|e| AppError::Internal(format!("DB connection failed: {}", e)))
    }

    fn mode_to_string(&self, mode: SearchMode) -> &'static str {
        match mode {
            SearchMode::Keyword => "keyword",
            SearchMode::Semantic => "semantic",
            SearchMode::Hybrid => "hybrid",
            SearchMode::Filename => "filename",
        }
    }
}
