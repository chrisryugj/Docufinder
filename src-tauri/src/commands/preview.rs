//! 문서 미리보기 + 북마크 + 요약 커맨드

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::search::textrank;
use crate::AppContainer;
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;

// ======================== 미리보기 ========================

/// 미리보기 청크 (프론트엔드용)
#[derive(Debug, Serialize)]
pub struct PreviewChunk {
    pub chunk_id: i64,
    pub chunk_index: i64,
    pub content: String,
    pub page_number: Option<i64>,
    pub location_hint: Option<String>,
}

/// 미리보기 섹션 (오버랩 제거 후 병합된 연속 텍스트)
#[derive(Debug, Serialize)]
pub struct PreviewSection {
    /// 섹션 라벨 (페이지 번호, 시트명 등)
    pub label: Option<String>,
    /// 병합된 연속 텍스트
    pub content: String,
}

/// 미리보기 응답
#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub file_path: String,
    pub file_name: String,
    pub chunks: Vec<PreviewChunk>,
    /// 오버랩 제거 후 섹션별 병합 텍스트
    pub sections: Vec<PreviewSection>,
    pub total_chars: usize,
}

/// 파일 경로로 문서 전체 텍스트 로드 (미리보기용)
#[tauri::command]
pub async fn load_document_preview(
    file_path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<PreviewResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<PreviewResponse> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;

        // 1. 파일 경로로 청크 ID 조회
        let chunk_ids = db::get_chunk_ids_for_path(&conn, &file_path)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        if chunk_ids.is_empty() {
            return Ok(PreviewResponse {
                file_path: file_path.clone(),
                file_name: std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                chunks: vec![],
                sections: vec![],
                total_chars: 0,
            });
        }

        // 2. 청크 데이터 조회
        let chunk_infos = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        // 3. chunk_index 순 정렬
        let mut sorted = chunk_infos;
        sorted.sort_by_key(|c| c.chunk_index);

        let file_name = sorted
            .first()
            .map(|c| c.file_name.clone())
            .unwrap_or_default();

        // 오버랩 제거 후 섹션별 병합
        let sections = merge_chunks_into_sections(&sorted);
        let total_chars: usize = sections.iter().map(|s| s.content.len()).sum();

        let chunks: Vec<PreviewChunk> = sorted
            .into_iter()
            .map(|c| PreviewChunk {
                chunk_id: c.chunk_id,
                chunk_index: c.chunk_index,
                content: c.content,
                page_number: c.page_number,
                location_hint: c.location_hint,
            })
            .collect();

        Ok(PreviewResponse {
            file_path,
            file_name,
            chunks,
            sections,
            total_chars,
        })
    })
    .await??;

    Ok(result)
}

/// 청크들을 오버랩 제거 후 섹션(페이지/시트)별로 병합
fn merge_chunks_into_sections(sorted_chunks: &[db::ChunkInfo]) -> Vec<PreviewSection> {
    if sorted_chunks.is_empty() {
        return vec![];
    }

    let mut sections: Vec<PreviewSection> = Vec::new();
    let mut current_label: Option<String> = None;
    let mut current_text = String::new();
    let mut prev_end_offset: i64 = 0;

    for chunk in sorted_chunks {
        // 섹션 라벨 결정 (location_hint > page_number)
        let label = chunk
            .location_hint
            .clone()
            .or_else(|| chunk.page_number.map(|p| format!("{}페이지", p)));

        // 섹션 변경 감지 → 이전 섹션 저장 후 새 섹션 시작
        if label != current_label && !current_text.is_empty() {
            sections.push(PreviewSection {
                label: current_label.take(),
                content: current_text.clone(),
            });
            current_text.clear();
            prev_end_offset = 0;
        }
        current_label = label;

        // 오버랩 제거: 이전 청크의 end_offset과 현재 청크의 start_offset 비교
        let overlap = if prev_end_offset > chunk.start_offset && prev_end_offset > 0 {
            // 오버랩 바이트 수 = 이전 끝 - 현재 시작
            (prev_end_offset - chunk.start_offset) as usize
        } else {
            0
        };

        if overlap > 0 && overlap < chunk.content.len() {
            // 오버랩 구간을 건너뛰고 나머지만 추가
            // char 경계 안전하게 처리
            let content_chars: Vec<char> = chunk.content.chars().collect();
            if overlap < content_chars.len() {
                let trimmed: String = content_chars[overlap..].iter().collect();
                current_text.push_str(&trimmed);
            }
        } else if overlap == 0 {
            // 오버랩 없음 — 갭이 있으면 줄바꿈 추가
            if prev_end_offset > 0 && chunk.start_offset > prev_end_offset {
                current_text.push('\n');
            }
            current_text.push_str(&chunk.content);
        }
        // overlap >= content.len() → 완전 중복 청크, 스킵

        prev_end_offset = chunk.end_offset;
    }

    // 마지막 섹션 저장
    if !current_text.is_empty() {
        sections.push(PreviewSection {
            label: current_label,
            content: current_text,
        });
    }

    sections
}

// ======================== 북마크 ========================

/// 북마크 정보 (프론트엔드용)
#[derive(Debug, Serialize)]
pub struct BookmarkInfo {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub content_preview: String,
    pub page_number: Option<i64>,
    pub location_hint: Option<String>,
    pub note: Option<String>,
    pub created_at: i64,
}

/// 북마크 추가
#[tauri::command]
pub async fn add_bookmark(
    file_path: String,
    content_preview: String,
    page_number: Option<i64>,
    location_hint: Option<String>,
    note: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<i64> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<i64> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let file_name = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        conn.execute(
            "INSERT INTO bookmarks (file_path, file_name, content_preview, page_number, location_hint, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(file_path) DO UPDATE SET
                content_preview = excluded.content_preview,
                page_number = excluded.page_number,
                location_hint = excluded.location_hint,
                note = COALESCE(bookmarks.note, excluded.note),
                created_at = excluded.created_at",
            rusqlite::params![file_path, file_name, content_preview, page_number, location_hint, note, now],
        )
        .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        // UPSERT 후 정확한 ID 조회 (ON CONFLICT UPDATE 시 last_insert_rowid 부정확)
        let id: i64 = conn
            .query_row(
                "SELECT id FROM bookmarks WHERE file_path = ?",
                rusqlite::params![file_path],
                |row| row.get(0),
            )
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(id)
    })
    .await??;

    Ok(result)
}

/// 북마크 삭제
#[tauri::command]
pub async fn remove_bookmark(id: i64, state: State<'_, RwLock<AppContainer>>) -> ApiResult<()> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        conn.execute("DELETE FROM bookmarks WHERE id = ?", rusqlite::params![id])
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(())
    })
    .await??;

    Ok(())
}

/// 북마크 메모 수정
#[tauri::command]
pub async fn update_bookmark_note(
    id: i64,
    note: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<()> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    tokio::task::spawn_blocking(move || -> ApiResult<()> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        conn.execute(
            "UPDATE bookmarks SET note = ? WHERE id = ?",
            rusqlite::params![note, id],
        )
        .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;
        Ok(())
    })
    .await??;

    Ok(())
}

/// 모든 북마크 조회
#[tauri::command]
pub async fn get_bookmarks(state: State<'_, RwLock<AppContainer>>) -> ApiResult<Vec<BookmarkInfo>> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<Vec<BookmarkInfo>> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, file_path, file_name, content_preview, page_number, location_hint, note, created_at
                 FROM bookmarks ORDER BY created_at DESC",
            )
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkInfo {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_name: row.get(2)?,
                    content_preview: row.get(3)?,
                    page_number: row.get(4)?,
                    location_hint: row.get(5)?,
                    note: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let bookmarks: Vec<BookmarkInfo> = rows
            .filter_map(|r| r.ok())
            .collect();

        Ok(bookmarks)
    })
    .await??;

    Ok(result)
}

// ======================== 요약 ========================

/// 요약 문장 (프론트엔드용)
#[derive(Debug, Serialize)]
pub struct SummarySentence {
    /// 문장 텍스트
    pub text: String,
    /// TextRank 스코어
    pub score: f32,
    /// 원본 문장 순서 (0-based)
    pub original_index: usize,
    /// 해당 문장이 속한 페이지 번호
    pub page_number: Option<i64>,
    /// 위치 힌트
    pub location_hint: Option<String>,
}

/// 요약 응답
#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    /// 요약 문장 목록 (원문 순서)
    pub sentences: Vec<SummarySentence>,
    /// 전체 문장 수
    pub total_sentences: usize,
    /// 생성 시간 (ms)
    pub generation_time_ms: u64,
}

/// 문서 요약 생성 (TextRank 추출적 요약)
#[tauri::command]
pub async fn generate_summary(
    file_path: String,
    num_sentences: Option<usize>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<SummaryResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    let num = num_sentences.unwrap_or(3).min(10);

    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let result = tokio::task::spawn_blocking(move || -> ApiResult<SummaryResponse> {
        let start = std::time::Instant::now();
        let conn = db::get_connection(std::path::Path::new(&db_path))?;

        // 1. 청크 로드
        let chunk_ids = db::get_chunk_ids_for_path(&conn, &file_path)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        if chunk_ids.is_empty() {
            return Ok(SummaryResponse {
                sentences: vec![],
                total_sentences: 0,
                generation_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        let chunk_infos = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let mut sorted = chunk_infos;
        sorted.sort_by_key(|c| c.chunk_index);

        // 2. 전체 텍스트 병합 + 청크별 위치 매핑
        // (문자 오프셋 → 페이지/위치 힌트)
        let mut full_text = String::new();
        let mut chunk_ranges: Vec<(usize, usize, Option<i64>, Option<String>)> = Vec::new();

        for chunk in &sorted {
            let start_offset = full_text.len();
            full_text.push_str(&chunk.content);
            full_text.push('\n');
            let end_offset = full_text.len();
            chunk_ranges.push((
                start_offset,
                end_offset,
                chunk.page_number,
                chunk.location_hint.clone(),
            ));
        }

        // 3. TextRank 요약
        let ranked = textrank::summarize(&full_text, num);

        // 4. 각 요약 문장에 페이지/위치 매핑
        let total_sentences = textrank::count_sentences(&full_text);

        let sentences: Vec<SummarySentence> = ranked
            .into_iter()
            .map(|rs| {
                // 문장 텍스트가 어느 청크에 속하는지 찾기
                let sentence_pos = full_text.find(&rs.text).unwrap_or(0);
                let (page_number, location_hint) = chunk_ranges
                    .iter()
                    .find(|(start, end, _, _)| sentence_pos >= *start && sentence_pos < *end)
                    .map(|(_, _, pn, lh)| (*pn, lh.clone()))
                    .unwrap_or((None, None));

                SummarySentence {
                    text: rs.text,
                    score: rs.score,
                    original_index: rs.original_index,
                    page_number,
                    location_hint,
                }
            })
            .collect();

        Ok(SummaryResponse {
            sentences,
            total_sentences,
            generation_time_ms: start.elapsed().as_millis() as u64,
        })
    })
    .await??;

    Ok(result)
}
