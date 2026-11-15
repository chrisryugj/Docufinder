//! 창 불러오기 전역 단축키 (이슈 #52).
//!
//! 어디서든 단축키를 누르면 창을 앞으로 띄우고, 창이 이미 앞에 있으면 내린다.
//! Windows `RegisterHotKey` / macOS `RegisterEventHotKey` 기반이라 키보드 훅을 걸지 않는다 —
//! "Ctrl 두 번" 같은 방식은 모든 키 입력을 가로채는 훅(WH_KEYBOARD_LL)이 필요하고, 보안솔루션이
//! 키로거로 잡는 동작이라 쓰지 않는다(docs/LITE-BUILD.md 의 탐지 표면 최소화 원칙).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::AppContainer;

/// 메인 창 포커스 — `WindowEvent::Focused` 로 갱신한다. Windows 에서 `is_focused()` 는 최상위 창
/// 기준이라 포커스가 WebView2 자식 창으로 넘어가면 false 가 되어, 앞에 있는 창을 내리지 못한다.
/// Tauri 는 Focused 이벤트를 WebView 포커스 기준으로 보내므로 그 값을 쓴다.
static MAIN_FOCUSED: AtomicBool = AtomicBool::new(false);

pub fn set_main_focused(focused: bool) {
    MAIN_FOCUSED.store(focused, Ordering::Relaxed);
}

pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_main_window(app);
            }
        })
        .build()
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let in_front = window.is_visible().unwrap_or(false)
        && MAIN_FOCUSED.load(Ordering::Relaxed)
        && !window.is_minimized().unwrap_or(false);
    if !in_front {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    // 내릴 때는 X 버튼과 같은 규칙 — 트레이 최소화를 켠 사용자만 트레이로 숨긴다.
    let close_to_tray = app
        .try_state::<RwLock<AppContainer>>()
        .and_then(|s| s.read().ok().map(|c| c.get_settings().close_to_tray))
        .unwrap_or(false);
    if close_to_tray {
        let _ = window.hide();
    } else {
        let _ = window.minimize();
    }
}

fn parse(accelerator: &str) -> Result<Shortcut, String> {
    accelerator
        .parse::<Shortcut>()
        .map_err(|e| format!("단축키 형식을 읽지 못했습니다 ({accelerator}): {e}"))
}

fn non_empty(accelerator: Option<&str>) -> Option<&str> {
    accelerator.map(str::trim).filter(|s| !s.is_empty())
}

/// `prev` 단축키를 풀고 `next` 를 건다. None·빈 값은 "없음".
/// 다른 프로그램이 같은 조합을 이미 쓰면 등록이 실패한다 — `prev` 를 다시 걸고 사유를 돌려준다.
/// 전부 풀기(unregister_all) 대신 하나만 푼다: 플러그인의 unregister_all 은 내부 잠금을 쥔 채 메인
/// 스레드를 기다려, 바로 그때 눌린 단축키 처리(메인 스레드)와 서로 기다릴 수 있다.
pub fn apply<R: Runtime>(
    app: &AppHandle<R>,
    prev: Option<&str>,
    next: Option<&str>,
) -> Result<(), String> {
    let gs = app.global_shortcut();
    let prev = non_empty(prev).and_then(|p| parse(p).ok());
    if let Some(p) = prev {
        let _ = gs.unregister(p);
    }
    let Some(acc) = non_empty(next) else {
        return Ok(());
    };
    let result = parse(acc).and_then(|s| {
        gs.register(s).map_err(|e| {
            format!("단축키 {acc} 를 등록하지 못했습니다 — 다른 프로그램이 이미 쓰고 있을 수 있습니다. 다른 조합을 골라 주세요. ({e})")
        })
    });
    match &result {
        Ok(()) => tracing::info!("전역 단축키 등록: {acc}"),
        Err(_) => {
            if let Some(p) = prev {
                let _ = gs.register(p);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::parse;

    /// 설정 창 녹화기(src/utils/shortcut.ts)는 KeyboardEvent.code 이름을 그대로 저장한다 — 파서가 받는지.
    #[test]
    fn recorder_accelerators_parse() {
        for acc in [
            "Ctrl+Alt+KeyK",
            "Ctrl+Shift+Digit5",
            "Alt+Backquote",
            "Shift+Super+F13",
            "Ctrl+Alt+Shift+Super+KeyK", // 녹화기 출력 순서 그대로
            "Ctrl+Alt+ArrowUp",
            "Ctrl+Alt+Numpad7",
            "Ctrl+Alt+NumpadAdd",
            "Ctrl+Alt+BracketLeft",
            "Ctrl+Alt+Quote",
            "Ctrl+Alt+PageDown",
            // 프리셋
            "Alt+`",
            "Ctrl+Alt+Space",
            "Alt+Shift+Space",
        ] {
            assert!(parse(acc).is_ok(), "{acc}");
        }
        // 녹화기가 거르는 키(한/영 등)는 파서도 모른다
        assert!(parse("Ctrl+Alt+Lang1").is_err());
    }
}
