//! 디스크 유형 감지 (SSD/HDD)
//!
//! Windows에서 WMI를 통해 디스크 유형을 감지하거나,
//! 드라이브 문자로 추정 (C:=SSD, D:=HDD 패턴).
//! 결과는 드라이브별로 캐싱되어 PowerShell 재실행 방지.

use std::collections::HashMap;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// 콘솔 창 숨김 플래그 (Windows)
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 디스크 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskType {
    Ssd,
    Hdd,
    Unknown,
}

impl DiskType {
    /// HDD인지 여부 (Unknown도 안전하게 HDD로 처리)
    pub fn is_hdd(&self) -> bool {
        matches!(self, DiskType::Hdd | DiskType::Unknown)
    }
}

/// 디스크 유형 캐시 (드라이브별 1회만 감지)
static DISK_TYPE_CACHE: OnceLock<Mutex<HashMap<char, DiskType>>> = OnceLock::new();

/// 경로에서 드라이브 문자 추출 (Windows)
/// `\\?\C:\...`, `C:\...`, `c:\...` 모두 지원
fn get_drive_letter(path: &Path) -> Option<char> {
    let s = path.to_str()?;

    // \\?\ 접두사 제거 후 드라이브 문자 추출
    let normalized = s.strip_prefix(r"\\?\").unwrap_or(s);

    normalized
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic())
}

/// WMI로 디스크 유형 조회 (Windows PowerShell)
#[cfg(windows)]
fn query_disk_type_wmi(drive_letter: char) -> Option<DiskType> {
    // PowerShell 명령으로 MediaType 조회
    let script = format!(
        r#"
        $disk = Get-PhysicalDisk | Where-Object {{
            $partitions = Get-Partition -DiskNumber $_.DeviceId -ErrorAction SilentlyContinue
            $partitions.DriveLetter -contains '{}'
        }} | Select-Object -First 1
        if ($disk) {{ $disk.MediaType }} else {{ 'Unknown' }}
        "#,
        drive_letter.to_ascii_uppercase()
    );

    let encoded = super::encode_powershell_command(&script);
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    let result = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_lowercase();

    match result.as_str() {
        "ssd" => Some(DiskType::Ssd),
        "hdd" => Some(DiskType::Hdd),
        _ => None,
    }
}

#[cfg(not(windows))]
fn query_disk_type_wmi(_drive_letter: char) -> Option<DiskType> {
    None
}

/// 드라이브 문자로 디스크 유형 추정 (fallback)
/// 일반적 패턴: C: = SSD (OS), D: 이후 = HDD
fn guess_disk_type_by_letter(drive_letter: char) -> DiskType {
    match drive_letter.to_ascii_uppercase() {
        'C' => DiskType::Ssd,
        _ => DiskType::Hdd,
    }
}

/// 경로의 디스크 유형 감지
///
/// 1. 캐시에서 조회 (히트 시 즉시 반환)
/// 2. 캐시 미스 시 fallback 즉시 반환 + 백그라운드 WMI 업데이트
///    → DB 연결 등 startup 경로에서 PowerShell hang 방지
pub fn detect_disk_type(path: &Path) -> DiskType {
    let drive_letter = match get_drive_letter(path) {
        Some(c) => c.to_ascii_uppercase(),
        None => return DiskType::Unknown,
    };

    // 캐시 확인
    let cache = DISK_TYPE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(&cached) = map.get(&drive_letter) {
            tracing::debug!("Disk type for {}: {:?} (cached)", drive_letter, cached);
            return cached;
        }
    }

    // 캐시 미스 → fallback 즉시 반환 (WMI는 백그라운드에서 캐시 업데이트)
    let guessed = guess_disk_type_by_letter(drive_letter);
    tracing::debug!(
        "Disk type for {}: {:?} (guessed, WMI pending)",
        drive_letter,
        guessed
    );

    // 백그라운드에서 WMI 조회 후 캐시 갱신 (다음 호출부터 정확한 값 사용)
    std::thread::spawn(move || {
        if let Some(wmi_type) = query_disk_type_wmi(drive_letter) {
            let cache = DISK_TYPE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
            if let Ok(mut map) = cache.lock() {
                map.insert(drive_letter, wmi_type);
                tracing::debug!(
                    "Disk type for {}: {:?} (WMI updated)",
                    drive_letter,
                    wmi_type
                );
            }
        }
    });

    guessed
}

/// 디스크 유형에 따른 권장 설정
#[derive(Debug, Clone)]
pub struct DiskSettings {
    /// 파일 처리 간 대기 시간 (ms)
    pub throttle_ms: u64,
    /// 병렬 파싱 스레드 수 (0 = 비활성화)
    pub parallel_threads: usize,
}

impl DiskSettings {
    /// 디스크 유형에 따른 기본 설정
    pub fn for_disk_type(disk_type: DiskType) -> Self {
        match disk_type {
            DiskType::Ssd => Self {
                throttle_ms: 0,
                parallel_threads: num_cpus::get().min(4),
            },
            DiskType::Hdd | DiskType::Unknown => Self {
                throttle_ms: 10,     // HDD 부하 최소화 (50ms → 10ms)
                parallel_threads: 2, // 약간의 병렬 허용
            },
        }
    }
}

/// CPU 코어 수 반환 (num_cpus 없으면 기본값)
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_drive_letter() {
        assert_eq!(get_drive_letter(Path::new("C:\\Users")), Some('C'));
        assert_eq!(get_drive_letter(Path::new("D:\\Data")), Some('D'));
        // `/`는 is_ascii_alphabetic() false → None
        assert_eq!(get_drive_letter(Path::new("/home/user")), None);
        // \\?\ 접두사 처리
        assert_eq!(get_drive_letter(Path::new(r"\\?\C:\Users")), Some('C'));
        assert_eq!(get_drive_letter(Path::new(r"\\?\E:\Data")), Some('E'));
    }

    #[test]
    fn test_guess_disk_type() {
        assert_eq!(guess_disk_type_by_letter('C'), DiskType::Ssd);
        assert_eq!(guess_disk_type_by_letter('D'), DiskType::Hdd);
    }
}
