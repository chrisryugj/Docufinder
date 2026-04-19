//! Document Lineage 관리 커맨드.

use crate::application::container::AppContainer;
use crate::indexer::lineage;
use crate::{db, ApiError, ApiResult};
use rusqlite::params;
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct RebuildLineageResponse {
    pub files_updated: usize,
    pub lineages_created: usize,
    /// 벡터 유사도로 분리된 파일 수 (embedder 활성 시)
    pub vector_split: usize,
    /// Cross-folder reunion으로 병합된 lineage 수
    pub reunited: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct LineageVersion {
    pub file_path: String,
    pub file_name: String,
    pub lineage_role: Option<String>,
    pub version_label: Option<String>,
    pub modified_at: Option<i64>,
    pub size: Option<i64>,
}

/// Lineage 건강도 리포트 — 정리 대상 감지용.
#[derive(Debug, Serialize)]
pub struct LineageHealthEntry {
    pub lineage_id: String,
    pub canonical_name: String,
    pub canonical_path: String,
    pub file_count: i64,
    pub total_size: i64,
    /// "healthy" | "cluttered" | "ambiguous" | "abandoned"
    pub status: String,
    /// 사람이 읽을 수 있는 문제 설명들
    pub issues: Vec<String>,
    /// 오래된 version 파일 수 (modified_at < 180일)
    pub stale_count: i64,
}

/// 버전 간 diff 항목.
#[derive(Debug, Serialize)]
pub struct ChunkDiffEntry {
    /// "added" | "removed" | "modified" | "unchanged"
    pub kind: String,
    /// 원본 버전의 청크 인덱스 (removed/modified/unchanged)
    pub a_index: Option<i64>,
    /// 비교 대상 버전의 청크 인덱스 (added/modified/unchanged)
    pub b_index: Option<i64>,
    /// 요약 텍스트 (각 청크 앞 100자)
    pub a_preview: Option<String>,
    pub b_preview: Option<String>,
    /// 코사인 유사도 (modified/unchanged)
    pub similarity: Option<f32>,
    pub page_number: Option<i64>,
    pub location_hint: Option<String>,
    /// 바이트 수준 정확히 동일한지 (unchanged에서 "완전 동일" vs "거의 동일" 구분)
    pub byte_identical: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LineageDiffResponse {
    pub a_path: String,
    pub b_path: String,
    pub a_total_chunks: i64,
    pub b_total_chunks: i64,
    pub changes: Vec<ChunkDiffEntry>,
    pub unchanged_count: i64,
}

#[derive(Debug, Serialize)]
pub struct LineageHealthReport {
    pub total_lineages: i64,
    pub multi_version_lineages: i64,
    pub problem_lineages: Vec<LineageHealthEntry>,
    /// 전체 파일 중 lineage 미부여 파일 수 (rebuild 필요 신호)
    pub unassigned_files: i64,
}

/// 전체 files 테이블에 대해 lineage_id / stem_norm / canonical 역할을 재계산한다.
///
/// 처음 v12로 마이그레이션된 기존 DB에 대해 한 번 실행하거나, 정규화 규칙을 개선한 뒤
/// 재계산하고 싶을 때 사용한다.
#[tauri::command]
pub async fn rebuild_lineage(
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<RebuildLineageResponse> {
    let start = std::time::Instant::now();

    let (db_path, embedder) = {
        let container = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        (container.db_path.clone(), container.get_embedder().ok())
    };

    let (files_updated, lineages_created, vector_split, reunited) =
        tokio::task::spawn_blocking(move || -> ApiResult<(usize, usize, usize, usize)> {
            let conn = db::get_connection(&db_path)
                .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

            // 1. 파일명 기반 1차 그루핑
            let (files_updated, lineages_created) =
                lineage::rebuild_all(&conn).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

            // 2. 벡터 유사도 검증으로 같은 lineage 내 다른 내용 파일 분리
            let vector_split = if let Some(emb) = embedder.as_ref() {
                lineage::refine_with_vector(&conn, emb)
                    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
            } else {
                0
            };

            // 3. Cross-folder reunion — 다른 폴더에 있는 같은 문서 자동 병합
            let reunited = if let Some(emb) = embedder.as_ref() {
                lineage::reunite_cross_folder(&conn, emb)
                    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
            } else {
                0
            };

            Ok((files_updated, lineages_created, vector_split, reunited))
        })
        .await
        .map_err(|e| ApiError::IndexingFailed(e.to_string()))??;

    Ok(RebuildLineageResponse {
        files_updated,
        lineages_created,
        vector_split,
        reunited,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// 특정 lineage의 모든 버전 목록을 반환한다 (UI 펼치기용).
/// canonical 먼저, 이후 modified_at 내림차순.
#[tauri::command]
pub async fn get_lineage_versions(
    state: State<'_, RwLock<AppContainer>>,
    lineage_id: String,
) -> ApiResult<Vec<LineageVersion>> {
    let db_path = {
        let container = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        container.db_path.clone()
    };

    tokio::task::spawn_blocking(move || {
        let conn =
            db::get_connection(&db_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT path, name, lineage_role, version_label, modified_at, size
                 FROM files
                 WHERE lineage_id = ?1
                 ORDER BY (lineage_role = 'canonical') DESC, modified_at DESC",
            )
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        let rows = stmt
            .query_map([&lineage_id], |r| {
                Ok(LineageVersion {
                    file_path: r.get(0)?,
                    file_name: r.get(1)?,
                    lineage_role: r.get(2)?,
                    version_label: r.get(3)?,
                    modified_at: r.get(4)?,
                    size: r.get(5)?,
                })
            })
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| ApiError::IndexingFailed(e.to_string()))?);
        }
        Ok::<_, ApiError>(out)
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

fn chunk_cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb + 1e-9)
}

/// 두 파일 간 청크 레벨 diff — 변경된/추가된/제거된 청크를 식별한다.
///
/// 알고리즘:
/// 1. 각 파일의 모든 청크 content 조회
/// 2. 각 청크를 임베딩 (embedder 활성 시)
/// 3. Greedy match: A의 각 청크에 대해 B에서 가장 유사한 청크 탐색
///    - 유사도 ≥ 0.95: unchanged (diff 제외)
///    - 0.5 ≤ 유사도 < 0.95: modified
///    - 유사도 < 0.5 or 매칭 실패: removed (A에만) / added (B에만)
/// 4. B에 남은 미매칭 청크는 added
#[tauri::command]
pub async fn get_lineage_diff(
    state: State<'_, RwLock<AppContainer>>,
    a_path: String,
    b_path: String,
) -> ApiResult<LineageDiffResponse> {
    const UNCHANGED_THRESHOLD: f32 = 0.95;
    const MODIFIED_THRESHOLD: f32 = 0.5;
    const MAX_CHUNKS_PER_FILE: usize = 200; // 성능 보호

    let (db_path, embedder) = {
        let container = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        (container.db_path.clone(), container.get_embedder().ok())
    };

    let embedder = embedder.ok_or_else(|| {
        ApiError::IndexingFailed("시맨틱 검색이 비활성화되어 버전 비교가 불가능합니다".into())
    })?;

    tokio::task::spawn_blocking(move || {
        let conn =
            db::get_connection(&db_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

        // 각 파일의 청크 조회
        let load_chunks =
            |path: &str| -> rusqlite::Result<Vec<(i64, String, Option<i64>, Option<String>)>> {
                let mut stmt = conn.prepare(
                    "SELECT c.chunk_index, c.content, c.page_number, c.location_hint
                 FROM chunks c
                 JOIN files f ON f.id = c.file_id
                 WHERE f.path = ?1
                 ORDER BY c.chunk_index LIMIT ?2",
                )?;
                let mut rows = stmt.query(params![path, MAX_CHUNKS_PER_FILE as i64])?;
                let mut out = Vec::new();
                while let Some(row) = rows.next()? {
                    out.push((
                        row.get(0)?,
                        row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        row.get(2)?,
                        row.get(3)?,
                    ));
                }
                Ok(out)
            };

        let a_chunks = load_chunks(&a_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;
        let b_chunks = load_chunks(&b_path).map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

        if a_chunks.is_empty() && b_chunks.is_empty() {
            return Ok(LineageDiffResponse {
                a_path,
                b_path,
                a_total_chunks: 0,
                b_total_chunks: 0,
                changes: vec![],
                unchanged_count: 0,
            });
        }

        // 임베딩 (배치 가능하면 효율적, 없으면 개별)
        let embed_chunks =
            |chunks: &[(i64, String, Option<i64>, Option<String>)]| -> Vec<Option<Vec<f32>>> {
                chunks
                    .iter()
                    .map(|(_, content, _, _)| {
                        if content.len() < 20 {
                            None
                        } else {
                            embedder.embed(content, false).ok()
                        }
                    })
                    .collect()
            };

        let a_emb = embed_chunks(&a_chunks);
        let b_emb = embed_chunks(&b_chunks);

        let preview = |s: &str| -> String {
            let trimmed = s.trim();
            if trimmed.chars().count() <= 100 {
                trimmed.to_string()
            } else {
                let t: String = trimmed.chars().take(100).collect();
                format!("{}…", t)
            }
        };

        // Greedy 매칭: A의 각 청크 → B 최적 매치
        let mut b_matched: Vec<bool> = vec![false; b_chunks.len()];
        let mut changes: Vec<ChunkDiffEntry> = Vec::new();
        let mut unchanged = 0i64;

        // unchanged 샘플은 최대 20개까지 응답에 포함 (너무 많으면 UI 과부하)
        const MAX_UNCHANGED_SAMPLES: usize = 20;
        let mut unchanged_samples: Vec<ChunkDiffEntry> = Vec::new();

        for (ai, (a_idx, a_content, a_page, a_hint)) in a_chunks.iter().enumerate() {
            let Some(ae) = &a_emb[ai] else {
                continue;
            };
            let mut best_sim = -1.0f32;
            let mut best_bi: Option<usize> = None;
            for (bi, be_opt) in b_emb.iter().enumerate() {
                if b_matched[bi] {
                    continue;
                }
                if let Some(be) = be_opt {
                    let sim = chunk_cosine(ae, be);
                    if sim > best_sim {
                        best_sim = sim;
                        best_bi = Some(bi);
                    }
                }
            }

            match best_bi {
                Some(bi) if best_sim >= UNCHANGED_THRESHOLD => {
                    b_matched[bi] = true;
                    unchanged += 1;
                    if unchanged_samples.len() < MAX_UNCHANGED_SAMPLES {
                        let (b_idx, b_content, _, _) = &b_chunks[bi];
                        let byte_identical = a_content == b_content;
                        unchanged_samples.push(ChunkDiffEntry {
                            kind: "unchanged".into(),
                            a_index: Some(*a_idx),
                            b_index: Some(*b_idx),
                            a_preview: Some(preview(a_content)),
                            b_preview: Some(preview(b_content)),
                            similarity: Some(best_sim),
                            page_number: *a_page,
                            location_hint: a_hint.clone(),
                            byte_identical: Some(byte_identical),
                        });
                    }
                }
                Some(bi) if best_sim >= MODIFIED_THRESHOLD => {
                    b_matched[bi] = true;
                    let (b_idx, b_content, _, _) = &b_chunks[bi];
                    changes.push(ChunkDiffEntry {
                        kind: "modified".into(),
                        a_index: Some(*a_idx),
                        b_index: Some(*b_idx),
                        a_preview: Some(preview(a_content)),
                        b_preview: Some(preview(b_content)),
                        similarity: Some(best_sim),
                        page_number: *a_page,
                        location_hint: a_hint.clone(),
                        byte_identical: None,
                    });
                }
                _ => {
                    // A에만 있는 청크
                    changes.push(ChunkDiffEntry {
                        kind: "removed".into(),
                        a_index: Some(*a_idx),
                        b_index: None,
                        a_preview: Some(preview(a_content)),
                        b_preview: None,
                        similarity: None,
                        page_number: *a_page,
                        location_hint: a_hint.clone(),
                        byte_identical: None,
                    });
                }
            }
        }

        // B에 남은 미매칭 = added
        for (bi, matched) in b_matched.iter().enumerate() {
            if !matched {
                let (b_idx, b_content, b_page, b_hint) = &b_chunks[bi];
                if b_content.len() < 20 {
                    continue;
                }
                changes.push(ChunkDiffEntry {
                    kind: "added".into(),
                    a_index: None,
                    b_index: Some(*b_idx),
                    a_preview: None,
                    b_preview: Some(preview(b_content)),
                    similarity: None,
                    page_number: *b_page,
                    location_hint: b_hint.clone(),
                    byte_identical: None,
                });
            }
        }

        // 변경점이 없을 때 unchanged 샘플을 changes에 append — UI에서 "뭐가 비교됐는지" 보여줌
        changes.extend(unchanged_samples);

        Ok::<_, ApiError>(LineageDiffResponse {
            a_path,
            b_path,
            a_total_chunks: a_chunks.len() as i64,
            b_total_chunks: b_chunks.len() as i64,
            changes,
            unchanged_count: unchanged,
        })
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}

/// Lineage 건강도 리포트 — 버전이 과도하게 많거나 canonical이 모호한 lineage를 찾아냄.
/// UI 대시보드에서 "정리 대상"을 안내하는 용도.
#[tauri::command]
pub async fn get_lineage_health(
    state: State<'_, RwLock<AppContainer>>,
    limit: Option<usize>,
) -> ApiResult<LineageHealthReport> {
    let db_path = {
        let container = state
            .read()
            .map_err(|_| ApiError::IndexingFailed("Lock error".into()))?;
        container.db_path.clone()
    };
    let limit = limit.unwrap_or(20);

    tokio::task::spawn_blocking(move || {
        let conn = db::get_connection(&db_path)
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

        // 전체 통계
        let total_lineages: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT lineage_id) FROM files WHERE lineage_id IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let multi_version_lineages: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM (
                    SELECT lineage_id FROM files
                    WHERE lineage_id IS NOT NULL
                    GROUP BY lineage_id HAVING COUNT(*) >= 2
                 )",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let unassigned_files: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE lineage_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // 6개월 cutoff (stale 감지)
        let six_months_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
            - 180 * 24 * 3600;

        // 문제 lineage 후보 조회 (버전 ≥ 2)
        let mut stmt = conn
            .prepare(
                "SELECT f.lineage_id,
                        COUNT(*) AS cnt,
                        COALESCE(SUM(f.size), 0) AS total_size,
                        SUM(CASE WHEN f.modified_at < ?1 AND f.lineage_role != 'canonical' THEN 1 ELSE 0 END) AS stale,
                        (SELECT name FROM files WHERE lineage_id = f.lineage_id AND lineage_role = 'canonical' LIMIT 1) AS canonical_name,
                        (SELECT path FROM files WHERE lineage_id = f.lineage_id AND lineage_role = 'canonical' LIMIT 1) AS canonical_path
                 FROM files f
                 WHERE f.lineage_id IS NOT NULL
                 GROUP BY f.lineage_id
                 HAVING cnt >= 2
                 ORDER BY cnt DESC, stale DESC",
            )
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

        let rows = stmt
            .query_map([six_months_ago], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                ))
            })
            .map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

        let mut problems: Vec<LineageHealthEntry> = Vec::new();
        for row in rows {
            let (lid, cnt, total_size, stale, canonical_name, canonical_path) =
                row.map_err(|e| ApiError::IndexingFailed(e.to_string()))?;

            let mut issues = Vec::new();
            let mut status = "healthy";

            if cnt >= 10 {
                issues.push(format!("버전이 과도함 ({}개)", cnt));
                status = "cluttered";
            }
            if canonical_name.is_empty() {
                issues.push("canonical 미지정".to_string());
                status = "ambiguous";
            }
            if stale >= 3 && cnt >= 5 {
                issues.push(format!("오래된 버전 {}개 (6개월+)", stale));
                if status == "healthy" {
                    status = "abandoned";
                }
            }

            if issues.is_empty() {
                continue;
            }

            problems.push(LineageHealthEntry {
                lineage_id: lid,
                canonical_name,
                canonical_path,
                file_count: cnt,
                total_size,
                status: status.to_string(),
                issues,
                stale_count: stale,
            });

            if problems.len() >= limit {
                break;
            }
        }

        Ok::<_, ApiError>(LineageHealthReport {
            total_lineages,
            multi_version_lineages,
            problem_lineages: problems,
            unassigned_files,
        })
    })
    .await
    .map_err(|e| ApiError::IndexingFailed(e.to_string()))?
}
