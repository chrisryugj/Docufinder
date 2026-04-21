//! 주기 sync (v2.5.2)
//!
//! 전체 드라이브 감시에서 Windows `ReadDirectoryChangesW` 버퍼 오버플로로
//! notify 이벤트가 누락되는 경우를 보완. 10분 주기 + 창 포커스 복귀 시
//! `sync_folder` 를 실행해 실제 파일 시스템과 DB 를 재정합한다.
//!
//! 동작 원칙:
//! - 배치/벡터 인덱싱 중에는 skip (DB 락 경쟁 방지)
//! - 실행 전 watcher pause, 완료 후 resume (증분 이벤트와의 충돌 회피)
//! - 대용량 드라이브 sync 는 `spawn_blocking` 내부에서 처리되므로 UI freeze 없음
//! - 앱 종료 시 `sync_shutdown` 플래그로 루프 즉시 탈출

use crate::application::container::AppContainer;
use crate::db;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// 전역 sync 실행 lock — periodic_sync(interval + focus) 끼리의 중첩 실행 차단.
///
/// startup sync 는 별도 경로이지만 `is_busy()` 의 watcher pause_count 체크로 차단된다.
/// RAII guard (SyncGuard) 로 패닉/early-return 시에도 반드시 해제됨을 보장.
static SYNC_RUNNING: AtomicBool = AtomicBool::new(false);

struct SyncGuard;
impl Drop for SyncGuard {
    fn drop(&mut self) {
        SYNC_RUNNING.store(false, Ordering::Release);
    }
}

/// 주기 sync 변경분 요약 (프론트 알림용)
#[derive(serde::Serialize, Clone)]
pub struct PeriodicSyncEvent {
    pub trigger: &'static str, // "interval" | "focus"
    pub folders_synced: usize,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn is_busy(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
        return true;
    };
    let Ok(container) = state.read() else {
        return true;
    };
    if container.is_indexing_busy() {
        return true;
    }
    // startup sync / 외부 pause 감지 — pause 중이면 다른 sync 경로가 이미 DB 를 잡고 있다.
    // v2.5.2 에서 sync_folder 중복 실행으로 C:\ 를 3중으로 파싱하던 race 를 차단한다.
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(wm) = wm.read() {
            if wm.is_paused() {
                return true;
            }
        }
    }
    false
}

fn fetch_watched_folders(app: &AppHandle) -> Vec<PathBuf> {
    let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
        return Vec::new();
    };
    let Ok(container) = state.read() else {
        return Vec::new();
    };
    let Ok(conn) = db::get_connection(&container.db_path) else {
        return Vec::new();
    };
    let Ok(folders) = db::get_watched_folders(&conn) else {
        return Vec::new();
    };
    folders
        .into_iter()
        .filter(|f| Path::new(f).exists())
        .map(PathBuf::from)
        .collect()
}

fn pause_watching_inner(app: &AppHandle) {
    let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
        return;
    };
    let Ok(container) = state.read() else { return };
    if let Ok(wm) = container.get_watch_manager() {
        if let Ok(mut wm) = wm.write() {
            wm.pause();
        }
    }
}

fn resume_watching_inner(app: &AppHandle, db_path: &Path) {
    crate::db::wal_checkpoint(db_path);
    let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
        return;
    };
    let Ok(container) = state.read() else { return };
    let Ok(wm) = container.get_watch_manager() else {
        return;
    };
    let Ok(mut wm) = wm.write() else { return };
    let Ok(conn) = crate::db::get_connection(db_path) else {
        return;
    };
    let Ok(folders) = crate::db::get_watched_folders(&conn) else {
        return;
    };
    let existing: Vec<String> = folders
        .into_iter()
        .filter(|f| Path::new(f).exists())
        .collect();
    wm.resume_with_folders(&existing);
}

/// 모든 감시 폴더 대상 sync 실행.
///
/// `trigger` 는 "interval"(주기) 또는 "focus"(창 포커스 복귀) — 로깅/이벤트 payload 용.
pub async fn run_sync_all(app: AppHandle, trigger: &'static str) {
    // 전역 lock: 이미 다른 periodic_sync 가 실행 중이면 skip.
    // compare_exchange 로 진입 직전에 false→true 전환, Drop 시 해제.
    if SYNC_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::debug!(
            "[periodic_sync] skip: already running (trigger={})",
            trigger
        );
        return;
    }
    let _guard = SyncGuard;

    if is_busy(&app) {
        tracing::debug!("[periodic_sync] skip: indexing busy (trigger={})", trigger);
        return;
    }

    let folders = fetch_watched_folders(&app);
    if folders.is_empty() {
        return;
    }

    // 설정 + 서비스 추출 (lock 범위 최소화)
    let (service, include_subfolders, max_file_size_mb, exclude_dirs, db_path, shutdown_flag) = {
        let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
            return;
        };
        let Ok(container) = state.read() else {
            return;
        };
        let settings = container.get_settings();
        let mut dirs: Vec<String> = crate::constants::DEFAULT_EXCLUDED_DIRS
            .iter()
            .map(|s| s.to_string())
            .collect();
        dirs.extend(settings.exclude_dirs.clone());
        (
            container.index_service(),
            settings.include_subfolders,
            settings.max_file_size_mb,
            dirs,
            container.db_path.clone(),
            container.sync_shutdown_flag(),
        )
    };

    tracing::info!(
        "[periodic_sync] start: {} folders (trigger={})",
        folders.len(),
        trigger
    );

    pause_watching_inner(&app);

    let mut total_added = 0usize;
    let mut total_modified = 0usize;
    let mut total_deleted = 0usize;
    let mut synced_count = 0usize;

    for folder in &folders {
        if shutdown_flag.load(Ordering::Acquire) {
            tracing::info!("[periodic_sync] aborted by shutdown signal");
            break;
        }
        match service
            .sync_folder(
                folder,
                include_subfolders,
                None,
                max_file_size_mb,
                exclude_dirs.clone(),
            )
            .await
        {
            Ok(result) => {
                if result.added + result.modified + result.deleted > 0 {
                    tracing::info!(
                        "[periodic_sync] {:?}: +{} ~{} -{}",
                        folder,
                        result.added,
                        result.modified,
                        result.deleted
                    );
                }
                total_added += result.added;
                total_modified += result.modified;
                total_deleted += result.deleted;
                synced_count += 1;
                // DB 의 last_synced_at 갱신 (폴더 카드 UI 에서 쓰임)
                if let Ok(conn) = db::get_connection(&db_path) {
                    let path_str = folder.to_string_lossy().to_string();
                    let _ = db::update_last_synced_at(&conn, &path_str);
                }
            }
            Err(e) => {
                tracing::warn!("[periodic_sync] {:?} failed: {}", folder, e);
            }
        }
    }

    resume_watching_inner(&app, &db_path);

    // last_sync_at 기록 (성공/실패 무관 — 재시도 스팸 방지)
    if let Some(state) = app.try_state::<RwLock<AppContainer>>() {
        if let Ok(container) = state.read() {
            container.mark_sync_done(now_ts());
        }
    }

    let had_changes = total_added + total_modified + total_deleted > 0;
    if had_changes {
        // 파일명 캐시는 sync 가 파일을 추가/삭제했을 때만 재로드
        if let Some(state) = app.try_state::<RwLock<AppContainer>>() {
            if let Ok(container) = state.read() {
                let _ = container.load_filename_cache();
            }
        }
    }

    let _ = app.emit(
        "periodic-sync-updated",
        PeriodicSyncEvent {
            trigger,
            folders_synced: synced_count,
            added: total_added,
            modified: total_modified,
            deleted: total_deleted,
        },
    );

    tracing::info!(
        "[periodic_sync] done: {} folders, +{} ~{} -{} (trigger={})",
        synced_count,
        total_added,
        total_modified,
        total_deleted,
        trigger
    );
}

/// 백그라운드 주기 sync task — lib.rs setup 에서 1회만 spawn.
///
/// 매 분 tick 후 다음 조건 모두 만족 시 sync 실행:
/// - `auto_sync_interval_minutes` > 0
/// - 마지막 sync 로부터 interval 이상 경과
/// - 배치/벡터 인덱싱 중 아님
pub fn spawn_periodic_sync_task(app: AppHandle) {
    let shutdown = {
        let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
            tracing::warn!("[periodic_sync] no container, skip spawn");
            return;
        };
        let Ok(container) = state.read() else {
            return;
        };
        container.sync_shutdown_flag()
    };

    tauri::async_runtime::spawn(async move {
        // 부팅 직후 startup sync 와 충돌하지 않도록 60초 grace.
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut tick = tokio::time::interval(Duration::from_secs(60));
        // interval 의 첫 tick 은 즉시 반환되므로 버림 (grace 이후 즉시 실행 방지).
        tick.tick().await;

        loop {
            tick.tick().await;
            if shutdown.load(Ordering::Acquire) {
                tracing::info!("[periodic_sync] shutdown, loop exit");
                break;
            }

            let (interval_min, last_ts) = {
                let Some(state) = app.try_state::<RwLock<AppContainer>>() else {
                    continue;
                };
                let Ok(container) = state.read() else {
                    continue;
                };
                let settings = container.get_settings();
                (
                    settings.auto_sync_interval_minutes,
                    container.get_last_sync_at(),
                )
            };

            if interval_min == 0 {
                continue; // 사용자가 끔
            }

            let now = now_ts();
            let elapsed = now.saturating_sub(last_ts);
            if elapsed < (interval_min as i64) * 60 {
                continue;
            }

            run_sync_all(app.clone(), "interval").await;
        }
    });
}

/// 프론트 포커스 복귀 시 호출되는 커맨드.
///
/// `min_elapsed_secs` 이상 경과 + 인덱싱 idle 일 때만 백그라운드 sync 실행.
/// 응답은 즉시 반환 (프론트는 sync 완료를 기다리지 않음).
#[tauri::command]
pub async fn trigger_sync_if_stale(
    app: AppHandle,
    min_elapsed_secs: i64,
) -> Result<bool, crate::ApiError> {
    let (last_ts, interval_min) = {
        let state = app.state::<RwLock<AppContainer>>();
        let container = state.read()?;
        (
            container.get_last_sync_at(),
            container.get_settings().auto_sync_interval_minutes,
        )
    };

    if interval_min == 0 {
        return Ok(false);
    }

    // 과도 호출 방어 (60초 floor)
    let threshold = min_elapsed_secs.max(60);
    let elapsed = now_ts().saturating_sub(last_ts);
    if elapsed < threshold {
        return Ok(false);
    }

    if is_busy(&app) {
        return Ok(false);
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        run_sync_all(app_clone, "focus").await;
    });

    Ok(true)
}
