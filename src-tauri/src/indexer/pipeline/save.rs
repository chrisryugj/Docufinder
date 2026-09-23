//! 문서 DB 저장(FTS 전용, 트랜잭션 없음)과 단일 파일 FTS 인덱싱.

use super::{IndexError, IndexResult, FTS_TOKENIZER};
use crate::db;
use crate::indexer::collector::save_file_metadata_only;
use crate::ocr::OcrEngine;
use crate::parsers::{parse_file, ParsedDocument};
use crate::tokenizer::TextTokenizer;

use rusqlite::Connection;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// 문서를 DB에 저장 - FTS만 (트랜잭션 없음, 배치용)
///
/// `tokenizer`: 형태소 분석기가 있으면 FTS에 형태소 토큰도 함께 인덱싱.
/// unicode61 토크나이저의 한국어 토큰화 한계를 보완하여 검색 재현율 향상.
/// `precomputed_tokens`: 파싱 풀이 선계산한 청크별 형태소 토큰 (T3-3).
/// `Some` 이면 인라인 tokenize 를 건너뛴다 (배치 파이프라인 경로).
pub(crate) fn save_document_to_db_fts_only_no_tx(
    conn: &Connection,
    path: &Path,
    document: ParsedDocument,
    tokenizer: Option<&dyn crate::tokenizer::TextTokenizer>,
    vector_index: Option<&crate::search::vector::VectorIndex>,
    precomputed_tokens: Option<Vec<Option<String>>>,
) -> Result<usize, IndexError> {
    let path_str = path.to_string_lossy().to_string();

    let metadata = fs::metadata(path).map_err(|e| IndexError::IoError(e.to_string()))?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_type = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // upsert_file_fts_only 사용 (vector_indexed_at = NULL)
    let file_id =
        db::upsert_file_fts_only(conn, &path_str, &file_name, &file_type, size, modified_at)
            .map_err(|e| IndexError::DbError(e.to_string()))?;

    // Lineage 부여 — 같은 stem/폴더의 기존 canonical과 점수 비교 후 승자 지정
    if let Err(e) = crate::indexer::lineage::assign_for_file(
        conn,
        file_id,
        &path_str,
        &file_name,
        Some(modified_at),
    ) {
        tracing::warn!("lineage assign failed for {}: {}", path_str, e);
    }

    // 재인덱싱 시 구 청크의 벡터를 먼저 제거 (이슈 #34 후속).
    // chunks.id(rowid)는 AUTOINCREMENT가 아니라 삭제된 id가 재사용될 수 있는데,
    // 벡터를 남겨두면 vector_worker의 contains_chunk 스킵이 옛 내용의 임베딩을
    // 새 청크에 오귀속시킨다. 재사용이 안 되어도 고아 벡터가 usearch에 누적된다.
    // (제거가 커밋 전이라 강제종료+롤백 교차 시 벡터 누락 가능성이 있으나,
    //  이는 다음 재인덱싱에서 회복되는 "누락"이지 "오답"이 아니다.)
    if let Some(vi) = vector_index {
        match db::get_chunk_ids_for_file(conn, file_id) {
            Ok(old_chunk_ids) => {
                for chunk_id in old_chunk_ids {
                    if let Err(e) = vi.remove(chunk_id) {
                        tracing::debug!("stale vector remove failed {}: {}", chunk_id, e);
                    }
                }
            }
            Err(e) => tracing::warn!("stale vector 조회 실패 {}: {}", path_str, e),
        }
    }

    // _no_tx 버전 사용: 호출자(index_folder_fts_only)가 이미 트랜잭션을 관리하므로
    // 중첩 BEGIN 방지 (SQLite는 중첩 트랜잭션 미지원)
    db::delete_chunks_for_file_no_tx(conn, file_id)
        .map_err(|e| IndexError::DbError(e.to_string()))?;

    let chunks_count = document.chunks.len();

    // 복사 시 깨지는 문서(PDF CID/ToUnicode 누락, HWP/HWPX PUA 커스텀폰트) 판정 —
    // 청크 본문을 이어붙여 looks_like_garbage_text 로 검사한다. chunks 를 소비하기
    // 전에 계산해야 하므로 into_iter() 루프 앞에서 수행.
    //
    // 판정 자체를 pdf/hwp/hwpx 로 게이팅한다. 판정기는 "한글/라틴이 지배적이지 않으면
    // 깨짐"으로도 보므로 한자 지배 문서(중/일)를 오탐하는데, 이 CID/PUA 깨짐은 pdf 와 한글
    // 오피스 계열(hwp·hwpx)에서만 실제로 발생한다. hwpx 도 커스텀폰트 PUA 코드포인트가
    // 그대로 실려 깨질 수 있어 포함한다(유니코드 네이티브라도 PUA 는 유니코드다 — hwpx 파서는
    // PUA 를 걸러내지 않는다). 그 외 타입(docx/xlsx/txt 등)은 판정 생략 → 중/일 문서 오탐
    // 배지를 원천 차단(항상 false = 깨끗함).
    let garbled_flag = if matches!(file_type.as_str(), "pdf" | "hwp" | "hwpx") {
        // 파서 힌트 OR — OCR 이 본문을 대체하면 chunks 는 깨끗해져 아래 판정이 놓치지만,
        // 원본 텍스트층이 깨졌다는 사실(복사 시 깨짐)은 그대로다 (kordoc pageQuality /
        // Rust PDF 파서의 페이지별 판정에서 전달).
        document.garbled_hint || {
            let joined: String = document
                .chunks
                .iter()
                .map(|c| c.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            crate::parsers::pdf::looks_like_garbage_text(&joined)
        }
    } else {
        false
    };

    let mut tokenize_panics: usize = 0;
    let mut precomputed_tokens = precomputed_tokens;

    for (idx, chunk) in document.chunks.into_iter().enumerate() {
        // 형태소 분석기가 있으면 FTS에 형태소 토큰도 함께 저장.
        // 배치 경로는 파싱 풀 선계산분(precomputed_tokens)을 사용 (T3-3) —
        // 항목 None = 파싱 풀에서의 tokenize panic (형태소 없이 인덱싱).
        // 그 외 경로(감시자 단건 등)는 기존처럼 인라인 tokenize 하되, lindera 가
        // 특정 입력에서 panic 하는 사례 (BENIGN_PANIC_SOURCES 에 등재)에 대비해
        // 청크 단위로 catch_unwind. panic 시 형태소 토큰 없이 진행 — 검색 재현율은
        // 살짝 낮아지지만 인덱싱 자체는 성공 (강제종료 회피가 우선).
        let extra_tokens = match precomputed_tokens.as_mut() {
            Some(tokens) => {
                let t = tokens.get_mut(idx).and_then(Option::take);
                if t.is_none() {
                    tokenize_panics += 1;
                }
                t
            }
            None => tokenizer.and_then(|tok| {
                let content = &chunk.content;
                match catch_unwind(AssertUnwindSafe(|| tok.tokenize(content))) {
                    Ok(morphemes) => Some(morphemes.join(" ")),
                    Err(_) => {
                        tokenize_panics += 1;
                        None
                    }
                }
            }),
        };

        db::insert_chunk(
            conn,
            file_id,
            idx,
            &chunk.content,
            chunk.start_offset,
            chunk.end_offset,
            chunk.page_number,
            chunk.page_end,
            chunk.location_hint.as_deref(),
            extra_tokens.as_deref(),
        )
        .map_err(|e| IndexError::DbError(e.to_string()))?;
    }

    if tokenize_panics > 0 {
        tracing::warn!(
            "[FTS] Tokenizer panicked on {}/{} chunks of {} (indexed without morphemes)",
            tokenize_panics,
            chunks_count,
            path_str
        );
    }

    // 복사 시 깨짐 표식 저장 — 검색 결과/미리보기 배지용. 실패해도 인덱싱 자체는
    // 성공으로 두어야 하므로(배지는 부가 정보) lineage assign 과 동일하게 warn 후 진행.
    if let Err(e) = db::set_file_garbled(conn, file_id, garbled_flag) {
        tracing::warn!("set_file_garbled failed for {}: {}", path_str, e);
    }

    tracing::debug!("[FTS] Indexed: {} ({} chunks)", path_str, chunks_count);

    Ok(chunks_count)
}

// ==================== 단일 파일 FTS 인덱싱 (manager용) ====================

/// 단일 파일 FTS 인덱싱 (트랜잭션 없음) - WatchManager 배치 처리용
///
/// 호출자가 BEGIN/COMMIT을 관리해야 함.
pub(crate) fn index_file_fts_only_no_tx(
    conn: &Connection,
    path: &Path,
    ocr_engine: Option<&OcrEngine>,
    vector_index: Option<&crate::search::vector::VectorIndex>,
) -> Result<IndexResult, IndexError> {
    index_file_fts_only_no_tx_opts(conn, path, ocr_engine, vector_index, false)
}

/// `force_ocr`: PDF 를 kordoc `--ocr-force`(전 페이지 강제 재인식)로 파싱 —
/// "OCR로 다시 읽기" 단건 재인덱싱 전용.
pub(crate) fn index_file_fts_only_no_tx_opts(
    conn: &Connection,
    path: &Path,
    ocr_engine: Option<&OcrEngine>,
    vector_index: Option<&crate::search::vector::VectorIndex>,
    force_ocr: bool,
) -> Result<IndexResult, IndexError> {
    let parsed = if force_ocr {
        crate::parsers::parse_file_force_ocr(path, ocr_engine)
    } else {
        parse_file(path, ocr_engine)
    };
    let document = match parsed {
        Ok(doc) => doc,
        Err(crate::parsers::ParseError::CloudPlaceholder(_)) => {
            // 클라우드 placeholder: 본문 hydrate 회피, 메타데이터만 저장.
            save_file_metadata_only(conn, path, vector_index)
                .map_err(|e| IndexError::DbError(e.to_string()))?;
            return Ok(IndexResult {
                file_path: path.to_string_lossy().to_string(),
                chunks_count: 0,
                vectors_count: 0,
                total_chars: 0,
            });
        }
        Err(e) => return Err(IndexError::ParseError(e.to_string())),
    };
    // parse_file 이 전문 content 를 비우므로(T2-5) 청크 길이합으로 집계
    // (청크 overlap 만큼 과대집계되나 이 필드는 통계 메타데이터일 뿐이다)
    let total_chars = document.chunks.iter().map(|c| c.content.len()).sum();

    let chunks_count = save_document_to_db_fts_only_no_tx(
        conn,
        path,
        document,
        FTS_TOKENIZER.as_ref().map(|t| t as &dyn TextTokenizer),
        vector_index,
        None, // 단건 경로 — 인라인 tokenize (파싱 풀 없음)
    )?;

    Ok(IndexResult {
        file_path: path.to_string_lossy().to_string(),
        chunks_count,
        vectors_count: 0,
        total_chars,
    })
}
