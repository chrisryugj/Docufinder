//! kordoc parse-worker 풀 — 파일마다 node 를 새로 띄우던 콜드스타트를 없앤다 (kordoc 4.14.3+).
//!
//! 프로토콜 (NDJSON, 한 줄 = 한 메시지):
//! - 시작 `{"ready":true,"version":"4.14.3","protocol":1}`
//! - 요청 `{"id":1,"file":"a.hwpx","images":false,"ocr":"off","formulaOcr":false,"password":null}`
//! - 응답 `{"id":1,"rss":123456789,"result":{…}}` — result 는 `--format json` 이 내는 객체 그대로
//!   (파싱 실패도 `success:false` + `code` 로 여기 담긴다). 요청 자체가 잘못되면 `{"id":1,"error":"…"}`.
//! - 종료 `{"cmd":"quit"}` 또는 stdin 닫힘.
//!
//! 워커를 못 쓰는 경우(구버전 kordoc·시작 실패·파이프 끊김·비정상 종료)는 호출자가 1회성
//! 실행으로 폴백한다. 타임아웃은 워커만 버리고 폴백하지 않는다 (대기가 두 배가 된다).

use super::process::which_node;
use super::{find_kordoc_cli, KordocOcrMode, KordocOptions};
use crate::parsers::ParseError;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// 동시에 띄우는 워커 수 상한 (node + kordoc 한 개 약 150MB)
const MAX_WORKERS: usize = 4;
/// 요청을 이만큼 처리한 워커는 교체한다 (오래 돈 V8 힙의 누적 메모리 방어)
const RECYCLE_AFTER: u32 = 300;
/// 워커가 보고한 RSS 가 이 값을 넘으면 교체한다
const RECYCLE_RSS_BYTES: u64 = 1536 * 1024 * 1024;
/// 이만큼 쉬면 대기 워커를 모두 내린다 (인덱싱이 끝난 뒤 node 가 메모리를 쥐고 있지 않게)
const IDLE_SHUTDOWN: Duration = Duration::from_secs(120);
/// 워커 준비(모듈 로드) 대기 상한
const READY_TIMEOUT: Duration = Duration::from_secs(30);
/// 응답 한 줄 상한 — 1회성 실행의 stdout 상한과 같다
const MAX_RESPONSE_BYTES: usize = 100 * 1024 * 1024;

/// 살아 있는 parse-worker 프로세스 한 개
pub(super) struct ParseWorker {
    child: Child,
    stdin: ChildStdin,
    resp_rx: Receiver<String>,
    next_id: u64,
    served: u32,
}

impl ParseWorker {
    fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait(); // kill 만 하면 유닉스에서 좀비로 남는다
    }
}

/// 워커를 쓸 수 없는 이유 — 호출자가 1회성 실행으로 폴백할지 가른다
#[derive(Debug)]
pub(super) enum WorkerError {
    /// 파이프 끊김·프로세스 종료·잘못된 응답: 1회성 실행으로 다시 시도할 가치가 있다
    Broken(String),
    /// 제한 시간 초과: 폴백하면 대기만 두 배가 된다
    Timeout(u64),
}

struct PoolState {
    idle: Vec<ParseWorker>,
    /// 대기 + 사용 중 워커 수
    live: usize,
    /// 시작이 한 번이라도 실패하면 이번 실행 동안 풀을 끈다 (계속 실패할 가능성이 높다)
    disabled: bool,
    last_used: Option<Instant>,
    reaper_started: bool,
}

pub(super) struct WorkerPool {
    state: Mutex<PoolState>,
    freed: Condvar,
    max: usize,
}

/// 앱 전역 풀
pub(super) static PARSE_POOL: WorkerPool = WorkerPool::new(MAX_WORKERS);

impl WorkerPool {
    pub(super) const fn new(max: usize) -> Self {
        Self {
            state: Mutex::new(PoolState {
                idle: Vec::new(),
                live: 0,
                disabled: false,
                last_used: None,
                reaper_started: false,
            }),
            freed: Condvar::new(),
            max,
        }
    }

    fn lock(&self) -> Option<MutexGuard<'_, PoolState>> {
        self.state.lock().ok()
    }

    /// 대기 워커를 꺼내거나 새로 띄운다. `wait=false`(미리보기)는 모두 바쁘면 기다리지 않고
    /// None 을 돌려 1회성 실행으로 가게 한다 — 인덱싱이 워커를 다 쓰는 동안 미리보기가
    /// 몇 초씩 밀리지 않도록. None 은 "워커 없이 진행" 이다.
    fn checkout(
        &'static self,
        wait: bool,
        spawn: impl Fn() -> Result<ParseWorker, WorkerError>,
    ) -> Option<ParseWorker> {
        let mut state = self.lock()?;
        loop {
            if state.disabled {
                return None;
            }
            if let Some(worker) = state.idle.pop() {
                return Some(worker);
            }
            if state.live < self.max {
                state.live += 1;
                self.start_reaper(&mut state);
                drop(state);
                return match spawn() {
                    Ok(worker) => Some(worker),
                    Err(e) => {
                        warn!("kordoc parse-worker 시작 실패, 이번 실행은 1회성 실행으로: {e:?}");
                        if let Some(mut s) = self.lock() {
                            s.live -= 1;
                            s.disabled = true;
                        }
                        self.freed.notify_all();
                        None
                    }
                };
            }
            if !wait {
                return None;
            }
            state = self.freed.wait(state).ok()?;
        }
    }

    /// 다 쓴 워커를 돌려놓는다. `reusable=false` 면 내리고 자리를 비운다.
    fn checkin(&self, worker: ParseWorker, reusable: bool) {
        let Some(mut state) = self.lock() else {
            worker.kill();
            return;
        };
        state.last_used = Some(Instant::now());
        if reusable && !state.disabled {
            state.idle.push(worker);
            drop(state);
        } else {
            state.live -= 1;
            drop(state);
            worker.kill();
        }
        self.freed.notify_one();
    }

    /// 쉬는 워커 정리 스레드 (첫 워커를 띄울 때 한 번)
    fn start_reaper(&'static self, state: &mut PoolState) {
        if state.reaper_started {
            return;
        }
        state.reaper_started = true;
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            let drained: Vec<ParseWorker> = {
                let Some(mut s) = self.lock() else { return };
                let idle_for = s.last_used.map(|t| t.elapsed()).unwrap_or_default();
                if s.idle.is_empty() || idle_for < IDLE_SHUTDOWN {
                    continue;
                }
                let drained = std::mem::take(&mut s.idle);
                s.live -= drained.len();
                drained
            };
            debug!("kordoc parse-worker {}개 유휴 종료", drained.len());
            for worker in drained {
                worker.kill();
            }
        });
    }
}

/// `node <cli> parse-worker` 를 띄우고 ready 신호를 받는다.
pub(super) fn spawn_worker(node: &Path, cli: &Path) -> Result<ParseWorker, WorkerError> {
    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli.to_string_lossy().as_ref())
        .arg("parse-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| WorkerError::Broken(format!("parse 워커 시작 실패: {e}")))?;
    // 이슈 #33: 앱 종료/크래시 시 고아 방지
    crate::utils::process_job::track_child(&child);

    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(WorkerError::Broken(
            "parse 워커 파이프 캡처 실패".to_string(),
        ));
    };

    // 백그라운드 리더 — stdout 라인을 채널로 (요청 스레드는 recv_timeout 으로 기다린다)
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let worker = ParseWorker {
        child,
        stdin,
        resp_rx: rx,
        next_id: 1,
        served: 0,
    };
    // ready 줄을 기다린다. 모듈을 불러오며 찍힌 잡음 줄은 건너뛴다(한 줄에 풀을 통째로 끄지 않게).
    // 구버전 kordoc 은 parse-worker 를 파일 경로로 받아 곧바로 에러 종료한다 → 채널이 닫혀 즉시 실패.
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let line = match worker.resp_rx.recv_timeout(remaining) {
            Ok(line) => line,
            Err(_) => {
                worker.kill();
                return Err(WorkerError::Broken(
                    "parse 워커 준비 실패 (응답 없음)".to_string(),
                ));
            }
        };
        match parse_reply(&line) {
            Some(reply) if reply.ready == Some(true) => return Ok(worker),
            Some(_) => {
                worker.kill();
                return Err(WorkerError::Broken(format!(
                    "parse 워커 준비 실패, 예상치 못한 응답: {}",
                    line.chars().take(80).collect::<String>()
                )));
            }
            None => continue, // 잡음 줄
        }
    }
}

/// 워커 출력 한 줄. result 는 파싱하지 않고 원문 그대로 받아(RawValue) 호출자가 한 번만 파싱한다 —
/// Value 트리로 읽었다가 문자열로 되돌려 다시 파싱하면 큰 문서에서 메모리를 세 배로 썼다.
#[derive(serde::Deserialize)]
struct Reply<'a> {
    ready: Option<bool>,
    id: Option<u64>,
    rss: Option<u64>,
    #[serde(borrow)]
    result: Option<&'a serde_json::value::RawValue>,
    error: Option<String>,
}

/// pdfjs 경고 같은 잡음이 같은 줄 앞에 붙어도 첫 `{` 부터 읽는다. JSON 이 아니면 None.
fn parse_reply(line: &str) -> Option<Reply<'_>> {
    let start = line.find('{')?;
    serde_json::from_str(&line[start..]).ok()
}

/// 요청 한 건 JSON
pub(super) fn request_json(id: u64, file: &str, opts: &KordocOptions) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "file": file,
        // 앱은 문서 속 이미지를 쓰지 않는다 (미리보기는 "그림" 자리 표시만)
        "images": false,
        "ocr": match opts.ocr {
            KordocOcrMode::Off => "off",
            KordocOcrMode::Auto => "auto",
            KordocOcrMode::Force => "force",
        },
        "formulaOcr": opts.formula_ocr,
        "password": opts.password,
    })
}

/// 워커 하나에 요청을 보내고 응답(result 객체 JSON)과 워커가 보고한 RSS 를 받는다.
pub(super) fn run_request(
    worker: &mut ParseWorker,
    file: &str,
    opts: &KordocOptions,
    timeout: Duration,
) -> Result<(String, Option<u64>), WorkerError> {
    let id = worker.next_id;
    worker.next_id += 1;
    worker.served += 1;

    let req = request_json(id, file, opts);
    if writeln!(worker.stdin, "{req}")
        .and_then(|_| worker.stdin.flush())
        .is_err()
    {
        return Err(WorkerError::Broken(
            "parse 워커 통신 실패 (파이프 끊김)".to_string(),
        ));
    }

    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(WorkerError::Timeout(timeout.as_secs()));
        }
        let line = match worker.resp_rx.recv_timeout(remaining) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => return Err(WorkerError::Timeout(timeout.as_secs())),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(WorkerError::Broken("parse 워커 종료됨".to_string()))
            }
        };
        if line.len() > MAX_RESPONSE_BYTES {
            return Err(WorkerError::Broken(format!(
                "parse 워커 응답 크기 초과: {}MB",
                line.len() / 1_048_576
            )));
        }
        let Some(reply) = parse_reply(&line) else {
            continue; // 잡음 라인
        };
        if reply.id != Some(id) {
            continue; // 버린 요청의 늦은 응답
        }
        if let Some(result) = reply.result {
            return Ok((result.get().to_string(), reply.rss));
        }
        let msg = reply
            .error
            .unwrap_or_else(|| "parse 워커 응답에 result 가 없습니다".to_string());
        return Err(WorkerError::Broken(msg));
    }
}

/// 풀의 워커로 파싱한다. `None` = 워커를 쓰지 못했으니 1회성 실행으로 진행.
/// `interactive`(미리보기)는 워커가 모두 바쁘면 기다리지 않는다.
pub(super) fn parse_via_pool(
    file_path: &Path,
    opts: &KordocOptions,
    timeout_secs: u64,
    interactive: bool,
) -> Option<Result<String, ParseError>> {
    let (node, cli): (PathBuf, PathBuf) = (which_node()?, find_kordoc_cli()?);
    let mut worker = PARSE_POOL.checkout(!interactive, || spawn_worker(&node, &cli))?;

    let file_owned = crate::utils::network_path::simplify(file_path);
    let file_str = file_owned.to_string_lossy().to_string();
    match run_request(
        &mut worker,
        &file_str,
        opts,
        Duration::from_secs(timeout_secs),
    ) {
        Ok((json, rss)) => {
            let reusable =
                worker.served < RECYCLE_AFTER && rss.is_none_or(|r| r < RECYCLE_RSS_BYTES);
            PARSE_POOL.checkin(worker, reusable);
            Some(Ok(json))
        }
        Err(WorkerError::Timeout(secs)) => {
            PARSE_POOL.checkin(worker, false);
            warn!(
                "kordoc parse 워커 타임아웃 ({secs}초 초과): {}",
                file_path.display()
            );
            Some(Err(ParseError::ParseError(format!(
                "kordoc 타임아웃 ({secs}초 초과): {}",
                file_path.display()
            ))))
        }
        Err(WorkerError::Broken(msg)) => {
            PARSE_POOL.checkin(worker, false);
            warn!("kordoc parse 워커 실패, 1회성 실행으로 다시: {msg}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 프로토콜을 흉내 내는 가짜 parse-worker. 파일 이름으로 동작을 고른다.
    const FAKE_WORKER: &str = r#"
import { createInterface } from "node:readline";
const rl = createInterface({ input: process.stdin });
const write = (o) => process.stdout.write(JSON.stringify(o) + "\n");
write({ ready: true, version: "9.9.9", protocol: 1 });
for await (const line of rl) {
  const req = JSON.parse(line);
  if (req.cmd === "quit") break;
  if (req.file.endsWith("crash.hwp")) process.exit(3);
  if (req.file.endsWith("slow.hwp")) await new Promise((r) => setTimeout(r, 5000));
  if (req.file.endsWith("noise.hwp")) {
    process.stdout.write("Warning: TT: undefined function\n");
    write({ id: 999999, rss: 1, result: { success: true, markdown: "늦은 응답" } });
  }
  if (req.file.endsWith("bad.hwp")) { write({ id: req.id, error: "file 필수" }); continue; }
  write({
    id: req.id,
    rss: req.file.endsWith("fat.hwp") ? 9e9 : 1000,
    result: { success: true, markdown: "본문 " + req.file, images: req.images, ocr: req.ocr },
  });
}
"#;
    /// parse-worker 가 없는 구버전 kordoc 흉내 — 인자를 파일로 받아 곧바로 에러 종료
    const OLD_KORDOC: &str =
        r#"process.stderr.write("파일을 찾을 수 없습니다: parse-worker\n"); process.exit(1);"#;

    fn setup(script: &str) -> Option<(tempfile::TempDir, PathBuf, PathBuf)> {
        let node = which_node()?;
        let dir = tempfile::tempdir().ok()?;
        let cli = dir.path().join("cli.mjs");
        std::fs::write(&cli, script).ok()?;
        Some((dir, node, cli))
    }

    #[test]
    fn worker_roundtrip_reuses_process() {
        let Some((_d, node, cli)) = setup(FAKE_WORKER) else {
            return;
        };
        let mut w = spawn_worker(&node, &cli).expect("spawn");
        let opts = KordocOptions::default();
        let (json, rss) = run_request(&mut w, "a.hwpx", &opts, Duration::from_secs(10)).unwrap();
        assert!(json.contains("본문 a.hwpx"), "{json}");
        assert!(
            json.contains("\"images\":false") && json.contains("\"ocr\":\"off\""),
            "{json}"
        );
        assert_eq!(rss, Some(1000));
        let (json2, _) = run_request(&mut w, "b.hwpx", &opts, Duration::from_secs(10)).unwrap();
        assert!(json2.contains("본문 b.hwpx"), "{json2}");
        assert_eq!(w.served, 2);
        w.kill();
    }

    #[test]
    fn worker_skips_noise_and_stale_ids() {
        let Some((_d, node, cli)) = setup(FAKE_WORKER) else {
            return;
        };
        let mut w = spawn_worker(&node, &cli).expect("spawn");
        let (json, _) = run_request(
            &mut w,
            "noise.hwp",
            &KordocOptions::default(),
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(
            json.contains("본문 noise.hwp"),
            "잡음 줄·다른 id 응답을 건너뛰어야 함: {json}"
        );
        w.kill();
    }

    #[test]
    fn worker_crash_and_bad_request_are_broken() {
        let Some((_d, node, cli)) = setup(FAKE_WORKER) else {
            return;
        };
        let opts = KordocOptions::default();
        let mut w = spawn_worker(&node, &cli).expect("spawn");
        let err = run_request(&mut w, "bad.hwp", &opts, Duration::from_secs(10)).unwrap_err();
        assert!(
            matches!(err, WorkerError::Broken(ref m) if m.contains("file 필수")),
            "{err:?}"
        );
        let err = run_request(&mut w, "crash.hwp", &opts, Duration::from_secs(10)).unwrap_err();
        assert!(matches!(err, WorkerError::Broken(_)), "{err:?}");
        w.kill();
    }

    #[test]
    fn worker_timeout_is_not_broken() {
        let Some((_d, node, cli)) = setup(FAKE_WORKER) else {
            return;
        };
        let mut w = spawn_worker(&node, &cli).expect("spawn");
        let err = run_request(
            &mut w,
            "slow.hwp",
            &KordocOptions::default(),
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(matches!(err, WorkerError::Timeout(1)), "{err:?}");
        w.kill();
    }

    /// 모듈을 불러오며 찍힌 잡음 줄 뒤에 ready 가 와도 워커를 쓴다 (한 줄에 풀이 꺼지지 않게).
    #[test]
    fn noise_before_ready_is_skipped() {
        let noisy =
            format!("process.stdout.write(\"(node) Warning: something noisy\\n\");\n{FAKE_WORKER}");
        let Some((_d, node, cli)) = setup(&noisy) else {
            return;
        };
        let mut w = spawn_worker(&node, &cli).expect("잡음 뒤 ready 를 받아야 함");
        let (json, _) = run_request(
            &mut w,
            "a.hwpx",
            &KordocOptions::default(),
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(json.contains("본문 a.hwpx"), "{json}");
        w.kill();
    }

    #[test]
    fn old_kordoc_without_parse_worker_fails_fast() {
        let Some((_d, node, cli)) = setup(OLD_KORDOC) else {
            return;
        };
        let t = Instant::now();
        assert!(spawn_worker(&node, &cli).is_err());
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "ready 타임아웃(30초)까지 기다리면 안 됨"
        );
    }

    #[test]
    fn pool_reuses_recycles_and_skips_when_busy() {
        let Some((_d, node, cli)) = setup(FAKE_WORKER) else {
            return;
        };
        let pool: &'static WorkerPool = Box::leak(Box::new(WorkerPool::new(1)));
        let spawned = AtomicUsize::new(0);
        let spawn = || {
            spawned.fetch_add(1, Ordering::SeqCst);
            spawn_worker(&node, &cli)
        };

        let mut w = pool.checkout(true, spawn).expect("first worker");
        run_request(
            &mut w,
            "a.hwpx",
            &KordocOptions::default(),
            Duration::from_secs(10),
        )
        .unwrap();
        // 한 개뿐인 워커가 쓰이는 중: 미리보기(wait=false)는 기다리지 않고 None
        assert!(pool.checkout(false, spawn).is_none());
        pool.checkin(w, true);

        let w = pool.checkout(false, spawn).expect("idle worker");
        assert_eq!(w.served, 1, "같은 워커를 다시 써야 함");
        assert_eq!(spawned.load(Ordering::SeqCst), 1);
        pool.checkin(w, false); // 교체 → 자리 비움

        let w = pool.checkout(true, spawn).expect("replacement");
        assert_eq!(w.served, 0);
        assert_eq!(spawned.load(Ordering::SeqCst), 2);
        pool.checkin(w, false);
    }

    #[test]
    fn pool_disables_after_spawn_failure() {
        let pool: &'static WorkerPool = Box::leak(Box::new(WorkerPool::new(2)));
        let spawned = AtomicUsize::new(0);
        let spawn = || {
            spawned.fetch_add(1, Ordering::SeqCst);
            Err(WorkerError::Broken("no parse-worker".to_string()))
        };
        assert!(pool.checkout(true, spawn).is_none());
        assert!(pool.checkout(true, spawn).is_none());
        assert_eq!(
            spawned.load(Ordering::SeqCst),
            1,
            "한 번 실패하면 다시 띄우지 않는다"
        );
    }
}
