//! 인메모리 파일명 캐시
//!
//! Everything 스타일의 빠른 파일명 검색을 위한 인메모리 캐시.
//! 10만 파일에서 ~5ms 검색 목표.

use rusqlite::Connection;
use std::sync::RwLock;

/// 파일명 엔트리
#[derive(Debug, Clone)]
pub struct FilenameEntry {
    pub file_id: i64,
    pub path: String,
    pub name: String,
    /// 검색용 소문자 변환 파일명
    pub name_lower: String,
    pub file_type: String,
    pub size: i64,
    pub modified_at: i64,
}

/// 인메모리 파일명 캐시
pub struct FilenameCache {
    entries: RwLock<Vec<FilenameEntry>>,
}

impl FilenameCache {
    /// 빈 캐시 생성
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }

    /// DB에서 캐시 로드
    pub fn load_from_db(&self, conn: &Connection) -> Result<usize, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, path, name, file_type, COALESCE(size, 0), COALESCE(modified_at, 0)
             FROM files
             ORDER BY name"
        )?;

        let rows = stmt.query_map([], |row| {
            let name: String = row.get(2)?;
            Ok(FilenameEntry {
                file_id: row.get(0)?,
                path: row.get(1)?,
                name: name.clone(),
                name_lower: name.to_lowercase(),
                file_type: row.get(3)?,
                size: row.get(4)?,
                modified_at: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            if let Ok(entry) = row {
                entries.push(entry);
            }
        }

        let count = entries.len();
        if let Ok(mut cache) = self.entries.write() {
            *cache = entries;
        }

        tracing::info!("FilenameCache: loaded {} entries", count);
        Ok(count)
    }

    /// 파일명 검색 (O(n) 벡터 스캔, 10만 파일 ~5ms)
    pub fn search(&self, query: &str, limit: usize) -> Vec<FilenameEntry> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return vec![];
        }

        // 검색어들 (AND 조건)
        let terms: Vec<String> = trimmed
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        if terms.is_empty() {
            return vec![];
        }

        let entries = match self.entries.read() {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        // O(n) 스캔 - 모든 검색어가 포함된 파일만
        entries
            .iter()
            .filter(|e| terms.iter().all(|term| e.name_lower.contains(term)))
            .take(limit)
            .cloned()
            .collect()
    }

    /// 파일 추가/갱신
    pub fn upsert(&self, entry: FilenameEntry) {
        if let Ok(mut entries) = self.entries.write() {
            // 기존 엔트리 찾기
            if let Some(pos) = entries.iter().position(|e| e.file_id == entry.file_id) {
                entries[pos] = entry;
            } else {
                entries.push(entry);
            }
        }
    }

    /// 파일 삭제
    pub fn remove(&self, file_id: i64) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|e| e.file_id != file_id);
        }
    }

    /// 경로로 삭제 (폴더 삭제 시)
    pub fn remove_by_path_prefix(&self, path_prefix: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|e| !e.path.starts_with(path_prefix));
        }
    }

    /// 캐시 크기
    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    /// 캐시 비어있는지
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 캐시 초기화
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }
}

impl Default for FilenameCache {
    fn default() -> Self {
        Self::new()
    }
}

/// 검색 결과 (filename.rs와 호환)
/// NOTE: 현재 미사용 (SearchResponse 직접 사용 중)
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize)]
pub struct FilenameSearchResult {
    pub file_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub size: i64,
    pub modified_at: i64,
    pub score: f64,
}

impl From<FilenameEntry> for FilenameSearchResult {
    fn from(entry: FilenameEntry) -> Self {
        Self {
            file_id: entry.file_id,
            file_path: entry.path,
            file_name: entry.name,
            file_type: entry.file_type,
            size: entry.size,
            modified_at: entry.modified_at,
            score: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(id: i64, name: &str) -> FilenameEntry {
        FilenameEntry {
            file_id: id,
            path: format!("C:\\test\\{}", name),
            name: name.to_string(),
            name_lower: name.to_lowercase(),
            file_type: "txt".to_string(),
            size: 1000,
            modified_at: 0,
        }
    }

    #[test]
    fn test_search() {
        let cache = FilenameCache::new();

        // 직접 엔트리 추가
        cache.upsert(create_test_entry(1, "보고서_2024.docx"));
        cache.upsert(create_test_entry(2, "회의록_2024.txt"));
        cache.upsert(create_test_entry(3, "보고서_2023.docx"));

        // 검색
        let results = cache.search("보고서", 10);
        assert_eq!(results.len(), 2);

        let results = cache.search("2024", 10);
        assert_eq!(results.len(), 2);

        let results = cache.search("보고서 2024", 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_case_insensitive() {
        let cache = FilenameCache::new();
        cache.upsert(create_test_entry(1, "Report.DOCX"));

        let results = cache.search("report", 10);
        assert_eq!(results.len(), 1);

        let results = cache.search("REPORT", 10);
        assert_eq!(results.len(), 1);
    }
}
