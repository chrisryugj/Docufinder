//! Windows COM 자동화 기반 Office 문서 파서 (Windows 전용)
//!
//! 설치된 MS Office (Word · PowerPoint · Excel) 를 COM IDispatch 지연 바인딩으로
//! 구동해 문서 본문을 추출한다. Rust ZIP/XML 파서(`parsers::docx` 등) 가 처리하지
//! 못하는 레거시 포맷(.doc, .ppt, .xls) 이나 손상된 OOXML 의 fallback 경로로
//! `parsers/mod.rs` 의 `wincom_fallback_*` 래퍼에서 호출된다.
//!
//! 핵심 설계:
//! - 모든 COM 호출은 별도 STA(단일 스레드 아파트먼트) 스레드에서 실행 —
//!   Office COM 은 STA 를 요구하며 `parse_with_timeout` 래퍼와 짝을 이룸.
//! - 암호 문서 대비: Open 시 항상 가짜 암호(`BOGUS_PASSWORD`) 전달 →
//!   보호된 문서는 즉시 password 에러로 실패 (시스템 모달 다이얼로그 차단),
//!   비보호 문서는 무시됨.
//! - 매크로 보안: `AutomationSecurity = ForceDisable` 로 Open 시 매크로 자동실행 차단.

pub mod docx;
pub mod pptx;
pub mod xlsx;

use std::path::Path;

use windows::core::{BSTR, GUID, PCWSTR};
use windows::Win32::System::Com::{
    CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch,
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, DISPATCH_FLAGS, DISPATCH_METHOD,
    DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS,
};
use windows::Win32::System::Ole::DISPID_PROPERTYPUT;
use windows::Win32::System::Variant::{
    VariantChangeType, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VAR_CHANGE_FLAGS, VT_ARRAY,
    VT_BSTR, VT_EMPTY, VT_ERROR, VT_NULL,
};

use crate::parsers::ParseError;

// ============================================================================
// Office 자동화 상수
// ============================================================================

/// `IDispatch::Invoke` 호출 시 사용하는 lcid — 시스템 기본 로케일.
const LOCALE_USER_DEFAULT: u32 = 0x0400;

/// MsoTriState — Office 자동화의 불리언 값 (msoTrue = -1, msoFalse = 0).
const MSO_TRUE: i32 = -1;
const MSO_FALSE: i32 = 0;

/// `MsoAutomationSecurity::msoAutomationSecurityForceDisable` —
/// 파일 열 때 매크로 자동 실행을 완전히 차단 (매크로 바이러스 방어).
const MSO_AUTOMATION_SECURITY_FORCE_DISABLE: i32 = 3;

/// Word 가 암호 불일치 시 반환하는 HRESULT 값.
const WORD_ERR_WRONG_PASSWORD: i32 = 0x800A_1533u32 as i32;

/// 보호된 문서에 전달하는 가짜 암호 — Open 호출이 모달 다이얼로그를 띄우지 않고
/// 즉시 password 에러로 실패하도록 유도 (`to_parse_error` 가 `PasswordProtected` 로 변환).
/// 비보호 문서에서는 Word 가 이 값을 무시한다.
const BOGUS_PASSWORD: &str = "\u{1}docufinder-com-noopen\u{1}";

// ============================================================================
// COM 아파트먼트 가드
// ============================================================================

/// COM 호출을 위한 단일 스레드 아파트먼트(STA) RAII 가드.
///
/// Office COM 자동화는 STA 를 요구한다. `CoInitializeEx` 로 아파트먼트를 초기화하고
/// drop 시 `CoUninitialize` 로 정리한다. 이미 초기화된 스레드에서 재호출 시
/// `S_FALSE` 를 반환하므로, `should_uninit` 플래그로 중복 해제를 방지한다.
pub(crate) struct ComApartment {
    should_uninit: bool,
}

impl ComApartment {
    /// 현재 스레드를 STA 로 초기화. 이미 초기화된 경우 uninitialize 하지 않는다.
    pub(crate) fn init() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        ComApartment {
            should_uninit: hr.is_ok(),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

// ============================================================================
// IDispatch 래퍼
// ============================================================================

/// COM 자동화 객체(`IDispatch`) 래퍼 — VBA 스타일 지연 바인딩(late-binding) 호출.
#[derive(Clone)]
pub(crate) struct Obj(IDispatch);

impl Obj {
    /// `prog_id` (예: "Word.Application") 로 최상위 Application 객체 생성.
    pub(crate) fn create(prog_id: &str) -> windows::core::Result<Obj> {
        let wide = to_wide(prog_id);
        let clsid: GUID = unsafe { CLSIDFromProgID(PCWSTR(wide.as_ptr())) }?;
        let disp: IDispatch = unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER) }?;
        Ok(Obj(disp))
    }

    /// 메서드/속성 이름 → DISPID 조회 (`IDispatch::GetIDsOfNames`).
    fn dispid(&self, name: &str) -> windows::core::Result<i32> {
        let wide = to_wide(name);
        let names = [PCWSTR(wide.as_ptr())];
        let mut id = 0i32;
        unsafe {
            self.0.GetIDsOfNames(
                &GUID::zeroed(),
                names.as_ptr(),
                1,
                LOCALE_USER_DEFAULT,
                &mut id,
            )
        }?;
        Ok(id)
    }

    /// `IDispatch::Invoke` 저수준 래퍼.
    /// 인자를 역순으로 배치(VARIANT 역순 규약)하며, property put 시
    /// `DISPID_PROPERTYPUT` named arg 를 설정한다.
    fn invoke(
        &self,
        name: &str,
        flags: DISPATCH_FLAGS,
        args: &[VARIANT],
    ) -> windows::core::Result<VARIANT> {
        let dispid = self.dispid(name)?;

        let mut rev: Vec<VARIANT> = args.iter().rev().cloned().collect();
        let mut named_put = DISPID_PROPERTYPUT;

        let mut params = DISPPARAMS::default();
        if !rev.is_empty() {
            params.rgvarg = rev.as_mut_ptr();
            params.cArgs = rev.len() as u32;
        }
        if flags == DISPATCH_PROPERTYPUT {
            params.cNamedArgs = 1;
            params.rgdispidNamedArgs = &mut named_put;
        }

        let mut result = VARIANT::default();
        unsafe {
            self.0.Invoke(
                dispid,
                &GUID::zeroed(),
                LOCALE_USER_DEFAULT,
                flags,
                &params,
                Some(&mut result),
                None,
                None,
            )
        }?;
        Ok(result)
    }

    /// 메서드 호출 (예: Open, Close, Quit).
    pub(crate) fn call(&self, name: &str, args: &[VARIANT]) -> windows::core::Result<VARIANT> {
        self.invoke(name, DISPATCH_METHOD, args)
    }

    /// 속성 읽기 (예: Count, Text, Item(i)).
    pub(crate) fn get(&self, name: &str, args: &[VARIANT]) -> windows::core::Result<VARIANT> {
        self.invoke(name, DISPATCH_PROPERTYGET | DISPATCH_METHOD, args)
    }

    /// 속성 쓰기 (예: Visible, DisplayAlerts).
    pub(crate) fn put(&self, name: &str, value: VARIANT) -> windows::core::Result<()> {
        self.invoke(name, DISPATCH_PROPERTYPUT, &[value])?;
        Ok(())
    }

    /// 자식 객체 획득 (예: Documents, Content, Slides, UsedRange).
    pub(crate) fn get_obj(&self, name: &str, args: &[VARIANT]) -> windows::core::Result<Obj> {
        as_obj(&self.get(name, args)?)
    }

    /// 정수 속성 읽기 — 실패 시 0.
    pub(crate) fn get_i32(&self, name: &str, args: &[VARIANT]) -> i32 {
        self.get(name, args).map(|v| v_i32(&v)).unwrap_or(0)
    }

    /// 문자열 속성 읽기 — 실패 시 빈 문자열.
    pub(crate) fn get_string(&self, name: &str, args: &[VARIANT]) -> String {
        self.get(name, args)
            .map(|v| v_string(&v))
            .unwrap_or_default()
    }
}

// ============================================================================
// VARIANT 변환 유틸리티
// ============================================================================

/// 결과 VARIANT(`VT_DISPATCH`) 에서 자식 Obj 추출.
pub(crate) fn as_obj(v: &VARIANT) -> windows::core::Result<Obj> {
    Ok(Obj(IDispatch::try_from(v)?))
}

/// VARIANT → i32 (숫자/불리언/문자 강제 변환), 실패 시 0.
pub(crate) fn v_i32(v: &VARIANT) -> i32 {
    i32::try_from(v).unwrap_or(0)
}

/// VARIANT → String (`VT_BSTR` 및 강제 변환 가능 타입).
pub(crate) fn v_string(v: &VARIANT) -> String {
    BSTR::try_from(v).map(|b| b.to_string()).unwrap_or_default()
}

/// 임의 스칼라 VARIANT → 로케일 문자열 (날짜·통화 등 `VariantChangeType` 사용).
pub(crate) fn v_to_display_string(v: &VARIANT) -> String {
    let mut dest = VARIANT::default();
    let ok = unsafe { VariantChangeType(&mut dest, v, VAR_CHANGE_FLAGS(0), VT_BSTR).is_ok() };
    if ok {
        v_string(&dest)
    } else {
        String::new()
    }
}

/// `VT_ARRAY`(`SAFEARRAY`) 여부 확인.
pub(crate) fn v_is_array(v: &VARIANT) -> bool {
    (v.vt().0 & VT_ARRAY.0) != 0
}

/// `VT_EMPTY` 또는 `VT_NULL` 여부 확인.
pub(crate) fn v_is_empty(v: &VARIANT) -> bool {
    let vt = v.vt();
    vt == VT_EMPTY || vt == VT_NULL
}

/// 생략된 선택 인자 — positional 인자 사이의 빈 칸을 채운다.
/// COM 자동화에서 `Workbooks.Open` 의 Format 인자처럼 생략하는 파라미터는
/// `VT_ERROR` + `DISP_E_PARAMNOTFOUND` 로 표현한다.
const DISP_E_PARAMNOTFOUND: i32 = 0x8002_0004u32 as i32;
pub(crate) fn missing_arg() -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: core::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_ERROR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    scode: DISP_E_PARAMNOTFOUND,
                },
            }),
        },
    }
}

// ============================================================================
// VARIANT 생성 헬퍼
// ============================================================================

pub(crate) fn var_str(s: &str) -> VARIANT {
    VARIANT::from(s)
}
pub(crate) fn var_i32(n: i32) -> VARIANT {
    VARIANT::from(n)
}
pub(crate) fn var_bool(b: bool) -> VARIANT {
    VARIANT::from(b)
}

/// UTF-16 널 종단 wide 문자열 변환 (COM API 호출용).
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ============================================================================
// Application 객체 수명 가드
// ============================================================================

pub(crate) enum AppKind {
    Word,
    Excel,
    PowerPoint,
}

impl AppKind {
    fn collection_name(&self) -> &'static str {
        match self {
            Self::Word => "Documents",
            Self::Excel => "Workbooks",
            Self::PowerPoint => "Presentations",
        }
    }
}

/// Application 객체의 RAII 가드 — 우리가 바꾼 앱 전역 설정을 되돌리고,
/// 열린 문서가 남지 않은 인스턴스만 `Quit` 으로 종료한다.
///
/// Word·PowerPoint 는 단일 인스턴스 자동화 서버라 사용자가 앱을 이미 켜 둔 상태면
/// `CoCreateInstance` 가 새 프로세스를 띄우는 대신 **그 인스턴스에 붙는다**. 이때
/// 무조건 `Quit` 하면 사용자가 편집 중이던 문서까지 닫히고(`DisplayAlerts` 를 꺼 둔
/// 탓에 저장 확인 없이), `Visible=false` · `ScreenUpdating=false` 같은 전역 설정도
/// 그대로 남아 앱이 먹통처럼 보인다. 그래서
/// ① `put_scoped` 로 바꾼 값은 drop 에서 원래 값으로 되돌리고,
/// ② 열린 문서 수가 0 일 때만 Quit 한다 (우리가 연 문서는 그전에 Close 된다).
/// 조회 실패 시에도 Quit 하지 않는다 — 이미 죽은 인스턴스라 Quit 도 실패한다.
pub(crate) struct AppGuard {
    app: Obj,
    kind: AppKind,
    /// `put_scoped` 로 덮어쓴 속성의 원래 값 (설정한 순서대로).
    saved: Vec<(&'static str, VARIANT)>,
}

impl AppGuard {
    pub(crate) fn new(app: &Obj, kind: AppKind) -> Self {
        AppGuard {
            app: app.clone(),
            kind,
            saved: Vec::new(),
        }
    }

    /// 앱 전역 속성을 원래 값으로 백업한 뒤 새 값으로 설정한다 (drop 에서 복원).
    /// 읽기가 실패하는 속성(해당 앱이 지원하지 않음)은 설정만 하고 복원 대상에서 뺀다.
    pub(crate) fn put_scoped(&mut self, name: &'static str, value: VARIANT) {
        if let Ok(original) = self.app.get(name, &[]) {
            self.saved.push((name, original));
        }
        let _ = self.app.put(name, value);
    }

    fn collection_size(&self) -> windows::core::Result<i32> {
        let collection = self.app.get_obj(self.kind.collection_name(), &[])?;
        let count = collection.get("Count", &[])?;
        i32::try_from(&count)
    }
}

impl Drop for AppGuard {
    fn drop(&mut self) {
        let saved = std::mem::take(&mut self.saved);
        for (name, original) in saved.into_iter().rev() {
            let _ = self.app.put(name, original);
        }
        if matches!(self.collection_size(), Ok(0)) {
            let _ = self.app.call("Quit", &[]);
        }
    }
}

// ============================================================================
// 에러 변환 · 경로 헬퍼
// ============================================================================

/// `windows::core::Error` → `ParseError` 변환.
/// HRESULT 또는 메시지에서 암호 관련 키워드(password, 암호, encrypt) 가 발견되면
/// `PasswordProtected` 로, 그 외는 일반 COM 오류로 매핑한다.
pub(crate) fn to_parse_error(context: &str, e: &windows::core::Error) -> ParseError {
    let code = e.code().0;
    let msg = e.message();
    let lower = msg.to_lowercase();
    if code == WORD_ERR_WRONG_PASSWORD
        || lower.contains("password")
        || lower.contains("암호")
        || lower.contains("encrypt")
    {
        ParseError::PasswordProtected(format!("암호로 보호된 문서입니다 ({context})"))
    } else {
        ParseError::ParseError(format!("{context} COM 오류: {msg} (0x{:08X})", code as u32))
    }
}

/// 파일 경로를 COM `Open` 호출에 넘길 절대 경로 문자열로 정규화.
/// `dunce` 로 UNC `\\?\` prefix 를 단순화 — Office 가 prefix 를 못 읽는 경우 방지.
pub(crate) fn path_arg(path: &Path) -> String {
    std::fs::canonicalize(path)
        .ok()
        .and_then(|p| dunce::simplified(&p).to_str().map(String::from))
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
