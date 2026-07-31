//! 문서 미리보기 + 북마크 + 요약 커맨드

use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::AppContainer;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use tauri::State;

/// 프리뷰 경로 검증: canonicalize + 감시 폴더 내 경로인지 확인.
///
/// canonicalize·DB 연결·폴더 조회 전부 블로킹 I/O — render_pdf_page 가 페이지마다
/// 호출하므로 spawn_blocking 으로 옮겨 async 런타임 스레드를 막지 않는다.
async fn validate_preview_path(
    file_path: &str,
    state: &State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };
    let fp = file_path.to_string();

    tokio::task::spawn_blocking(move || -> ApiResult<String> {
        // 1. 경로 정규화 (path traversal 방지)
        let canonical = std::fs::canonicalize(&fp)
            .map_err(|_| ApiError::Validation("파일을 찾을 수 없습니다".to_string()))?;
        let canonical_str = canonical.to_string_lossy().to_string();

        // 2. 감시 폴더 내 경로인지 확인 (화이트리스트, 감시 폴더 미등록 시 거부)
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
    })
    .await?
}

// ======================== PDF 미리보기 소스 캐시 ========================

/// PDF 미리보기 본문 소스 판별 결과 (kordoc 원본 파싱 vs DB 청크).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfPreviewSource {
    Kordoc,
    Db,
}

/// 판별 캐시 엔트리 상한 — 초과 시 전체 비움 (엔트리가 작아 정교한 LRU 불필요)
const PDF_SOURCE_CACHE_CAP: usize = 256;

/// path+mtime → 소스 판별 인메모리 캐시. 판별 휴리스틱은 kordoc 전체 파싱과
/// DB 청크 로드를 둘 다 요구하므로, 재방문 시 판별 결과를 재사용해 한쪽만 로드한다.
static PDF_SOURCE_CACHE: OnceLock<Mutex<HashMap<(String, i64), PdfPreviewSource>>> =
    OnceLock::new();

fn pdf_source_cache() -> &'static Mutex<HashMap<(String, i64), PdfPreviewSource>> {
    PDF_SOURCE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pdf_source_cache_get(key: &(String, i64)) -> Option<PdfPreviewSource> {
    pdf_source_cache().lock().ok()?.get(key).copied()
}

fn pdf_source_cache_put(key: (String, i64), source: PdfPreviewSource) {
    if let Ok(mut map) = pdf_source_cache().lock() {
        if map.len() >= PDF_SOURCE_CACHE_CAP && !map.contains_key(&key) {
            map.clear();
        }
        map.insert(key, source);
    }
}

fn pdf_source_cache_remove(key: &(String, i64)) {
    if let Ok(mut map) = pdf_source_cache().lock() {
        map.remove(key);
    }
}

/// 파일 수정 시각 (Unix seconds) — 캐시 키용. 실패 시 None (캐시 미사용).
fn file_mtime_secs(path: &str) -> Option<i64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
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
    /// 열기 암호가 필요한 문서 — 프론트가 비밀번호 입력을 띄운다.
    /// password 를 주고 재호출했는데도 true 면 입력한 암호가 틀린 것.
    #[serde(default)]
    pub needs_password: bool,
}

/// kordoc으로 파일의 마크다운을 직접 추출 (미리보기 렌더링용)
///
/// DB 청크가 아닌 원본 파일을 직접 파싱하여 완전한 마크다운을 반환한다.
/// kordoc 미지원 또는 실패 시 DB 청크 병합 텍스트로 fallback.
#[tauri::command]
pub async fn load_markdown_preview(
    file_path: String,
    // password: 프론트가 비밀번호 입력을 받아 재호출할 때만 전달한다
    password: Option<String>,
    state: State<'_, RwLock<AppContainer>>,
) -> ApiResult<MarkdownPreviewResponse> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Validation("파일 경로가 비어있습니다".to_string()));
    }

    // 경로 검증: canonicalize + 감시 폴더 화이트리스트
    let fp = validate_preview_path(&file_path, &state).await?;

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

    let result = if is_pdf {
        // PDF 는 판별 휴리스틱 + path+mtime 캐시 (이중 파싱 방지)
        load_pdf_markdown(&fp, &file_path, &state).await
    } else {
        parse_kordoc_markdown(fp.clone(), ext_lower.clone(), password.clone()).await
    };

    match result {
        Ok(markdown) => {
            let garbled = crate::parsers::pdf::looks_like_garbage_text(&markdown);
            Ok(MarkdownPreviewResponse {
                file_path,
                file_name,
                markdown,
                garbled,
                needs_password: false,
            })
        }
        Err(e) => {
            // 암호 문서는 DB 청크로 조용히 대체하지 않는다 — 본문이 없는 이유를 알려야
            // 사용자가 비밀번호를 넣어 다시 열 수 있다.
            let needs_password = matches!(e, ApiError::PasswordProtected(_));
            let markdown = if needs_password {
                String::new()
            } else {
                fetch_db_markdown(&file_path, &state)
                    .await
                    .unwrap_or_default()
            };
            let garbled = crate::parsers::pdf::looks_like_garbage_text(&markdown);
            Ok(MarkdownPreviewResponse {
                file_path,
                file_name,
                markdown,
                garbled,
                needs_password,
            })
        }
    }
}

/// kordoc으로 마크다운 직접 추출 (블로킹 파싱 — spawn_blocking).
/// kordoc 미지원/실패 시 Err → 호출부가 DB 청크로 fallback.
async fn parse_kordoc_markdown(
    fp: String,
    ext: String,
    password: Option<String>,
) -> ApiResult<String> {
    tokio::task::spawn_blocking(move || -> ApiResult<String> {
        let path = std::path::Path::new(&fp);

        let kordoc_exts = ["hwp", "hwpx", "docx", "pdf"];
        if kordoc_exts.contains(&ext.as_str()) && crate::parsers::kordoc::is_available() {
            let result = match password.as_deref() {
                Some(pw) => crate::parsers::kordoc::get_markdown_with_password(path, pw),
                None => crate::parsers::kordoc::get_markdown(path),
            };
            match result {
                Ok(md) => {
                    tracing::info!("preview: kordoc 성공 ({}자) — {}", md.len(), fp);
                    return Ok(md);
                }
                Err(crate::parsers::ParseError::PasswordProtected(msg)) => {
                    // 비밀번호가 필요/불일치 — DB 청크 fallback 대신 그대로 올려보낸다
                    tracing::info!("preview: 암호 문서 — {} — {}", fp, msg);
                    return Err(ApiError::PasswordProtected(msg));
                }
                Err(e) => {
                    tracing::warn!("preview: kordoc 실패, fallback 사용 — {} — {:?}", fp, e);
                }
            }
        } else {
            tracing::debug!(
                "preview: kordoc 미사용 (ext={}, available={}) — {}",
                ext,
                crate::parsers::kordoc::is_available(),
                fp
            );
        }

        // fallback: DB 청크 병합
        Err(ApiError::IndexingFailed("kordoc 미사용".to_string()))
    })
    .await?
}

/// PDF 미리보기 본문 결정 — 세 가지 이슈 대응:
///  (1) 스캔본: kordoc 은 임베디드 텍스트만(짧음) → DB(OCR) 사용
///  (2) CID 디코딩 실패: kordoc 이 쓰레기 유니코드 반환 → DB 사용
///  (3) 정상: kordoc 사용
///
/// 판별에는 kordoc 전체 파싱 + DB 청크 로드가 둘 다 필요하므로, 결과를
/// path+mtime 캐시에 저장해 재방문 시 판별된 소스 한쪽만 로드한다.
/// 캐시된 소스가 비어 있으면(재인덱싱 등) 엔트리를 무효화하고 전체 판별로 복귀.
async fn load_pdf_markdown(
    fp: &str,
    orig_path: &str,
    state: &State<'_, RwLock<AppContainer>>,
) -> ApiResult<String> {
    let mtime = {
        let p = fp.to_string();
        tokio::task::spawn_blocking(move || file_mtime_secs(&p)).await?
    };
    let key = mtime.map(|mt| (fp.to_string(), mt));

    // 캐시 히트: 판별 생략, 해당 소스만 로드
    if let Some(k) = &key {
        match pdf_source_cache_get(k) {
            Some(PdfPreviewSource::Db) => {
                let db_md = fetch_db_markdown(orig_path, state)
                    .await
                    .unwrap_or_default();
                if !db_md.is_empty() {
                    return Ok(db_md);
                }
                pdf_source_cache_remove(k);
            }
            Some(PdfPreviewSource::Kordoc) => {
                if let Ok(md) = parse_kordoc_markdown(fp.to_string(), "pdf".to_string(), None).await
                {
                    if !md.is_empty() && !crate::parsers::pdf::looks_like_garbage_text(&md) {
                        return Ok(md);
                    }
                }
                pdf_source_cache_remove(k);
            }
            None => {}
        }
    }

    // 전체 판별 (기존 휴리스틱 그대로)
    let kordoc_md = parse_kordoc_markdown(fp.to_string(), "pdf".to_string(), None)
        .await
        .unwrap_or_default();
    let db_md = fetch_db_markdown(orig_path, state)
        .await
        .unwrap_or_default();
    let kordoc_len = kordoc_md.chars().count();
    let db_len = db_md.chars().count();
    let kordoc_garbage = crate::parsers::pdf::looks_like_garbage_text(&kordoc_md);
    let much_longer_in_db = db_len > kordoc_len.saturating_mul(2).max(kordoc_len + 500);

    let (source, result) = if kordoc_garbage && !db_md.is_empty() {
        tracing::info!(
            "preview: PDF CID 깨짐 감지 — DB 사용 (kordoc {}자, DB {}자)",
            kordoc_len,
            db_len
        );
        (Some(PdfPreviewSource::Db), Ok(db_md))
    } else if much_longer_in_db {
        tracing::info!(
            "preview: PDF OCR 감지 — DB 사용 (kordoc {}자 vs DB {}자)",
            kordoc_len,
            db_len
        );
        (Some(PdfPreviewSource::Db), Ok(db_md))
    } else if !kordoc_md.is_empty() && !kordoc_garbage {
        (Some(PdfPreviewSource::Kordoc), Ok(kordoc_md))
    } else if !db_md.is_empty() {
        (Some(PdfPreviewSource::Db), Ok(db_md))
    } else {
        (None, Err(ApiError::IndexingFailed("본문 없음".to_string())))
    };

    if let (Some(k), Some(s)) = (key, source) {
        pdf_source_cache_put(k, s);
    }

    result
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
    let fp = validate_preview_path(&file_path, &state).await?;

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
    let fp = validate_preview_path(&file_path, &state).await?;
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

/// 고아 북마크 정리 중복 실행 방지 플래그 (죽은 UNC 경로에서 정리가 오래 걸릴 때 중첩 방지)
static ORPHAN_CLEANUP_RUNNING: AtomicBool = AtomicBool::new(false);

/// 모든 북마크 조회.
///
/// 응답을 먼저 반환하고, 삭제된 파일의 고아 레코드 정리는 별도 백그라운드
/// 태스크에서 수행한다 — 북마크당 `Path::exists()` 검사가 죽은 UNC 경로에서
/// 분 단위로 블로킹될 수 있어, 조회 응답과 분리한다. (고아 북마크는 정리
/// 완료 후 다음 조회부터 제외됨)
#[tauri::command]
pub async fn get_bookmarks(state: State<'_, RwLock<AppContainer>>) -> ApiResult<Vec<BookmarkInfo>> {
    let db_path = {
        let container = state.read()?;
        container.db_path.to_string_lossy().to_string()
    };

    let db_path_for_query = db_path.clone();
    let result = tokio::task::spawn_blocking(move || -> ApiResult<Vec<BookmarkInfo>> {
        let conn = db::get_connection(std::path::Path::new(&db_path_for_query))?;

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

        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;

    // 고아 정리 백그라운드 kick-off (이미 실행 중이면 스킵)
    if !result.is_empty()
        && ORPHAN_CLEANUP_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let entries: Vec<(i64, String)> =
            result.iter().map(|b| (b.id, b.file_path.clone())).collect();
        tokio::task::spawn_blocking(move || {
            cleanup_orphan_bookmarks(&db_path, &entries);
            ORPHAN_CLEANUP_RUNNING.store(false, Ordering::Release);
        });
    }

    Ok(result)
}

/// 파일이 사라진 북마크를 DB에서 제거 (블로킹 — 백그라운드 전용)
fn cleanup_orphan_bookmarks(db_path: &str, entries: &[(i64, String)]) {
    let orphan_ids: Vec<i64> = entries
        .iter()
        .filter(|(_, path)| !std::path::Path::new(path).exists())
        .map(|(id, _)| *id)
        .collect();
    if orphan_ids.is_empty() {
        return;
    }

    let conn = match db::get_connection(std::path::Path::new(db_path)) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Orphan bookmark cleanup: DB connection failed — {}", e);
            return;
        }
    };
    let placeholders: String = orphan_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("DELETE FROM bookmarks WHERE id IN ({})", placeholders);
    let prepared = conn.prepare(&sql);
    if let Ok(mut del_stmt) = prepared {
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = orphan_ids
            .iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let deleted = del_stmt.execute(param_refs.as_slice()).unwrap_or(0);
        tracing::info!(
            "Cleaned up {} orphaned bookmarks (files no longer exist)",
            deleted
        );
    }
}
