use once_cell::sync::Lazy;
use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

// ==================== 커넥션 풀 ====================

/// 커넥션 풀 (DB 경로별, 경로당 최대 MAX_POOL_SIZE개, Drop 시 자동 반환)
/// 매 쿼리마다 Connection::open + PRAGMA 8개 실행하던 오버헤드를 제거.
/// HDD 환경에서 쿼리당 10-30ms 절감.
/// i3-12100 (4C) 기준 동시 DB 접근은 3-4개면 충분.
///
/// `HashMap<DB 경로, 풀 커넥션 목록>`. 구버전은 단일 경로만 추적해, 서로 다른 DB를
/// 동시에 여는 경로(테스트의 병렬 tempdir DB, 앱의 data_root 변경)에서 풀을 끊임없이
/// 무효화 → 비결정적 동작을 유발했다(통합테스트 flaky의 근본 원인). 경로별 버킷으로
/// 분리해 상호 간섭을 제거한다. 앱은 단일 DB 경로만 쓰므로 동작/오버헤드는 동일하다.
static CONN_POOL: Lazy<Mutex<HashMap<String, Vec<Connection>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
/// 풀 크기: Repository 2개(into_inner 영구 점유) + pipeline/vector_worker/prefetch/
/// watch event_loop + 다수 IPC 커맨드 동시 실행을 흡수.
/// 6은 Repository 2 고정 점유 후 4개만 남아 폭주 상황에서 부족 → 16으로 상향.
const MAX_POOL_SIZE: usize = 16;

/// 풀에서 관리되는 DB 커넥션 래퍼
/// Deref<Target=Connection>으로 기존 &Connection API 호환.
/// Drop 시 트랜잭션이 없으면 풀에 자동 반환.
pub struct PooledConnection {
    inner: Option<Connection>,
    db_path: String,
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.inner.take() {
            if !conn.is_autocommit() {
                // 열린 트랜잭션 발견 → ROLLBACK 후 풀에 반환
                // (panic 후 catch_unwind에서 연결이 Drop되는 경우 WAL lock 잔류 방지)
                if let Err(e) = conn.execute_batch("ROLLBACK") {
                    tracing::warn!("[Pool] ROLLBACK failed on drop: {}", e);
                    return; // ROLLBACK 실패 시 풀에 반환하지 않음
                }
            }
            let mut pool = CONN_POOL.lock().unwrap_or_else(|e| e.into_inner());
            let bucket = pool.entry(std::mem::take(&mut self.db_path)).or_default();
            if bucket.len() < MAX_POOL_SIZE {
                bucket.push(conn);
            }
        }
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
pub fn drain_pool() {
    if let Ok(mut pool) = CONN_POOL.lock().or_else(|e| Ok::<_, ()>(e.into_inner())) {
        let count: usize = pool.values().map(Vec::len).sum();
        pool.clear();
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

    // 풀에서 재사용 시도 (PRAGMA 스킵, poison 복구 포함). 경로별 버킷이라
    // 다른 경로의 동시 접근은 이 경로의 버킷에 영향을 주지 않는다.
    if let Ok(mut pool) = CONN_POOL.lock().or_else(|e| Ok::<_, ()>(e.into_inner())) {
        if let Some(conn) = pool.get_mut(&path_str).and_then(Vec::pop) {
            return Ok(PooledConnection {
                inner: Some(conn),
                db_path: path_str,
            });
        }
    }

    // 새 커넥션 생성 + PRAGMA 설정
    let conn = Connection::open(db_path)?;

    // HDD 감지: mmap은 HDD에서 랜덤 I/O → 디스크 헤드 thrashing
    let is_hdd = crate::utils::disk_info::detect_disk_type(db_path).is_hdd();
    let mmap_size = if is_hdd { 0 } else { 67108864 }; // SSD: 64MB, HDD: 0

    // 모든 PRAGMA를 단일 배치로 실행 (개별 호출 대비 ~50% 오버헤드 절감)
    // cache_size 64MB: 수 GB FTS DB에서 16MB는 b-tree 내부 노드도 못 담음.
    // HDD는 mmap=0이라 page cache가 유일한 캐시 — 상향 체감이 특히 큼.
    conn.execute_batch(&format!(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 30000;
         PRAGMA journal_size_limit = 67108864;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -65536;
         PRAGMA mmap_size = {};
         PRAGMA temp_store = MEMORY;",
        mmap_size
    ))?;

    Ok(PooledConnection {
        inner: Some(conn),
        db_path: path_str,
    })
}

/// WAL 체크포인트 (대량 배치 작업 후 WAL 파일 크기 관리)
/// TRUNCATE 모드: WAL 파일을 0바이트로 줄임
pub fn wal_checkpoint(db_path: &std::path::Path) {
    if let Ok(conn) = get_connection(db_path) {
        match conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
            Ok(_) => tracing::debug!("[Pool] WAL checkpoint completed"),
            Err(e) => tracing::debug!("[Pool] WAL checkpoint skipped: {}", e),
        }
    }
}
