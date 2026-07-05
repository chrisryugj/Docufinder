//! 문서 미리보기 + 북마크 + 요약 커맨드

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use serde::Serialize;
use std::sync::RwLock;
use tauri::State;

/// 프리뷰 경로 검증: canonicalize + 감시 폴더 내 경로인지 확인
fn validate_preview_path(
    file_path: &str,
    state: &State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    // 1. 경로 정규화 (path traversal 방지)
    let canonical = std::fs::canonicalize(file_path)
        .map_err(|_| ApiError::Validation("파일을 찾을 수 없습니다".to_string()))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    // 2. 감시 폴더 내 경로인지 확인 (화이트리스트, 감시 폴더 미등록 시 거부)
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };
    let conn = db::get_connection(std::path::Path::new(&db_path))
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    let folders =
        db::get_watched_folders(&conn).map_err(|e| ApiError::Validation(e.to_string()))?;
    if folders.is_empty() {
        return Err(ApiError::Validation(
            "등록된 감시 폴더가 없어 미리보기할 수 없습니다".to_string(),
        ));
    }
    let in_scope = folders
        .iter()
        .any(|f| crate::utils::folder_scope::path_in_scope(&canonical_str, f));
    if !in_scope {
        return Err(ApiError::Validation(
            "감시 폴더 외부 파일은 미리보기할 수 없습니다".to_string(),
        ));
    }

    Ok(canonical_str)
}

// ======================== 미리보기 ========================

/// 미리보기 섹션 (오버랩 제거 후 병합된 연속 텍스트)
#[derive(Debug, Serialize)]
pub struct PreviewSection {
    /// 섹션 라벨 (페이지 번호, 시트명 등)
    pub label: Option<String>,
    /// 병합된 연속 텍스트
    pub content: String,
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

// ======================== 마크다운 미리보기 ========================

/// 마크다운 미리보기 응답
#[derive(Debug, Serialize)]
pub struct MarkdownPreviewResponse {
    pub file_path: String,
    pub file_name: String,
    pub markdown: String,
    /// 복사 시 글자가 깨지는 문서(PDF CID/ToUnicode 누락, HWP PUA 커스텀폰트)로 감지됨.
    /// 최종 반환 markdown(교체 후) 기준 판정 = "복사 시 실제로 깨지는가"와 일치.
    pub garbled: bool,
}

/// kordoc으로 파일의 마크다운을 직접 추출 (미리보기 렌더링용)
///
/// DB 청크가 아닌 원본 파일을 직접 파싱하여 완전한 마크다운을 반환한다.
/// kordoc 미지원 또는 실패 시 DB 청크 병합 텍스트로 fallback.
#[tauri::command]
pub async fn load_markdown_preview(
    file_path: String,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<MarkdownPreviewResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    // 경로 검증: canonicalize + 감시 폴더 화이트리스트
    let fp = validate_preview_path(&file_path, &state)?;

    let file_name = std::path::Path::new(&fp)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let ext_lower = std::path::Path::new(&fp)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_pdf = ext_lower == "pdf";

    let fp_for_kordoc = fp.clone();
    let ext_for_kordoc = ext_lower.clone();
    let result = tokio::task::spawn_blocking(move || -> ApiResult<String> {
        let path = std::path::Path::new(&fp_for_kordoc);

        let kordoc_exts = ["hwp", "hwpx", "docx", "pdf"];
        if kordoc_exts.contains(&ext_for_kordoc.as_str()) && crate::parsers::kordoc::is_available()
        {
            match crate::parsers::kordoc::get_markdown(path) {
                Ok(md) => {
                    tracing::info!("preview: kordoc 성공 ({}자) — {}", md.len(), fp_for_kordoc);
                    return Ok(md);
                }
                Err(e) => {
                    tracing::warn!(
                        "preview: kordoc 실패, fallback 사용 — {} — {:?}",
                        fp_for_kordoc,
                        e
                    );
                }
            }
        } else {
            tracing::debug!(
                "preview: kordoc 미사용 (ext={}, available={}) — {}",
                ext_for_kordoc,
                crate::parsers::kordoc::is_available(),
                fp_for_kordoc
            );
        }

        // fallback: DB 청크 병합
        Err(ApiError::IndexingFailed("kordoc 미사용".to_string()))
    })
    .await?;

    // PDF 는 세 가지 이슈 대응:
    //  (1) 스캔본: kordoc 은 임베디드 텍스트만(짧음) → DB(OCR) 사용
    //  (2) CID 디코딩 실패: kordoc 이 쓰레기 유니코드 반환 → DB 사용
    //  (3) 정상: kordoc 사용
    let result = if is_pdf {
        let kordoc_md = result.ok().unwrap_or_default();
        let db_md = fetch_db_markdown(&file_path, &state)
            .await
            .unwrap_or_default();
        let kordoc_len = kordoc_md.chars().count();
        let db_len = db_md.chars().count();
        let kordoc_garbage = crate::parsers::pdf::looks_like_garbage_text(&kordoc_md);
        let much_longer_in_db = db_len > kordoc_len.saturating_mul(2).max(kordoc_len + 500);

        if kordoc_garbage && !db_md.is_empty() {
            tracing::info!(
                "preview: PDF CID 깨짐 감지 — DB 사용 (kordoc {}자, DB {}자)",
                kordoc_len,
                db_len
            );
            Ok(db_md)
        } else if much_longer_in_db {
            tracing::info!(
                "preview: PDF OCR 감지 — DB 사용 (kordoc {}자 vs DB {}자)",
                kordoc_len,
                db_len
            );
            Ok(db_md)
        } else if !kordoc_md.is_empty() && !kordoc_garbage {
            Ok(kordoc_md)
        } else if !db_md.is_empty() {
            Ok(db_md)
        } else {
            Err(ApiError::IndexingFailed("본문 없음".to_string()))
        }
    } else {
        result
    };

    match result {
        Ok(markdown) => {
            let garbled = crate::parsers::pdf::looks_like_garbage_text(&markdown);
            Ok(MarkdownPreviewResponse {
                file_path,
                file_name,
                markdown,
                garbled,
            })
        }
        Err(_) => {
            let markdown = fetch_db_markdown(&file_path, &state)
                .await
                .unwrap_or_default();
            let garbled = crate::parsers::pdf::looks_like_garbage_text(&markdown);
            Ok(MarkdownPreviewResponse {
                file_path,
                file_name,
                markdown,
                garbled,
            })
        }
    }
}

// ======================== 레이아웃 미리보기 (SVG) ========================

/// 원본 조판 그대로의 SVG 로 렌더 (레이아웃 보기 토글).
/// HWPX 는 kordoc render(조판 보존 SVG, `highlight_query` 공백 구분 검색어를 형광펜 rect 로
/// 구워 넣음), HWP5(.hwp) 바이너리는 rhwp 네이티브 렌더(DocumentCore→SVG)로 분기한다.
///
/// HWPX: 한컴 저장/조판 캐시 없는 파일도 kordoc 이 reflow 로 렌더(캐시 있으면 재생), 실패 시 stderr 거절.
/// HWP: rhwp 는 검색어 형광펜을 지원하지 않아 `highlight_query` 를 무시한다(프론트 LayoutView 의
/// 인앱 찾기 Ctrl+F 는 계속 동작). 실패 시 프론트는 에러 토스트 후 마크다운 뷰를 유지한다.
#[tauri::command]
pub async fn render_layout_svg(
    file_path: String,
    highlight_query: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    // 경로 검증: canonicalize + 감시 폴더 화이트리스트 (마크다운 미리보기와 동일)
    let fp = validate_preview_path(&file_path, &state)?;

    let ext = std::path::Path::new(&fp)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let highlights: Vec<String> = highlight_query
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect();

    tokio::task::spawn_blocking(move || -> ApiResult<String> {
        let result = if ext == "hwp" {
            // HWP5 바이너리: rhwp 네이티브 렌더 (검색어 형광펜 미지원 — highlights 무시)
            crate::parsers::rhwp::render_svg(std::path::Path::new(&fp))
        } else {
            crate::parsers::kordoc::render_svg(std::path::Path::new(&fp), &highlights)
        };
        result.map_err(|e| match e {
            // ParseError Display 의 "Parse error:" 프리픽스 없이 진단 메시지만 토스트로
            crate::parsers::ParseError::ParseError(msg) => ApiError::CommandFailed(msg),
            other => ApiError::CommandFailed(other.to_string()),
        })
    })
    .await?
}

// ======================== PDF 레이아웃 미리보기 (페이지 이미지) ========================

/// PDF 한 페이지 렌더 응답 — data URI PNG + 총 페이지 수.
#[derive(Debug, Serialize)]
pub struct PdfPageResponse {
    /// data:image/png;base64,… (CSP img-src data: 허용 — 프론트 <img>에 직접 사용)
    pub data_url: String,
    pub page_count: usize,
    pub width: u32,
    pub height: u32,
}

/// PDF 페이지를 pdfium 으로 원본 조판 그대로 래스터화 → PNG data URI (원본 레이아웃 미리보기).
/// `page` 는 0-based. HWPX 의 render_layout_svg 에 대응하는 PDF 경로.
#[tauri::command]
pub async fn render_pdf_page(
    file_path: String,
    page: usize,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<PdfPageResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }
    // 경로 검증: canonicalize + 감시 폴더 화이트리스트 (다른 미리보기와 동일)
    let fp = validate_preview_path(&file_path, &state)?;
    let ext = std::path::Path::new(&fp)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "pdf" {
        return Err(ApiError::Validation("PDF 파일만 지원합니다".to_string()));
    }

    tokio::task::spawn_blocking(move || -> ApiResult<PdfPageResponse> {
        let render = crate::parsers::pdf::render_page_png(std::path::Path::new(&fp), page)
            .map_err(|e| ApiError::CommandFailed(e.to_string()))?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&render.png);
        Ok(PdfPageResponse {
            data_url: format!("data:image/png;base64,{}", b64),
            page_count: render.page_count,
            width: render.width,
            height: render.height,
        })
    })
    .await?
}

/// DB 청크를 병합해 마크다운 본문 생성 (스캔 PDF OCR 결과 복원용)
async fn fetch_db_markdown(
    file_path: &str,
    state: &State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };
    let fp = file_path.to_string();

    tokio::task::spawn_blocking(move || -> ApiResult<String> {
        let conn = db::get_connection(std::path::Path::new(&db_path))?;
        let chunk_ids = db::get_chunk_ids_for_path(&conn, &fp)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        if chunk_ids.is_empty() {
            return Ok(String::new());
        }

        let chunk_infos = db::get_chunks_by_ids(&conn, &chunk_ids)
            .map_err(|e| ApiError::DatabaseQuery(e.to_string()))?;

        let mut sorted = chunk_infos;
        sorted.sort_by_key(|c| c.chunk_index);

        let sections = merge_chunks_into_sections(&sorted);
        Ok(sections
            .into_iter()
            .map(|s| {
                if let Some(label) = s.label {
                    format!("## {}\n\n{}", label, s.content)
                } else {
                    s.content
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n"))
    })
    .await?
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

/// 모든 북마크 조회 (삭제된 파일의 고아 레코드 자동 정리)
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

        let all_bookmarks: Vec<BookmarkInfo> = rows
            .filter_map(|r| r.ok())
            .collect();

        // 고아 레코드 정리: 파일이 삭제된 북마크 자동 제거
        let orphan_ids: Vec<i64> = all_bookmarks
            .iter()
            .filter(|b| !std::path::Path::new(&b.file_path).exists())
            .map(|b| b.id)
            .collect();

        if !orphan_ids.is_empty() {
            let placeholders: String = orphan_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM bookmarks WHERE id IN ({})", placeholders);
            if let Ok(mut del_stmt) = conn.prepare(&sql) {
                let params: Vec<Box<dyn rusqlite::types::ToSql>> = orphan_ids
                    .iter()
                    .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
                    .collect();
                let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
                let deleted = del_stmt.execute(param_refs.as_slice()).unwrap_or(0);
                tracing::info!("Cleaned up {} orphaned bookmarks (files no longer exist)", deleted);
            }
        }

        let bookmarks: Vec<BookmarkInfo> = all_bookmarks
            .into_iter()
            .filter(|b| !orphan_ids.contains(&b.id))
            .collect();

        Ok(bookmarks)
    })
    .await??;

    Ok(result)
}
