//! 인메모리 파일명 캐시
//!
//! Everything 스타일의 빠른 파일명 검색을 위한 인메모리 캐시.
//! 10만 파일에서 ~5ms 검색 목표.

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::RwLock;

/// 캐시 최대 엔트리 수 (~30MB 상한, 8GB RAM 환경 기준)
const MAX_CACHE_ENTRIES: usize = 200_000;

/// 파일명 엔트리 (메모리 최적화: name 제거, String → Box<str>)
#[derive(Debug, Clone)]
pub struct FilenameEntry {
    pub file_id: i64,
    pub path: Box<str>,
    /// 검색용 소문자 변환 파일명 (path에서 추출 후 캐시)
    pub name_lower: Box<str>,
    pub file_type: Box<str>,
    pub size: i64,
    pub modified_at: i64,
}

impl FilenameEntry {
    /// 파일명 추출 (path에서 O(1) 추출)
    pub fn name(&self) -> &str {
        std::path::Path::new(&*self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    }
}

/// 캐시 내부 데이터 (entries + file_id → index 매핑)
struct CacheData {
    entries: Vec<FilenameEntry>,
    /// file_id → entries 인덱스 매핑 (upsert/remove O(1))
    id_index: HashMap<i64, usize>,
    /// DB 로드 시 MAX_CACHE_ENTRIES 초과로 잘렸는지 여부
    truncated: bool,
}

impl CacheData {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            id_index: HashMap::new(),
            truncated: false,
        }
    }

    /// entries로부터 id_index 재구축
    fn rebuild_index(&mut self) {
        self.id_index.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            self.id_index.insert(entry.file_id, idx);
        }
    }
}

/// 인메모리 파일명 캐시
pub struct FilenameCache {
    data: RwLock<CacheData>,
}

impl FilenameCache {
    /// 빈 캐시 생성
    pub fn new() -> Self {
        Self {
            data: RwLock::new(CacheData::new()),
        }
    }

    /// DB에서 캐시 로드
    pub fn load_from_db(&self, conn: &Connection) -> Result<usize, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, path, name, file_type, COALESCE(size, 0), COALESCE(modified_at, 0)
             FROM files
             ORDER BY name",
        )?;

        let rows = stmt.query_map([], |row| {
            let name: String = row.get(2)?;
            let path: String = row.get(1)?;
            let file_type: String = row.get(3)?;
            Ok(FilenameEntry {
                file_id: row.get(0)?,
                path: path.into_boxed_str(),
                name_lower: name.to_lowercase().into_boxed_str(),
                file_type: file_type.into_boxed_str(),
                size: row.get(4)?,
                modified_at: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for entry in rows.flatten() {
            entries.push(entry);
        }

        let total = entries.len();
        let was_truncated = total > MAX_CACHE_ENTRIES;
        if was_truncated {
            tracing::warn!(
                "FilenameCache: {} entries exceeds max {}. Truncating. DB fallback will be used for filename search.",
                total, MAX_CACHE_ENTRIES
            );
            entries.truncate(MAX_CACHE_ENTRIES);
        }

        let count = entries.len();
        if let Ok(mut cache) = self.data.write() {
            cache.entries = entries;
            cache.truncated = was_truncated;
            cache.rebuild_index();
        }

        tracing::info!(
            "FilenameCache: loaded {} entries (total in DB: {})",
            count,
            total
        );
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

        let cache = match self.data.read() {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        // O(n) 스캔 - 모든 검색어가 포함된 파일만
        cache
            .entries
            .iter()
            .filter(|e| {
                terms
                    .iter()
                    .all(|term| e.name_lower.contains(term.as_str()))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// 파일 추가/갱신 - O(1) HashMap 룩업 (기존 O(n) position() 대비 대폭 개선)
    pub fn upsert(&self, entry: FilenameEntry) {
        if let Ok(mut cache) = self.data.write() {
            let file_id = entry.file_id;
            if let Some(&pos) = cache.id_index.get(&file_id) {
                cache.entries[pos] = entry;
            } else {
                let idx = cache.entries.len();
                cache.id_index.insert(file_id, idx);
                cache.entries.push(entry);
            }
        }
    }

    /// 파일 삭제 - swap_remove + 인덱스 갱신으로 O(1)
    pub fn remove(&self, file_id: i64) {
        if let Ok(mut cache) = self.data.write() {
            if let Some(pos) = cache.id_index.remove(&file_id) {
                // swap_remove: 마지막 원소와 교체 후 제거 (O(1))
                cache.entries.swap_remove(pos);
                // swap된 원소의 인덱스 갱신
                if pos < cache.entries.len() {
                    let swapped_id = cache.entries[pos].file_id;
                    cache.id_index.insert(swapped_id, pos);
                }
            }
        }
    }

    /// 경로로 삭제 (폴더 삭제 시) - 삭제 후 인덱스 재구축
    pub fn remove_by_path_prefix(&self, path_prefix: &str) {
        if let Ok(mut cache) = self.data.write() {
            cache.entries.retain(|e| !e.path.starts_with(path_prefix));
            cache.rebuild_index();
        }
    }

    /// DB 로드 시 truncated 되었는지 여부
    pub fn is_truncated(&self) -> bool {
        self.data.read().map(|c| c.truncated).unwrap_or(false)
    }

    /// 캐시 크기
    pub fn len(&self) -> usize {
        self.data.read().map(|c| c.entries.len()).unwrap_or(0)
    }

    /// 캐시 비어있는지
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 캐시 초기화
    pub fn clear(&self) {
        if let Ok(mut cache) = self.data.write() {
            cache.entries.clear();
            cache.id_index.clear();
        }
    }
}

impl Default for FilenameCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(id: i64, name: &str) -> FilenameEntry {
        FilenameEntry {
            file_id: id,
            path: format!("C:\\test\\{}", name).into_boxed_str(),
            name_lower: name.to_lowercase().into_boxed_str(),
            file_type: "txt".to_string().into_boxed_str(),
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

    #[test]
    fn test_name_from_path() {
        let entry = create_test_entry(1, "test.docx");
        assert_eq!(entry.name(), "test.docx");
    }
}
