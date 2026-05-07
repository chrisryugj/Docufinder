//! FolderService - 폴더 관리 비즈니스 로직
//!
//! 감시 폴더 추가/삭제, 즐겨찾기, 통계 조회 등

use crate::application::dto::indexing::{FolderStats, WatchedFolderInfo};
use crate::application::errors::{AppError, AppResult};
use crate::db;
use crate::indexer::manager::WatchManager;
use crate::search::vector::VectorIndex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// 폴더 관리 서비스
pub struct FolderService {
    db_path: PathBuf,
    watch_manager: Option<Arc<RwLock<WatchManager>>>,
    vector_index: Option<Arc<VectorIndex>>,
}

impl FolderService {
    /// 새 FolderService 생성
    pub fn new(
        db_path: PathBuf,
        watch_manager: Option<Arc<RwLock<WatchManager>>>,
        vector_index: Option<Arc<VectorIndex>>,
    ) -> Self {
        Self {
            db_path,
            watch_manager,
            vector_index,
        }
    }

    /// 감시 폴더 추가 (DB 등록만)
    pub async fn add_folder(&self, path: &Path) -> AppResult<String> {
        // 경로 유효성 검증
        let canonical = self.validate_and_canonicalize(path)?;
        let path_str = canonical.to_string_lossy().to_string();

        let conn = self.get_connection()?;
        db::add_watched_folder(&conn, &path_str).map_err(|e| AppError::Internal(e.to_string()))?;

        // 파일 감시 시작
        if let Some(wm) = self.watch_manager.as_ref() {
            if let Ok(mut wm) = wm.write() {
                if let Err(e) = wm.watch(&canonical) {
                    tracing::warn!("Failed to start watching {}: {}", path_str, e);
                }
            }
        }

        tracing::info!("Added folder: {}", path_str);
        Ok(path_str)
    }

    /// 감시 폴더 제거
    ///
    /// 순서:
    /// 1) 파일 감시 중지
    /// 2) **`watched_folders` DELETE** — 사용자 UX 기준점. 사이드바에서 즉시 사라져야 한다.
    /// 3) 벡터/파일 정리 — best-effort. 부분 실패해도 폴더 자체는 사라진 상태로 유지.
    ///
    /// 이전 구현은 vi.save() 또는 delete_files_in_folder() 실패 시 watched_folders DELETE 까지
    /// 도달 못 해 "토스트는 제거됨, 재시작하면 부활" 현상이 발생했다 (이슈 #22).
    pub async fn remove_folder(&self, path: &str) -> AppResult<()> {
        let folder_path = Path::new(path);

        if let Some(wm) = self.watch_manager.as_ref() {
            if let Ok(mut wm) = wm.write() {
                let _ = wm.unwatch(folder_path);
            }
        }

        let conn = self.get_connection()?;

        db::remove_watched_folder(&conn, path).map_err(|e| AppError::Internal(e.to_string()))?;
        tracing::info!("watched_folders 삭제 완료: {}", path);

        if let Some(vi) = self.vector_index.as_ref() {
            match db::get_file_and_chunk_ids_in_folder(&conn, path) {
                Ok(file_chunk_ids) => {
                    let mut removed = 0;
                    for (_file_id, chunk_ids) in file_chunk_ids {
                        for chunk_id in chunk_ids {
                            if vi.remove(chunk_id).is_ok() {
                                removed += 1;
                            }
                        }
                    }
                    tracing::info!("벡터 청크 {}개 제거 (폴더 {})", removed, path);
                }
                Err(e) => tracing::warn!(
                    "폴더 제거 중 벡터 청크 조회 실패(best-effort, 폴더는 이미 제거됨): {}",
                    e
                ),
            }
            if let Err(e) = vi.save() {
                tracing::warn!(
                    "폴더 제거 중 벡터 인덱스 저장 실패(best-effort, 폴더는 이미 제거됨): {}",
                    e
                );
            }
        }

        match db::delete_files_in_folder(&conn, path) {
            Ok(deleted) => tracing::info!("파일 행 {}개 삭제 (폴더 {})", deleted, path),
            Err(e) => tracing::warn!(
                "폴더 제거 중 파일 삭제 실패(best-effort, 폴더는 이미 제거됨): {}",
                e
            ),
        }

        Ok(())
    }

    /// 감시 폴더 목록 조회
    pub async fn get_folders(&self) -> AppResult<Vec<String>> {
        let conn = self.get_connection()?;
        db::get_watched_folders(&conn).map_err(|e| AppError::Internal(e.to_string()))
    }

    /// 감시 폴더 상세 목록 조회
    pub async fn get_folders_with_info(&self) -> AppResult<Vec<WatchedFolderInfo>> {
        let conn = self.get_connection()?;
        let folders = db::get_watched_folders_with_info(&conn)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(folders
            .into_iter()
            .map(|f| WatchedFolderInfo {
                path: f.path,
                is_favorite: f.is_favorite,
                added_at: f.added_at,
                indexing_status: f.indexing_status,
            })
            .collect())
    }

    /// 폴더 통계 조회
    pub async fn get_folder_stats(&self, path: &str) -> AppResult<FolderStats> {
        let conn = self.get_connection()?;
        let stats =
            db::get_folder_stats(&conn, path).map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(FolderStats {
            file_count: stats.file_count,
            indexed_count: stats.indexed_count,
            last_indexed: stats.last_indexed,
        })
    }

    /// 전체 폴더 통계 배치 조회 (N+1 IPC 방지)
    pub async fn get_all_folder_stats(&self) -> AppResult<Vec<(String, FolderStats)>> {
        let conn = self.get_connection()?;
        let folders =
            db::get_watched_folders(&conn).map_err(|e| AppError::Internal(e.to_string()))?;

        let mut result = Vec::with_capacity(folders.len());
        for folder in folders {
            let stats = db::get_folder_stats(&conn, &folder)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            result.push((
                folder,
                FolderStats {
                    file_count: stats.file_count,
                    indexed_count: stats.indexed_count,
                    last_indexed: stats.last_indexed,
                },
            ));
        }

        Ok(result)
    }

    /// 즐겨찾기 토글
    pub async fn toggle_favorite(&self, path: &str) -> AppResult<bool> {
        let conn = self.get_connection()?;
        let is_favorite =
            db::toggle_favorite(&conn, path).map_err(|e| AppError::Internal(e.to_string()))?;

        tracing::info!("Toggled favorite for {}: {}", path, is_favorite);
        Ok(is_favorite)
    }

    /// 기존 감시 폴더들 자동 감시 시작
    pub async fn resume_watching(&self) -> AppResult<usize> {
        let conn = self.get_connection()?;
        let folders =
            db::get_watched_folders(&conn).map_err(|e| AppError::Internal(e.to_string()))?;

        let mut resumed = 0;
        if let Some(wm) = self.watch_manager.as_ref() {
            if let Ok(mut wm) = wm.write() {
                for folder in folders {
                    let path = Path::new(&folder);
                    if path.exists() && wm.watch(path).is_ok() {
                        tracing::info!("Resumed watching: {}", folder);
                        resumed += 1;
                    }
                }
            }
        }

        Ok(resumed)
    }

    // ============================================
    // Private Helpers
    // ============================================

    fn get_connection(&self) -> AppResult<db::PooledConnection> {
        db::get_connection(&self.db_path)
            .map_err(|e| AppError::Internal(format!("DB connection failed: {}", e)))
    }

    fn validate_and_canonicalize(&self, path: &Path) -> AppResult<PathBuf> {
        if !path.exists() {
            return Err(AppError::PathNotFound(path.display().to_string()));
        }

        let canonical = dunce::canonicalize(path)
            .map_err(|e| AppError::InvalidPath(format!("{}: {}", path.display(), e)))?;

        // `validate_watch_path`는 `allow_system_folders` 토글을 반영해 진입점 전체에서
        // 일관된 차단/허용 동작을 보장한다.
        crate::constants::validate_watch_path(&canonical)
            .map_err(|msg| AppError::AccessDenied(msg.to_string()))?;

        Ok(canonical)
    }
}
