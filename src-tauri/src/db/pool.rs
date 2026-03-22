use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::Mutex;

// ==================== 커넥션 풀 ====================

/// 커넥션 풀 (최대 4개, Drop 시 자동 반환)
/// 매 쿼리마다 Connection::open + PRAGMA 8개 실행하던 오버헤드를 제거.
/// HDD 환경에서 쿼리당 10-30ms 절감.
/// i3-12100 (4C) 기준 동시 DB 접근은 3-4개면 충분.
///
/// (Option<String>, Vec<Connection>): (현재 DB 경로, 풀 커넥션 목록)
/// DB 경로 변경 시 풀을 drain하고 새 커넥션 생성.
static CONN_POOL: Mutex<(Option<String>, Vec<Connection>)> = Mutex::new((None, Vec::new()));
const MAX_POOL_SIZE: usize = 4;

/// 풀에서 관리되는 DB 커넥션 래퍼
/// Deref<Target=Connection>으로 기존 &Connection API 호환.
/// Drop 시 트랜잭션이 없으면 풀에 자동 반환.
pub struct PooledConnection {
    inner: Option<Connection>,
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.inner.take() {
            // 열린 트랜잭션이 있으면 반환하지 않음 (안전)
            if conn.is_autocommit() {
                let mut pool = CONN_POOL.lock().unwrap_or_else(|e| e.into_inner());
                if pool.1.len() < MAX_POOL_SIZE {
                    pool.1.push(conn);
                }
            }
            // 풀이 가득 차거나 트랜잭션 중이면 그냥 drop
        }
    }
}

impl PooledConnection {
    /// 커넥션을 풀에서 분리하여 반환 (Drop 시 풀로 반환하지 않음)
    /// 장기 보유하는 Repository 등에서 사용
    /// 이미 take된 경우 None 반환
    pub fn into_inner(mut self) -> Option<Connection> {
        self.inner.take()
    }
}

impl std::ops::Deref for PooledConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.inner
            .as_ref()
            .expect("PooledConnection used after take")
    }
}

/// 풀의 모든 커넥션을 drain (data_root 변경 시 호출)
///
/// DB 경로가 변경되면 기존 풀의 커넥션은 이전 DB를 가리키므로 제거 필요.
#[allow(dead_code)] // data_root 설정 기능 구현 시 사용 예정
pub fn drain_pool() {
    if let Ok(mut pool) = CONN_POOL.lock().or_else(|e| Ok::<_, ()>(e.into_inner())) {
        let count = pool.1.len();
        pool.0 = None;
        pool.1.clear();
        if count > 0 {
            tracing::info!("Connection pool drained: {} connections removed", count);
        }
    }
}

/// DB 연결 획득 (풀 우선, 없으면 새 연결 + PRAGMA 설정)
///
/// 풀에 유휴 커넥션이 있으면 PRAGMA 없이 즉시 반환 (~0ms).
/// DB 경로가 변경된 경우, 풀을 drain하고 새 커넥션 생성.
/// HDD에서는 mmap_size=0으로 설정하여 랜덤 I/O 방지.
pub fn get_connection(db_path: &Path) -> Result<PooledConnection> {
    let path_str = db_path.to_string_lossy().to_string();

    // 풀에서 재사용 시도 (PRAGMA 스킵, poison 복구 포함)
    if let Ok(mut pool) = CONN_POOL.lock().or_else(|e| Ok::<_, ()>(e.into_inner())) {
        // 경로 불일치 시 풀 drain
        let path_matches = pool.0.as_ref().is_some_and(|p| *p == path_str);
        if !path_matches {
            let drained = pool.1.len();
            pool.1.clear();
            pool.0 = Some(path_str.clone());
            if drained > 0 {
                tracing::info!(
                    "Connection pool drained ({} conns): DB path changed to {}",
                    drained,
                    path_str
                );
            }
        } else if let Some(conn) = pool.1.pop() {
            return Ok(PooledConnection { inner: Some(conn) });
        }
    }

    // 새 커넥션 생성 + PRAGMA 설정
    let conn = Connection::open(db_path)?;

    // HDD 감지: mmap은 HDD에서 랜덤 I/O → 디스크 헤드 thrashing
    let is_hdd = crate::utils::disk_info::detect_disk_type(db_path).is_hdd();
    let mmap_size = if is_hdd { 0 } else { 67108864 }; // SSD: 64MB, HDD: 0

    // 모든 PRAGMA를 단일 배치로 실행 (개별 호출 대비 ~50% 오버헤드 절감)
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA journal_size_limit = 67108864;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -16384;
         PRAGMA mmap_size = {};
         PRAGMA temp_store = MEMORY;",
        mmap_size
    ))?;

    Ok(PooledConnection { inner: Some(conn) })
}
