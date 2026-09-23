//! kordoc CLI 프로세스 실행: node 탐색, 파일 크기 검증, 공용 러너, 파싱 호출.

use super::{
    KordocOcrMode, KordocOptions, KORDOC_FORMULA_TIMEOUT_SECS, KORDOC_TIMEOUT_SECS, NODE_BIN,
};
use crate::parsers::ParseError;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

// ─── 내부 헬퍼 ────────────────────────────────────────

/// node 실행 파일 탐색 (번들 node 우선 → 시스템 PATH)
pub(super) fn which_node() -> Option<PathBuf> {
    // 1. 번들된 node
    //    Windows: $INSTALLDIR/resources/<NODE_BIN>
    //    macOS:   Contents/Resources/resources/<NODE_BIN> (앱 번들)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(target_os = "macos")]
            {
                let mac_bundled = dir
                    .join("..")
                    .join("Resources")
                    .join("resources")
                    .join(NODE_BIN);
                if mac_bundled.exists() {
                    return Some(mac_bundled);
                }
            }
            let bundled = dir.join("resources").join(NODE_BIN);
            if bundled.exists() {
                return Some(bundled);
            }
            // 폴백: 평평한 배치 (구버전 호환)
            let flat = dir.join(NODE_BIN);
            if flat.exists() {
                return Some(flat);
            }
        }
    }

    // 2. 시스템 PATH (개발 모드 전용)
    // 프로덕션에서는 PATH hijacking(CWE-426) 방지를 위해 번들 node 만 허용.
    #[cfg(debug_assertions)]
    {
        which::which("node").ok()
    }
    #[cfg(not(debug_assertions))]
    {
        tracing::warn!(
            "Bundled {} not found next to executable — \
             HWP/HWPX parsing disabled. Reinstall the application.",
            NODE_BIN
        );
        None
    }
}

/// 파일 크기 검증 (MAX_FILE_SIZE 초과 시 거부)
pub(super) fn validate_file_size(path: &Path) -> Result<(), ParseError> {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > crate::parsers::MAX_FILE_SIZE {
        return Err(ParseError::ParseError(format!(
            "파일 크기 초과: {} bytes (최대 {} bytes)",
            size,
            crate::parsers::MAX_FILE_SIZE
        )));
    }
    Ok(())
}

/// kordoc 프로세스 실행 결과 — 성공/실패 판정과 출력 해석은 호출자 몫
/// (파싱은 stdout JSON, 렌더는 exit code + 출력 파일).
pub(super) struct KordocOutput {
    pub(super) status: std::process::ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

/// kordoc CLI 공용 러너 (blocking thread에서 사용)
///
/// spawn → Job Object 등록(이슈 #33: 고아 node 방지) → stdout/stderr drain 스레드
/// (파이프 블록 방지) → try_wait 폴링 타임아웃까지의 프로세스 안전장치를 모든
/// kordoc 서브커맨드가 공유한다 (std::process::Child는 Drop에서 kill하지 않으므로
/// 타임아웃 시 명시적 child.kill() 호출 필수).
pub(super) fn run_kordoc_process(
    cli_path: &Path,
    args: &[std::ffi::OsString],
    timeout_secs: u64,
    file_display: &str,
) -> Result<KordocOutput, ParseError> {
    use std::io::Read;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    let node = which_node()
        .ok_or_else(|| ParseError::ParseError("Node.js가 설치되지 않았습니다".to_string()))?;

    let mut cmd = std::process::Command::new(node);
    cmd.arg(cli_path.to_string_lossy().as_ref())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| ParseError::ParseError(format!("kordoc 프로세스 시작 실패: {e}")))?;

    // 이슈 #33: 앱 종료/크래시 시 이 node 자식이 고아로 남지 않도록 Job Object 에 묶는다.
    crate::utils::process_job::track_child(&child);

    let timeout = Duration::from_secs(timeout_secs);

    // stdout/stderr drain 스레드 (파이프 블록 방지)
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| ParseError::ParseError("kordoc stdout 캡처 실패".to_string()))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| ParseError::ParseError("kordoc stderr 캡처 실패".to_string()))?;

    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>();
    let (stderr_tx, stderr_rx) = mpsc::channel::<Vec<u8>>();

    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        let _ = stdout_tx.send(buf);
    });
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        let _ = stderr_tx.send(buf);
    });

    // 폴링 기반 타임아웃: std::process::Child::try_wait + 명시적 kill
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    // 타임아웃 — 명시적 kill + wait (좀비 방지)
                    let _ = child.kill();
                    let _ = child.wait();
                    warn!(
                        "kordoc 타임아웃 ({}초 초과), 프로세스 강제 종료: {}",
                        timeout_secs, file_display
                    );
                    return Err(ParseError::ParseError(format!(
                        "kordoc 타임아웃 ({}초 초과): {}",
                        timeout_secs, file_display
                    )));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ParseError::ParseError(format!(
                    "kordoc 프로세스 대기 실패: {e}"
                )));
            }
        }
    };

    // 프로세스 종료 후 파이프 drain 결과 수거 (짧은 타임아웃 — 이미 프로세스 끝났으면 즉시 EOF)
    let drain_timeout = Duration::from_secs(2);
    let stdout = stdout_rx.recv_timeout(drain_timeout).unwrap_or_default();
    let stderr = stderr_rx.recv_timeout(drain_timeout).unwrap_or_default();

    Ok(KordocOutput {
        status,
        stdout,
        stderr,
    })
}

/// stderr 의 비어있지 않은 라인 전부를 " | " 로 합쳐 사용자 가시 에러 스니펫으로 (최대 300자).
/// 첫 줄만 잡으면 "FAIL" 같은 헤더에 진짜 진단 메시지가 묻힌다 (이슈 #22) —
/// 마지막 라인 부근에 가장 구체적인 에러가 나오는 경향이 있어 모두 보존한다.
pub(super) fn stderr_snippet(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(300)
        .collect()
}

/// kordoc 버전 캐시. 읽은 값은 앱 실행 내내 쓰고(앱 시작 점검 probe_runtime 이 먼저 채우는 게 보통),
/// 못 읽었으면(첫 실행이 느린 PC 에서 15초 초과 등) 1분 뒤 다시 잰다. 실패를 세션 내내 붙들면
/// 워커·이미지 끄기가 그 실행 동안 영영 꺼졌다.
struct VersionCache {
    value: Option<(u32, u32, u32)>,
    failed_at: Option<Instant>,
}
static KORDOC_VERSION: Mutex<VersionCache> = Mutex::new(VersionCache {
    value: None,
    failed_at: None,
});
const VERSION_RETRY: Duration = Duration::from_secs(60);

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim().trim_start_matches('v').split(['.', '-', '+']);
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    ))
}

/// 앱 시작 점검(`probe_runtime`)이 읽은 버전을 캐시에 넣는다
pub(super) fn remember_kordoc_version(raw: &str) {
    if let (Some(v), Ok(mut cache)) = (parse_version(raw), KORDOC_VERSION.lock()) {
        cache.value = Some(v);
    }
}

fn kordoc_version(cli_path: &Path) -> Option<(u32, u32, u32)> {
    let mut cache = KORDOC_VERSION.lock().ok()?;
    if cache.value.is_some() {
        return cache.value;
    }
    if cache.failed_at.is_some_and(|t| t.elapsed() < VERSION_RETRY) {
        return None;
    }
    // 잠근 채로 잰다 — 색인 스레드들이 한꺼번에 --version 을 띄우지 않게
    let version = run_kordoc_process(cli_path, &["--version".into()], 15, "version")
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| parse_version(&String::from_utf8_lossy(&out.stdout)));
    match version {
        Some(v) => cache.value = Some(v),
        None => cache.failed_at = Some(Instant::now()),
    }
    version
}

/// `--no-images`·`parse-worker` 가 있는 kordoc (4.14.3+). 버전을 못 읽으면 종전 방식.
fn supports_lean_parse(cli_path: &Path) -> bool {
    kordoc_version(cli_path).is_some_and(|v| v >= (4, 14, 3))
}

/// kordoc CLI 동기 호출 — 파싱 경로 (`--format json --silent`), stdout 의 JSON 을 반환.
/// 4.14.3+ 이면 상주 워커를 먼저 쓰고(워커를 못 쓰면 1회성 실행), 이미지 바이트는 받지 않는다.
/// `interactive`(미리보기)는 워커가 모두 바쁘면 기다리지 않는다.
pub(super) fn call_kordoc_sync(
    cli_path: &Path,
    file_path: &Path,
    #[cfg_attr(feature = "online", allow(unused_mut))] mut opts: KordocOptions,
    interactive: bool,
) -> Result<String, ParseError> {
    // lite(내부망) 빌드의 마지막 관문. `--ocr`/`--ocr-force`/`--formula-ocr` 는 kordoc 이
    // 첫 사용 시 HuggingFace 에서 모델(텍스트 OCR ~18MB, 수식 ~155MB)을 직접 내려받게 만든다.
    // 그 다운로드는 Rust 쪽 `DOCUFINDER_OFFLINE` 스위치가 닿지 않는 자식 프로세스에서 일어나므로
    // (앱이 자식을 띄워 외부 바이너리를 받는 = 전형적 dropper 패턴), 플래그를 만드는 지점이
    // 아니라 **CLI 인자를 조립하는 이 한 곳**에서 잘라 우회 경로가 남지 않게 한다.
    #[cfg(not(feature = "online"))]
    {
        opts.formula_ocr = false;
        opts.ocr = KordocOcrMode::Off;
    }

    // Windows extended-length / UNC prefix 제거 (Node.js/kordoc가 처리하지 못함).
    // 단순 strip("\\?\\") 만 하면 \\?\UNC\server\share\... 가 UNC\server\... 로 깨지므로
    // dunce::simplified 로 \\srv\share\... 형태까지 정확히 복원한다.
    let file_owned = crate::utils::network_path::simplify(file_path);
    let file_str = file_owned.to_string_lossy();

    let mut args: Vec<std::ffi::OsString> = vec![
        file_str.as_ref().into(),
        "--format".into(),
        "json".into(),
        "--silent".into(),
    ];
    if opts.formula_ocr {
        args.push("--formula-ocr".into());
    }
    match opts.ocr {
        KordocOcrMode::Off => {}
        KordocOcrMode::Auto => args.push("--ocr".into()),
        KordocOcrMode::Force => args.push("--ocr-force".into()),
    }
    if let Some(pw) = opts.password.as_deref() {
        args.push("--password".into());
        args.push(pw.into());
    }

    // 텍스트 OCR 도 수식 OCR 과 같은 사유(모델 로드 + 페이지별 ONNX 추론)로 장시간 타임아웃.
    let timeout_secs = if opts.formula_ocr || opts.ocr != KordocOcrMode::Off {
        KORDOC_FORMULA_TIMEOUT_SECS
    } else {
        KORDOC_TIMEOUT_SECS
    };

    if supports_lean_parse(cli_path) {
        if let Some(result) =
            super::worker::parse_via_pool(file_path, &opts, timeout_secs, interactive)
        {
            return result;
        }
        // 앱은 문서 속 그림을 쓰지 않는다 — 그림 많은 PDF 는 base64 이미지가 출력의 대부분이었다.
        // 워커를 못 띄운 kordoc 은 이 옵션도 몰라 모든 파싱이 실패하므로 뺀다.
        if !super::worker::pool_disabled() {
            args.push("--no-images".into());
        }
    }

    debug!(
        "kordoc: {} {:?}",
        cli_path.display(),
        args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>()
    );

    let out = run_kordoc_process(
        cli_path,
        &args,
        timeout_secs,
        &file_path.display().to_string(),
    )?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        warn!("kordoc failed (exit {}): {}", out.status, stderr);
        // kordoc 은 실패해도 stdout 에 실패 JSON(success:false + code)을 내고 exit 1 한다(#69).
        // 그대로 넘겨야 호출부가 원인 코드로 분기한다. 버리면 ENCRYPTED 가 일반 에러로 뭉개져
        // 미리보기 비밀번호 입력이 뜨지 않는다.
        if let Some(json) = failure_json(&out.stdout) {
            return Ok(json);
        }
        // "이미지 기반 PDF"는 kordoc이 본문 없는 스캔 PDF를 만났을 때 내는 마커.
        // parse_file 이 OCR 여부 보고 Rust 재시도를 건너뛸 수 있게 에러 문자열에 태그 유지.
        if stderr.contains("이미지 기반 PDF") {
            return Err(ParseError::ParseError(format!(
                "kordoc: 이미지 기반 PDF (exit {})",
                out.status
            )));
        }
        let snippet = stderr_snippet(&stderr);
        return Err(ParseError::ParseError(if snippet.is_empty() {
            format!("kordoc 실행 실패 (exit {})", out.status)
        } else {
            format!("kordoc 실행 실패 (exit {}): {snippet}", out.status)
        }));
    }

    // kordoc 출력 크기 제한 (100MB — OOM 방지)
    const MAX_OUTPUT_SIZE: usize = 100 * 1024 * 1024;
    if out.stdout.len() > MAX_OUTPUT_SIZE {
        return Err(ParseError::ParseError(format!(
            "kordoc 출력 크기 초과: {}MB (최대 {}MB)",
            out.stdout.len() / 1_048_576,
            MAX_OUTPUT_SIZE / 1_048_576
        )));
    }

    let output = String::from_utf8(out.stdout)
        .map_err(|_| ParseError::ParseError("kordoc 출력이 유효한 UTF-8이 아닙니다".to_string()))?;

    // pdfjs-dist 등 외부 라이브러리가 stdout에 경고를 출력하는 경우
    // JSON 시작점을 찾아서 앞의 garbage를 제거 (예: "Warning: TT: ...")
    let json_start = output
        .find('{')
        .ok_or_else(|| ParseError::ParseError("kordoc 출력에 JSON이 없습니다".to_string()))?;
    if json_start > 0 {
        // 200번째 바이트가 멀티바이트 문자 중간이면 slice가 panic — 문자 경계로 내림
        let mut preview_end = json_start.min(200);
        while !output.is_char_boundary(preview_end) {
            preview_end -= 1;
        }
        debug!(
            "kordoc stdout에 JSON 앞 {}바이트 garbage 제거: {:?}",
            json_start,
            &output[..preview_end]
        );
    }
    Ok(output[json_start..].to_string())
}

/// 실패한 kordoc 의 stdout 에서 실패 JSON(`"success": false`)을 꺼낸다. 없으면 None.
pub(super) fn failure_json(stdout: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(stdout).ok()?;
    let json = &text[text.find('{')?..];
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    (!v.get("success")?.as_bool()?).then(|| json.to_string())
}
