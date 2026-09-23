use super::*;
use crate::parsers::{DocumentChunk, DocumentMetadata, ParsedDocument};
use crate::search::vector::VectorIndex;

fn doc(text: &str) -> ParsedDocument {
    ParsedDocument {
        content: text.to_string(),
        metadata: DocumentMetadata {
            title: None,
            author: None,
            created_at: None,
            page_count: None,
        },
        chunks: vec![DocumentChunk {
            content: text.to_string(),
            start_offset: 0,
            end_offset: text.len(),
            page_number: None,
            page_end: None,
            location_hint: None,
        }],
        garbled_hint: false,
    }
}

/// 이슈 #34 후속: 재저장(재인덱싱) 시 구 청크의 벡터를 제거하지 않으면
/// chunks.id(rowid) 재사용 시 옛 임베딩이 새 내용에 오귀속된다.
#[test]
fn resave_removes_stale_chunk_vectors() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("t.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    crate::db::migrate_schema(&conn, &db_path).unwrap();

    let file = dir.path().join("a.txt");
    std::fs::write(&file, "v1 내용").unwrap();

    // 1차 저장 (벡터 인덱스 없이) → 청크 id 확보
    save_document_to_db_fts_only_no_tx(&conn, &file, doc("v1 내용"), None, None, None).unwrap();
    let file_id: i64 = conn
        .query_row(
            "SELECT id FROM files WHERE path = ?",
            [file.to_string_lossy()],
            |r| r.get(0),
        )
        .unwrap();
    let old_ids = crate::db::get_chunk_ids_for_file(&conn, file_id).unwrap();
    assert!(!old_ids.is_empty());

    // 구 청크 임베딩 등록
    let vi = VectorIndex::new(&dir.path().join("vec.usearch")).unwrap();
    let emb = vec![0.1_f32; crate::embedder::EMBEDDING_DIM];
    for id in &old_ids {
        vi.add(*id, &emb).unwrap();
    }
    assert!(old_ids.iter().all(|id| vi.contains_chunk(*id)));

    // 2차 저장 (재인덱싱) — 구 벡터가 제거되어야 rowid 재사용 시 오귀속이 없다
    std::fs::write(&file, "v2 완전히 다른 내용").unwrap();
    save_document_to_db_fts_only_no_tx(
        &conn,
        &file,
        doc("v2 완전히 다른 내용"),
        None,
        Some(&vi),
        None,
    )
    .unwrap();
    assert!(
        old_ids.iter().all(|id| !vi.contains_chunk(*id)),
        "재저장 후 구 청크 벡터가 남아있음"
    );
}
