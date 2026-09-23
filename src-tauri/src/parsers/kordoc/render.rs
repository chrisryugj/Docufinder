//! kordoc render 서브커맨드: 레이아웃 SVG 렌더 (persistent 워커 우선, 1회성 spawn 폴백).

use super::process::{run_kordoc_process, stderr_snippet, validate_file_size, which_node};
use super::{find_kordoc_cli, KORDOC_TIMEOUT_SECS, RENDER_MAX_SVG_SIZE};
use crate::parsers::ParseError;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// kordoc render — HWPX 조판을 전체 페이지 세로 스택 SVG 로 렌더 (레이아웃 미리보기용).
/// `highlights` 는 검색어 형광펜 (kordoc `--highlight`).
///
/// persistent 워커(render-worker)를 우선 사용해 node 콜드스타트를 없애고, 워커 통신
/// 실패 시 1회성 spawn 으로 폴백한다. `reflow=true` 로 조판 캐시 없는 HWPX(생성/편집본)도
/// 순수 TS 조판으로 렌더한다(캐시 있으면 kordoc 이 자동으로 캐시 재생 — 무회귀).
pub fn render_svg(path: &Path, highlights: &[String]) -> Result<String, ParseError> {
    validate_file_size(path)?;
    match render_svg_via_worker(path, highlights, true) {
        Ok(svg) => Ok(svg),
        // 워커 인프라 실패(spawn·파이프 끊김·프로세스 종료)만 1회성 spawn 폴백.
        // 렌더 실패(ok:false)는 콜드스타트로 재시도해도 동일하게 실패하고, 타임아웃은
        // 폴백까지 겹치면 60+60 ≈ 120초 스피너가 되므로 즉시 에러 반환한다.
        Err((e, true)) => {
            warn!("render 워커 인프라 실패 — 1회성 spawn 폴백: {}", e);
            render_svg_oneshot(path, highlights, true)
        }
        Err((e, false)) => Err(e),
    }
}

/// render 1회성 spawn (워커 폴백) — 임시 파일(-o)로 받아 읽은 뒤 삭제한다.
fn render_svg_oneshot(
    path: &Path,
    highlights: &[String],
    reflow: bool,
) -> Result<String, ParseError> {
    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    let file_owned = crate::utils::network_path::simplify(path);
    let file_str = file_owned.to_string_lossy();

    // 유니크 임시 경로만 만들고 파일 생성은 kordoc 에 맡긴다 — 미리 열어두면
    // Windows 파일 공유 잠금과 충돌할 수 있다.
    let out_path = std::env::temp_dir().join(format!(
        "docufinder-render-{}-{}.svg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));

    debug!("kordoc render: {} -o {}", file_str, out_path.display());

    let mut args: Vec<std::ffi::OsString> = vec![
        "render".into(),
        file_str.as_ref().into(),
        "-o".into(),
        out_path.clone().into(),
        "--silent".into(),
    ];
    // reflow 는 kordoc 기본값이라 끌 때만 넘긴다(`--reflow` 옵션은 없어 폴백이 매번 실패했다).
    if !reflow {
        args.push("--no-reflow".into());
    }
    // kordoc 은 쉼표 구분 목록을 받으므로 검색어 내 쉼표는 공백으로 정규화
    let terms: Vec<String> = highlights
        .iter()
        .map(|t| t.replace(',', " ").trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if !terms.is_empty() {
        args.push("--highlight".into());
        args.push(terms.join(",").into());
    }

    let result = (|| {
        let out = run_kordoc_process(
            &cli_path,
            &args,
            KORDOC_TIMEOUT_SECS,
            &path.display().to_string(),
        )?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("kordoc render failed (exit {}): {}", out.status, stderr);
            let snippet = stderr_snippet(&stderr);
            return Err(ParseError::ParseError(if snippet.is_empty() {
                format!("kordoc render 실패 (exit {})", out.status)
            } else {
                // "[kordoc] 오류:" 는 CLI 로그 프리픽스 — 사용자 노출 메시지에서 제거
                snippet
                    .trim_start_matches("[kordoc] 오류:")
                    .trim()
                    .to_string()
            }));
        }

        // exit 0 이어도 출력 파일 존재/크기로 최종 판정 (조판 캐시 이상 등 방어)
        let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        if size == 0 {
            return Err(ParseError::ParseError(
                "kordoc render: SVG 출력이 생성되지 않았습니다".to_string(),
            ));
        }
        if size > RENDER_MAX_SVG_SIZE {
            return Err(ParseError::ParseError(format!(
                "레이아웃 SVG 크기 초과: {}MB (최대 {}MB)",
                size / 1_048_576,
                RENDER_MAX_SVG_SIZE / 1_048_576
            )));
        }

        std::fs::read_to_string(&out_path)
            .map_err(|e| ParseError::ParseError(format!("SVG 읽기 실패: {e}")))
    })();

    // 성공/실패 무관 임시 파일 정리 (없으면 무시)
    let _ = std::fs::remove_file(&out_path);

    result
}

// ─── persistent 렌더 워커 (콜드스타트 제거) ───────────────

/// 워커 응답용 유니크 임시 SVG 경로
fn temp_svg_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "docufinder-render-{}-{}.svg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ))
}

/// 살아있는 render-worker 프로세스 + stdin + 응답 채널(백그라운드 리더 스레드)
struct RenderWorker {
    child: Child,
    stdin: ChildStdin,
    resp_rx: Receiver<String>,
    next_id: u64,
}

/// 전역 단일 렌더 워커 — 미리보기는 순차라 Mutex 직렬화로 충분하다.
static RENDER_WORKER: Mutex<Option<RenderWorker>> = Mutex::new(None);

/// render-worker 프로세스를 띄우고 ready 신호를 소비한다.
fn spawn_render_worker() -> Result<RenderWorker, ParseError> {
    let node = which_node()
        .ok_or_else(|| ParseError::ParseError("Node.js가 설치되지 않았습니다".to_string()))?;
    let cli_path = find_kordoc_cli()
        .ok_or_else(|| ParseError::ParseError("kordoc CLI를 찾을 수 없습니다".to_string()))?;

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli_path.to_string_lossy().as_ref())
        .arg("render-worker")
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
        .map_err(|e| ParseError::ParseError(format!("render 워커 시작 실패: {e}")))?;
    // 이슈 #33: 앱 종료/크래시 시 고아 방지
    crate::utils::process_job::track_child(&child);

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ParseError::ParseError("워커 stdin 캡처 실패".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ParseError::ParseError("워커 stdout 캡처 실패".to_string()))?;

    // 백그라운드 리더 — stdout 라인을 채널로 (요청 스레드는 recv_timeout 으로 대기)
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
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

    let mut worker = RenderWorker {
        child,
        stdin,
        resp_rx: rx,
        next_id: 1,
    };
    // ready 신호 대기 (모듈 로드 시간 고려 30초)
    match worker.resp_rx.recv_timeout(Duration::from_secs(30)) {
        Ok(line) if line.contains("\"ready\"") => Ok(worker),
        Ok(other) => {
            let _ = worker.child.kill();
            let _ = worker.child.wait(); // kill 만 하면 유닉스에서 좀비로 남는다
            Err(ParseError::ParseError(format!(
                "render 워커 준비 실패 — 예상치 못한 응답: {}",
                other.chars().take(80).collect::<String>()
            )))
        }
        Err(_) => {
            let _ = worker.child.kill();
            let _ = worker.child.wait();
            Err(ParseError::ParseError(
                "render 워커 준비 타임아웃".to_string(),
            ))
        }
    }
}

/// persistent 워커로 render — NDJSON 요청/응답, id 매칭, 통신/타임아웃 실패 시 워커 폐기.
///
/// 에러의 `bool` 은 "1회성 spawn 폴백이 의미 있는가" — 워커 인프라 실패(spawn·파이프
/// 끊김·프로세스 종료)만 true. 렌더 실패(ok:false)·타임아웃·SVG 후처리 실패는 폴백해도
/// 동일 실패거나 대기만 배가되므로 false.
fn render_svg_via_worker(
    path: &Path,
    highlights: &[String],
    reflow: bool,
) -> Result<String, (ParseError, bool)> {
    let file_owned = crate::utils::network_path::simplify(path);
    let file_str = file_owned.to_string_lossy().to_string();
    let out_path = temp_svg_path();
    let out_str = out_path.to_string_lossy().to_string();

    let terms: Vec<String> = highlights
        .iter()
        .map(|t| t.replace(',', " ").trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    let mut guard = RENDER_WORKER.lock().map_err(|_| {
        (
            ParseError::ParseError("render 워커 잠금 실패".to_string()),
            false,
        )
    })?;
    if guard.is_none() {
        *guard = Some(spawn_render_worker().map_err(|e| (e, true))?);
    }
    let worker = guard.as_mut().unwrap();

    let id = worker.next_id;
    worker.next_id += 1;

    let req = serde_json::json!({
        "id": id,
        "file": file_str,
        "out": out_str,
        "reflow": reflow,
        "highlight": terms,
    });

    // worker_dead: 통신/타임아웃 실패(워커 자체 이상)만 true — render 실패(ok:false)는 워커 정상.
    // 에러의 bool 은 폴백 힌트(retry_oneshot) — 타임아웃은 워커를 폐기(worker_dead)하되
    // 폴백은 하지 않는다(대기 배가 방지).
    let mut worker_dead = false;
    let result: Result<String, (ParseError, bool)> = (|| {
        if writeln!(worker.stdin, "{req}")
            .and_then(|_| worker.stdin.flush())
            .is_err()
        {
            worker_dead = true;
            return Err((
                ParseError::ParseError("render 워커 통신 실패 (파이프 끊김)".to_string()),
                true,
            ));
        }

        let deadline = Instant::now() + Duration::from_secs(KORDOC_TIMEOUT_SECS);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                worker_dead = true;
                return Err((
                    ParseError::ParseError(format!(
                        "render 워커 타임아웃 ({}초 초과)",
                        KORDOC_TIMEOUT_SECS
                    )),
                    false,
                ));
            }
            match worker.resp_rx.recv_timeout(remaining) {
                Ok(line) => {
                    let resp: serde_json::Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(_) => continue, // 잡음 라인 무시
                    };
                    if resp.get("id").and_then(|v| v.as_u64()) != Some(id) {
                        continue; // 이전 타임아웃 요청의 지연 응답 — 스킵
                    }
                    if resp.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                        let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
                        if size > RENDER_MAX_SVG_SIZE {
                            return Err((
                                ParseError::ParseError(format!(
                                    "레이아웃 SVG 크기 초과: {}MB (최대 {}MB)",
                                    size / 1_048_576,
                                    RENDER_MAX_SVG_SIZE / 1_048_576
                                )),
                                false,
                            ));
                        }
                        return std::fs::read_to_string(&out_path).map_err(|e| {
                            (ParseError::ParseError(format!("SVG 읽기 실패: {e}")), false)
                        });
                    }
                    let msg = resp
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("render 실패")
                        .to_string();
                    return Err((ParseError::ParseError(msg), false));
                }
                Err(RecvTimeoutError::Timeout) => {
                    worker_dead = true;
                    return Err((
                        ParseError::ParseError("render 워커 타임아웃".to_string()),
                        false,
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    worker_dead = true;
                    return Err((
                        ParseError::ParseError("render 워커 종료됨".to_string()),
                        true,
                    ));
                }
            }
        }
    })();

    let _ = std::fs::remove_file(&out_path);

    if worker_dead {
        if let Some(w) = guard.as_mut() {
            let _ = w.child.kill();
            let _ = w.child.wait(); // 좀비 회수 (oneshot 러너와 대칭)
        }
        *guard = None;
    }
    result
}
