//! IndexService - 인덱싱 비즈니스 로직
//!
//! 파일 인덱싱 (FTS, 벡터), 진행률 관리, 취소 처리 등

use crate::application::dto::indexing::IndexStatus;
use crate::application::errors::{AppError, AppResult};
use crate::constants::BLOCKED_PATH_PATTERNS;
use crate::db;
use crate::indexer::pipeline::{
    self, FolderIndexResult, FtsProgressCallback, MetadataScanProgress, MetadataScanResult,
};
use crate::indexer::vector_worker::{VectorIndexingStatus, VectorProgressCallback, VectorWorker};
use crate::search::vector::VectorIndex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// 메타데이터 스캔 콜백 타입
pub type MetadataProgressCallback = Box<dyn Fn(MetadataScanProgress) + Send + Sync>;

/// 인덱싱 서비스
pub struct IndexService {
    db_path: PathBuf,
    embedder: Option<Arc<crate::embedder::Embedder>>,
    vector_index: Option<Arc<VectorIndex>>,
    vector_worker: Arc<RwLock<VectorWorker>>,
    cancel_flag: Arc<AtomicBool>,
    ocr_engine: Option<Arc<crate::ocr::OcrEngine>>,
}

impl IndexService {
    /// 새 IndexService 생성
    pub fn new(
        db_path: PathBuf,
        embedder: Option<Arc<crate::embedder::Embedder>>,
        vector_index: Option<Arc<VectorIndex>>,
        vector_worker: Arc<RwLock<VectorWorker>>,
        cancel_flag: Arc<AtomicBool>,
        ocr_engine: Option<Arc<crate::ocr::OcrEngine>>,
    ) -> Self {
        Self {
            db_path,
            embedder,
            vector_index,
            vector_worker,
            cancel_flag,
            ocr_engine,
        }
    }

    /// 폴더 FTS 인덱싱 (1단계)
    pub async fn index_folder_fts(
        &self,
        path: &Path,
        include_subfolders: bool,
        progress_callback: Option<FtsProgressCallback>,
        max_file_size_mb: u64,
        excluded_dirs: Vec<String>,
    ) -> AppResult<FolderIndexResult> {
        // 경로 유효성 검증
        self.validate_path(path)?;

        // 취소 플래그 리셋
        self.cancel_flag.store(false, Ordering::Relaxed);

        let conn = self.get_connection()?;
        let path_buf = path.to_path_buf();
        let cancel_flag = self.cancel_flag.clone();
        let ocr_engine = self.ocr_engine.clone();

        // blocking 작업으로 실행
        let result = tokio::task::spawn_blocking(move || {
            pipeline::index_folder_fts_only(
                &conn,
                &path_buf,
                include_subfolders,
                cancel_flag,
                progress_callback,
                max_file_size_mb,
                &excluded_dirs,
                ocr_engine,
            )
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join failed: {}", e)))?
        .map_err(|e| AppError::IndexingFailed(e.to_string()))?;

        Ok(result)
    }

    /// 폴더 FTS 인덱싱 재개 (이미 인덱싱된 파일 스킵)
    pub async fn resume_folder_fts(
        &self,
        path: &Path,
        include_subfolders: bool,
        progress_callback: Option<FtsProgressCallback>,
        max_file_size_mb: u64,
        excluded_dirs: Vec<String>,
    ) -> AppResult<FolderIndexResult> {
        self.validate_path(path)?;
        self.cancel_flag.store(false, Ordering::Relaxed);

        let conn = self.get_connection()?;
        let path_buf = path.to_path_buf();
        let cancel_flag = self.cancel_flag.clone();
        let ocr_engine = self.ocr_engine.clone();

        let result = tokio::task::spawn_blocking(move || {
            pipeline::resume_folder_fts(
                &conn,
                &path_buf,
                include_subfolders,
                cancel_flag,
                progress_callback,
                max_file_size_mb,
                &excluded_dirs,
                ocr_engine,
            )
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join failed: {}", e)))?
        .map_err(|e| AppError::IndexingFailed(e.to_string()))?;

        Ok(result)
    }

    /// 폴더 동기화 (변경분만 인덱싱: 추가/수정/삭제)
    pub async fn sync_folder(
        &self,
        path: &Path,
        include_subfolders: bool,
        progress_callback: Option<FtsProgressCallback>,
        max_file_size_mb: u64,
        excluded_dirs: Vec<String>,
    ) -> AppResult<pipeline::SyncResult> {
        self.validate_path(path)?;
        self.cancel_flag.store(false, Ordering::Relaxed);

        let conn = self.get_connection()?;
        let path_buf = path.to_path_buf();
        let cancel_flag = self.cancel_flag.clone();
        let ocr_engine = self.ocr_engine.clone();

        let result = tokio::task::spawn_blocking(move || {
            pipeline::sync_folder_fts(
                &conn,
                &path_buf,
                include_subfolders,
                cancel_flag,
                progress_callback,
                max_file_size_mb,
                &excluded_dirs,
                ocr_engine,
            )
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join failed: {}", e)))?
        .map_err(|e| AppError::IndexingFailed(e.to_string()))?;

        Ok(result)
    }

    /// 메타데이터 전용 스캔 (파일 열지 않음, < 2초 목표)
    /// 파일명 검색을 위한 빠른 스캔
    pub async fn scan_metadata_only(
        &self,
        path: &Path,
        include_subfolders: bool,
        progress_callback: Option<MetadataProgressCallback>,
        max_file_size_mb: u64,
        excluded_dirs: Vec<String>,
    ) -> AppResult<MetadataScanResult> {
        self.validate_path(path)?;
        self.cancel_flag.store(false, Ordering::Relaxed);

        let conn = self.get_connection()?;
        let path_buf = path.to_path_buf();
        let cancel_flag = self.cancel_flag.clone();

        let result = tokio::task::spawn_blocking(move || {
            pipeline::scan_metadata_only(
                &conn,
                &path_buf,
                include_subfolders,
                cancel_flag,
                progress_callback,
                max_file_size_mb,
                &excluded_dirs,
            )
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join failed: {}", e)))?
        .map_err(|e| AppError::IndexingFailed(e.to_string()))?;

        Ok(result)
    }

    /// 벡터 인덱싱 시작 (2단계, 백그라운드)
    pub fn start_vector_indexing(
        &self,
        progress_callback: Option<VectorProgressCallback>,
        intensity: Option<crate::commands::settings::IndexingIntensity>,
    ) -> AppResult<bool> {
        let embedder = self
            .embedder
            .as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;
        let vector_index = self
            .vector_index
            .as_ref()
            .ok_or(AppError::SemanticSearchDisabled)?;

        let mut worker = self
            .vector_worker
            .write()
            .map_err(|e| AppError::Internal(format!("VectorWorker lock failed: {}", e)))?;

        if worker.is_running() {
            return Ok(false);
        }

        worker
            .start(
                self.db_path.clone(),
                embedder.clone(),
                vector_index.clone(),
                progress_callback,
                intensity,
            )
            .map_err(|e| AppError::IndexingFailed(e.to_string()))?;

        Ok(true)
    }

    /// 인덱싱 취소
    pub fn cancel_indexing(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        tracing::info!("Indexing cancelled");
    }

    /// 벡터 인덱싱 취소
    pub fn cancel_vector_indexing(&self) -> AppResult<()> {
        let worker = self
            .vector_worker
            .read()
            .map_err(|e| AppError::Internal(format!("VectorWorker lock failed: {}", e)))?;
        worker.cancel();
        tracing::info!("Vector indexing cancelled");
        Ok(())
    }

    /// 인덱스 상태 조회
    pub async fn get_status(&self) -> AppResult<IndexStatus> {
        let conn = self.get_connection()?;

        let total_files =
            db::get_file_count(&conn).map_err(|e| AppError::Internal(e.to_string()))?;
        let indexed_files =
            db::get_indexed_file_count(&conn).map_err(|e| AppError::Internal(e.to_string()))?;
        let watched_folders =
            db::get_watched_folders(&conn).map_err(|e| AppError::Internal(e.to_string()))?;
        let vectors_count = self.vector_index.as_ref().map(|vi| vi.size()).unwrap_or(0);
        let semantic_available = self.embedder.is_some();

        Ok(IndexStatus {
            total_files,
            indexed_files,
            watched_folders,
            vectors_count,
            semantic_available,
            filename_cache_truncated: false, // get_index_status 커맨드에서 덮어씀
        })
    }

    /// 벡터 인덱싱 상태 조회
    pub fn get_vector_status(&self) -> AppResult<VectorIndexingStatus> {
        let worker = self
            .vector_worker
            .read()
            .map_err(|e| AppError::Internal(format!("VectorWorker lock failed: {}", e)))?;
        let mut status = worker.get_status();

        if !status.is_running {
            // 누적 진행률: 완료된 청크 + 대기 청크 = 전체
            let conn = self.get_connection()?;
            let stats = db::get_vector_indexing_stats(&conn)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            status.pending_chunks = stats.pending_chunks;
            status.total_chunks = stats.completed_chunks + stats.pending_chunks;
            status.processed_chunks = stats.completed_chunks;
        }

        Ok(status)
    }

    /// 폴더 재인덱싱 (기존 데이터 삭제 후 다시)
    pub async fn reindex_folder(
        &self,
        path: &Path,
        include_subfolders: bool,
        progress_callback: Option<FtsProgressCallback>,
        max_file_size_mb: u64,
        excluded_dirs: Vec<String>,
    ) -> AppResult<FolderIndexResult> {
        // 경로 유효성 검증
        self.validate_path(path)?;
        let path_str = path.to_string_lossy().to_string();

        // 1. 벡터 인덱스에서 삭제
        if let Some(vi) = self.vector_index.as_ref() {
            let conn = self.get_connection()?;
            let file_chunk_ids = db::get_file_and_chunk_ids_in_folder(&conn, &path_str)
                .map_err(|e| AppError::Internal(e.to_string()))?;

            for (_file_id, chunk_ids) in file_chunk_ids {
                for chunk_id in chunk_ids {
                    let _ = vi.remove(chunk_id);
                }
            }
            let _ = vi.save();
        }

        // 2. DB에서 삭제
        {
            let conn = self.get_connection()?;
            let deleted = db::delete_files_in_folder(&conn, &path_str)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            tracing::info!("Deleted {} files for reindexing: {}", deleted, path_str);
        }

        // 3. FTS 재인덱싱 (재인덱싱은 메타 스캔 없이 직접 수행)
        self.index_folder_fts(
            path,
            include_subfolders,
            progress_callback,
            max_file_size_mb,
            excluded_dirs,
        )
        .await
    }

    /// 시맨틱 검색 사용 가능 여부
    pub fn is_semantic_available(&self) -> bool {
        self.embedder.is_some() && self.vector_index.is_some()
    }

    /// 감시 폴더 등록 (DB)
    pub fn add_watched_folder(&self, path: &str) -> AppResult<()> {
        let conn = self.get_connection()?;
        db::add_watched_folder(&conn, path)
            .map(|_| ())
            .map_err(|e| AppError::Internal(e.to_string()))
    }

    /// 모든 데이터 클리어 (벡터 + DB) — 한 번에 실행
    pub fn clear_all(&self) -> AppResult<()> {
        self.stop_vector_worker();
        self.clear_vector_index();
        self.clear_database()?;
        Ok(())
    }

    /// 벡터 워커 중지 + 완전 종료 대기
    pub fn stop_vector_worker(&self) {
        if let Ok(mut worker) = self.vector_worker.write() {
            worker.cancel();
            worker.join(); // embed_batch 완료까지 대기
        }
    }

    /// 벡터 인덱스 클리어
    pub fn clear_vector_index(&self) {
        if let Some(vi) = self.vector_index.as_ref() {
            vi.clear();
            let _ = vi.save();
            tracing::info!("Vector index cleared");
        }
    }

    /// DB 초기화 (DROP + re-CREATE, DELETE 대비 수백 배 빠름)
    pub fn clear_database(&self) -> AppResult<()> {
        let conn = self.get_connection()?;
        db::clear_all_data(&conn, &self.db_path).map_err(|e| AppError::Internal(e.to_string()))?;
        tracing::info!("Database cleared");
        Ok(())
    }

    // ============================================
    // Private Helpers
    // ============================================

    fn get_connection(&self) -> AppResult<db::PooledConnection> {
        db::get_connection(&self.db_path)
            .map_err(|e| AppError::Internal(format!("DB connection failed: {}", e)))
    }

    fn validate_path(&self, path: &Path) -> AppResult<()> {
        if !path.exists() {
            return Err(AppError::PathNotFound(path.display().to_string()));
        }

        // 경로 정규화 (심볼릭 링크 해결)
        let canonical = path
            .canonicalize()
            .map_err(|e| AppError::InvalidPath(format!("{}: {}", path.display(), e)))?;

        // 시스템 폴더 블랙리스트 검증
        let path_str = canonical.to_string_lossy().to_lowercase();
        if BLOCKED_PATH_PATTERNS.iter().any(|b| path_str.contains(b)) {
            return Err(AppError::AccessDenied(format!(
                "'{}' is a protected system folder",
                canonical.display()
            )));
        }

        Ok(())
    }
}
