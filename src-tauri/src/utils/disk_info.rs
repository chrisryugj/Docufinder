//! 디스크 유형 감지 (SSD/HDD)
//!
//! Windows에서 WMI를 통해 디스크 유형을 감지하거나,
//! 드라이브 문자로 추정 (C:=SSD, D:=HDD 패턴).

use std::path::Path;
use std::process::Command;

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

/// 경로에서 드라이브 문자 추출 (Windows)
fn get_drive_letter(path: &Path) -> Option<char> {
    path.to_str()
        .and_then(|s| s.chars().next())
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

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
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
/// 1. WMI로 정확한 MediaType 조회 시도
/// 2. 실패 시 드라이브 문자로 추정
pub fn detect_disk_type(path: &Path) -> DiskType {
    let drive_letter = match get_drive_letter(path) {
        Some(c) => c,
        None => return DiskType::Unknown,
    };

    // WMI 조회 시도 (캐시 가능하면 좋지만 일단 매번 조회)
    if let Some(disk_type) = query_disk_type_wmi(drive_letter) {
        tracing::debug!("Disk type for {}: {:?} (WMI)", drive_letter, disk_type);
        return disk_type;
    }

    // fallback: 드라이브 문자로 추정
    let guessed = guess_disk_type_by_letter(drive_letter);
    tracing::debug!("Disk type for {}: {:?} (guessed)", drive_letter, guessed);
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
                throttle_ms: 50,  // HDD는 랜덤 I/O 부하 방지
                parallel_threads: 1,  // 순차 처리
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
        assert_eq!(get_drive_letter(Path::new("/home/user")), Some('/'));
    }

    #[test]
    fn test_guess_disk_type() {
        assert_eq!(guess_disk_type_by_letter('C'), DiskType::Ssd);
        assert_eq!(guess_disk_type_by_letter('D'), DiskType::Hdd);
    }
}
