use crate::application::container::AppContainer;
use crate::{db, ApiError, ApiResult};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::RwLock;
use tauri::State;

#[derive(Debug, Serialize, Clone)]
pub struct DuplicateGroup {
    /// 그룹 내 파일 경로들
    pub files: Vec<DuplicateFile>,
    /// 중복 유형: "exact" | "similar"
    pub duplicate_type: String,
    /// 유사도 (exact=1.0, similar=0.90~0.99)
    pub similarity: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DuplicateFile {
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub size: i64,
    pub modified_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct DuplicateResponse {
    pub exact_groups: Vec<DuplicateGroup>,
    pub similar_groups: Vec<DuplicateGroup>,
    pub scan_time_ms: u64,
    pub total_files_scanned: usize,
}

/// 중복 문서 탐지 (정확 중복 + 유사 중복)
#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<DuplicateResponse> {
    let start = std::time::Instant::now();

    let (db_path, embedder, vector_index) = {
        let container = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        (
            container.db_path.clone(),
            container.get_embedder().ok(),
            container.get_vector_index().ok(),
        )
    };

    // 1) 정확 중복: 같은 size 파일 그룹 → SHA-256 해시 비교
    let exact_groups = tokio::task::spawn_blocking({
        let db_path = db_path.clone();
        move || find_exact_duplicates(&db_path)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))??;

    // 2) 유사 중복: 벡터 유사도 기반 (시맨틱 검색 활성 시만)
    let similar_groups = if let (Some(emb), Some(vi)) = (embedder, vector_index) {
        tokio::task::spawn_blocking({
            let db_path = db_path.clone();
            move || find_similar_duplicates(&db_path, &emb, &vi)
        })
        .await
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))??
    } else {
        vec![]
    };

    let total_files = {
        let conn = db::get_connection(&db_path)
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap_or(0);
        count as usize
    };

    Ok(DuplicateResponse {
        exact_groups,
        similar_groups,
        scan_time_ms: start.elapsed().as_millis() as u64,
        total_files_scanned: total_files,
    })
}

/// 정확 중복: 같은 size 파일 그룹 → SHA-256 해시 비교
fn find_exact_duplicates(
    db_path: &std::path::Path,
) -> ApiResult<Vec<DuplicateGroup>> {
    let conn =
        db::get_connection(db_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    // size가 같은 파일 그룹 조회 (최소 2개 이상, 0바이트 제외)
    let mut stmt = conn
        .prepare(
            "SELECT path, name, file_type, size, modified_at FROM files
             WHERE size > 0
             AND size IN (SELECT size FROM files WHERE size > 0 GROUP BY size HAVING COUNT(*) >= 2)
             ORDER BY size DESC, path",
        )
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    let files: Vec<(String, String, String, i64, Option<i64>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    // size별 그룹핑
    let mut size_groups: HashMap<i64, Vec<(String, String, String, i64, Option<i64>)>> =
        HashMap::new();
    for f in files {
        size_groups.entry(f.3).or_default().push(f);
    }

    // 각 size 그룹 내에서 SHA-256 해시 비교
    let mut result = Vec::new();
    for (_size, group) in &size_groups {
        let mut hash_map: HashMap<String, Vec<&(String, String, String, i64, Option<i64>)>> =
            HashMap::new();

        for file in group {
            let path = std::path::Path::new(&file.0);
            if !path.exists() {
                continue;
            }
            // 500MB 초과 파일은 해싱 스킵 (CPU/IO 과부하 방지)
            if file.3 > 500 * 1024 * 1024 {
                continue;
            }
            match compute_file_hash(path) {
                Ok(hash) => {
                    hash_map.entry(hash).or_default().push(file);
                }
                Err(_) => continue,
            }
        }

        for (_hash, duplicates) in hash_map {
            if duplicates.len() >= 2 {
                result.push(DuplicateGroup {
                    files: duplicates
                        .into_iter()
                        .map(|f| DuplicateFile {
                            file_path: f.0.clone(),
                            file_name: f.1.clone(),
                            file_type: f.2.clone(),
                            size: f.3,
                            modified_at: f.4,
                        })
                        .collect(),
                    duplicate_type: "exact".into(),
                    similarity: 1.0,
                });
            }
        }
    }

    // 파일 수 내림차순 정렬
    result.sort_by(|a, b| b.files.len().cmp(&a.files.len()));

    Ok(result)
}

/// 유사 중복: 벡터 유사도 기반 (0.90 이상)
fn find_similar_duplicates(
    db_path: &std::path::Path,
    embedder: &std::sync::Arc<crate::embedder::Embedder>,
    vector_index: &std::sync::Arc<crate::search::vector::VectorIndex>,
) -> ApiResult<Vec<DuplicateGroup>> {
    let conn =
        db::get_connection(db_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    // 벡터 인덱싱된 파일들의 대표 청크 (chunk_index=0) 조회
    let mut stmt = conn
        .prepare(
            "SELECT f.path, f.name, f.file_type, f.size, f.modified_at, c.id, c.content
             FROM files f
             JOIN chunks c ON c.file_id = f.id AND c.chunk_index = 0
             WHERE f.vector_indexed_at IS NOT NULL
             ORDER BY f.path",
        )
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

    let docs: Vec<(String, String, String, i64, Option<i64>, i64, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    if docs.len() < 2 {
        return Ok(vec![]);
    }

    // 문서별 임베딩 생성 (첫 번째 청크 기반)
    let mut doc_embeddings: Vec<(usize, Vec<f32>)> = Vec::new();
    // 최대 200개 문서만 검사 (성능)
    let check_limit = docs.len().min(200);
    for (i, doc) in docs.iter().take(check_limit).enumerate() {
        let text = &doc.6;
        if text.len() < 20 {
            continue; // 너무 짧은 문서 스킵
        }
        match embedder.embed(text, false) {
            Ok(embedding) => doc_embeddings.push((i, embedding)),
            Err(_) => continue,
        }
    }

    // 각 문서의 임베딩으로 벡터 검색 → 유사 문서 그룹 생성
    let mut processed: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    const SIMILARITY_THRESHOLD: f32 = 0.90;

    for &(doc_idx, ref embedding) in &doc_embeddings {
        if processed.contains(&doc_idx) {
            continue;
        }

        let search_results = match vector_index.search(embedding, 10) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // 자기 파일 제외 + 유사도 0.90 이상만
        let mut similar_indices: Vec<(usize, f64)> = Vec::new();
        for vr in &search_results {
            if vr.score < SIMILARITY_THRESHOLD {
                continue;
            }
            // chunk_id로 파일 경로 찾기
            let chunk_id = vr.chunk_id;
            if let Some(pos) = docs.iter().position(|d| d.5 == chunk_id) {
                if pos != doc_idx && !processed.contains(&pos) {
                    similar_indices.push((pos, vr.score as f64));
                }
            }
        }

        if !similar_indices.is_empty() {
            processed.insert(doc_idx);
            let mut group_files = vec![DuplicateFile {
                file_path: docs[doc_idx].0.clone(),
                file_name: docs[doc_idx].1.clone(),
                file_type: docs[doc_idx].2.clone(),
                size: docs[doc_idx].3,
                modified_at: docs[doc_idx].4,
            }];

            let mut max_similarity = 0.0f64;
            for (sim_idx, score) in &similar_indices {
                processed.insert(*sim_idx);
                group_files.push(DuplicateFile {
                    file_path: docs[*sim_idx].0.clone(),
                    file_name: docs[*sim_idx].1.clone(),
                    file_type: docs[*sim_idx].2.clone(),
                    size: docs[*sim_idx].3,
                    modified_at: docs[*sim_idx].4,
                });
                if *score > max_similarity {
                    max_similarity = *score;
                }
            }

            groups.push(DuplicateGroup {
                files: group_files,
                duplicate_type: "similar".into(),
                similarity: max_similarity,
            });
        }
    }

    // 유사도 내림차순 정렬
    groups.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    Ok(groups)
}

/// 파일 SHA-256 해시 계산
fn compute_file_hash(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
