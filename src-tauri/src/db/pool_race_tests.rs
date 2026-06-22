//! 회귀 테스트: 커넥션 풀이 서로 다른 DB 경로의 커넥션을 교차 반환하지 않는지 검증.
//!
//! 배경: 구버전 풀은 `(현재 경로, Vec<Connection>)` 전역 단일 슬롯이었다. `Drop` 은
//! 반환되는 커넥션이 어느 DB 출신인지 모른 채 "현재 경로" 버킷에 밀어 넣었기 때문에,
//! 병렬로 서로 다른 DB 를 여는 호출자(통합테스트의 tempdir DB, 앱의 data_root 변경)
//! 사이에서 커넥션이 섞였다 — A 가 커넥션을 빌린 사이 B 가 현재 경로를 바꾸면, A 가
//! 반환한 커넥션이 B 의 버킷으로 들어가 B 가 엉뚱한 DB 를 조회했다. 이로 인해
//! operator_search 통합테스트가 비결정적으로 "검색 결과 누락"으로 실패했다.
//!
//! 이 테스트는 두 경로를 tight-loop 로 번갈아 빌리고 즉시 반환하면서, 받은 커넥션이
//! 실제로 요청한 경로의 DB 파일을 가리키는지(`Connection::path()`) 검증한다. 경로별
//! 버킷 분리 수정 후에는 교차가 절대 발생하지 않아야 한다.

use super::pool::get_connection;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn pool_never_returns_connection_for_wrong_path() {
    let dir = tempfile::tempdir().unwrap();
    let pa = dir.path().join("a.db");
    let pb = dir.path().join("b.db");

    // 각 DB 에 고유 마커를 심는다. 받은 커넥션이 요청한 DB 의 것인지 마커로 판별 —
    // 경로 문자열 비교는 macOS 심볼릭 링크(/var → /private/var) 등으로 거짓 양성을
    // 내므로, DB 내용으로 출처를 식별한다.
    for (p, marker) in [(&pa, "A"), (&pb, "B")] {
        let c = get_connection(p).unwrap();
        c.execute_batch(&format!(
            "CREATE TABLE marker(id TEXT); INSERT INTO marker VALUES('{marker}');"
        ))
        .unwrap();
    }

    let stop = Arc::new(AtomicBool::new(false));
    let mismatches = Arc::new(AtomicUsize::new(0));

    let spawn_worker = |path: std::path::PathBuf,
                        expected: &'static str,
                        stop: Arc<AtomicBool>,
                        mism: Arc<AtomicUsize>| {
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let conn = get_connection(&path).unwrap();
                // 받은 커넥션이 요청한 경로의 DB(고유 마커)를 가리켜야 한다.
                let got: String = conn
                    .query_row("SELECT id FROM marker", [], |r| r.get(0))
                    .unwrap_or_default();
                if got != expected {
                    mism.fetch_add(1, Ordering::Relaxed);
                }
                drop(conn); // 즉시 반환 → 풀 회전(빌림↔반환)을 극대화해 race 노출
            }
        })
    };

    let h1 = spawn_worker(pa.clone(), "A", stop.clone(), mismatches.clone());
    let h2 = spawn_worker(pb.clone(), "B", stop.clone(), mismatches.clone());

    std::thread::sleep(Duration::from_millis(300));
    stop.store(true, Ordering::Relaxed);
    h1.join().unwrap();
    h2.join().unwrap();

    assert_eq!(
        mismatches.load(Ordering::Relaxed),
        0,
        "풀이 요청한 경로와 다른 DB 의 커넥션을 반환함 (경로 간 커넥션 누출)"
    );
}
