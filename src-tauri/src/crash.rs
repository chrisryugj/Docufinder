//! 크래시 기록: 패닉 훅(crash-날짜.log, Telegram 전송)과 Tauri 시작 실패 로그.

pub(crate) fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        // 파서 라이브러리의 알려진 패닉은 catch_unwind로 처리됨 → crash.log 오염 방지.
        // 해당 파일은 에러로 스킵되고 앱은 정상 동작하므로 crash 기록 불필요.
        // 패턴은 `panic_filter` 모듈에서 공유 — deferred flush(telemetry) 에서도 같은 필터 사용.
        if crate::panic_filter::is_benign_location(&location) {
            return;
        }

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        // 처리 중이던 파일/단계 (있으면 메시지 끝에 덧붙임 — 향후 디버깅에 결정적)
        let breadcrumb_line = crate::breadcrumb::snapshot()
            .as_ref()
            .map(crate::breadcrumb::format_for_log);

        eprintln!("╔══════════════════════════════════════════════════════════╗");
        eprintln!("║                    CRITICAL ERROR                        ║");
        eprintln!("╚══════════════════════════════════════════════════════════╝");
        eprintln!("Location: {}", location);
        eprintln!("Message: {}", message);
        if let Some(bc) = &breadcrumb_line {
            eprintln!("{}", bc);
        }
        eprintln!("Please contact the development team to report this issue.");

        // Telegram 자동 전송 (빌드 시 토큰 주입 + 사용자의 error_reporting_enabled
        // 양쪽을 통과할 때만 — report_panic_sync 내부에서 검사한다).
        let telegram_msg = match &breadcrumb_line {
            Some(bc) => format!("{message} | {bc}"),
            None => message.clone(),
        };
        crate::commands::telemetry::report_panic_sync(&location, &telegram_msg);

        // 긴급 로그 flush — 날짜 기반 로테이션 (최대 3개 파일 유지)
        if let Some(data_dir) = dirs::data_dir() {
            let crash_dir = data_dir.join(crate::APP_IDENTIFIER);
            let _ = std::fs::create_dir_all(&crash_dir);

            // 날짜별 crash log 파일
            let today = chrono::Local::now().format("%Y-%m-%d");
            let crash_log = crash_dir.join(format!("crash-{}.log", today));

            // 오래된 crash log 정리 (최대 3개 유지)
            const MAX_CRASH_LOGS: usize = 3;
            if let Ok(entries) = std::fs::read_dir(&crash_dir) {
                let mut crash_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
                    .collect();
                crash_files.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
                for old_file in crash_files.into_iter().skip(MAX_CRASH_LOGS) {
                    let _ = std::fs::remove_file(old_file.path());
                }
            }

            // 단일 파일 크기 제한 (1MB)
            const MAX_CRASH_LOG_SIZE: u64 = 1024 * 1024;
            if let Ok(meta) = std::fs::metadata(&crash_log) {
                if meta.len() > MAX_CRASH_LOG_SIZE {
                    let _ = std::fs::remove_file(&crash_log);
                }
            }

            let entry = match &breadcrumb_line {
                Some(bc) => format!(
                    "[{}] PANIC at {}: {}\n  {}\n",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    location,
                    message,
                    bc
                ),
                None => format!(
                    "[{}] PANIC at {}: {}\n",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    location,
                    message
                ),
            };
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&crash_log)
            {
                let _ = file.write_all(entry.as_bytes());
                let _ = file.sync_all(); // 전원 차단 시 유실 방지
            }
        }
    }));
}

pub(crate) fn exit_on_start_failure(e: tauri::Error) {
    eprintln!("Fatal: Tauri failed to start: {}", e);
    // 크래시 로그에도 기록 (append 모드: 이전 기록 보존)
    if let Some(data_dir) = dirs::data_dir() {
        let crash_dir = data_dir.join(crate::APP_IDENTIFIER);
        let _ = std::fs::create_dir_all(&crash_dir);
        let crash_log = crash_dir.join("crash.log");
        // 크기 제한: 1MB 초과 시 truncate
        const MAX_CRASH_LOG_SIZE: u64 = 1024 * 1024;
        if let Ok(meta) = std::fs::metadata(&crash_log) {
            if meta.len() > MAX_CRASH_LOG_SIZE {
                let _ = std::fs::remove_file(&crash_log);
            }
        }
        let entry = format!(
            "[{}] FATAL: Tauri failed to start: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            e
        );
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
        {
            let _ = file.write_all(entry.as_bytes());
        }
    }
    std::process::exit(1);
}
