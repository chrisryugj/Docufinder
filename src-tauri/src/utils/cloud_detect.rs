//! 클라우드 placeholder 파일 감지 (OneDrive / iCloud / Dropbox / Google Drive 등).
//!
//! Windows 의 Cloud Files API 는 "파일이 클라우드에만 있고 로컬엔 메타데이터만 있는" 상태를
//! 파일 속성 비트로 표시한다. 이 비트가 켜진 파일을 `fs::read()` 로 열면 OS 가
//! 투명하게 원본을 다운로드(hydrate)해 버리므로 — 인덱서가 수백 GB 의 클라우드 파일을
//! 끌어오는 사고를 일으킨다.
//!
//! 본 모듈은 그 비트를 검사해 "내용 파싱은 건너뛰되 파일명/메타데이터는 인덱싱"
//! 분기를 가능하게 한다.

#[cfg(windows)]
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
#[cfg(windows)]
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
#[cfg(windows)]
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

#[cfg(windows)]
const CLOUD_MASK: u32 =
    FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;

/// 경로가 클라우드 placeholder(로컬 미존재, 액세스 시 다운로드 트리거)인지 검사한다.
///
/// Windows: 파일 속성 비트로 판정.
/// 비-Windows: 항상 false (해당 OS 는 동일 시맨틱 미지원).
///
/// 메타데이터 조회 자체는 hydrate 를 트리거하지 않는다(`GetFileAttributes` 류 호출).
pub fn is_cloud_placeholder(path: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        match std::fs::symlink_metadata(path) {
            Ok(meta) => (meta.file_attributes() & CLOUD_MASK) != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}
