//! 파서 단위 테스트
//!
//! 각 파일 포맷별 파싱 및 청크 생성 테스트

use std::path::Path;

// TXT 파서 테스트
mod txt_tests {
    use super::*;

    #[test]
    fn test_txt_parse() {
        let path = Path::new("tests/fixtures/sample.txt");
        if !path.exists() {
            eprintln!("Test file not found: {:?}", path);
            return;
        }

        let result = docufinder_lib::parsers::txt::parse(path);
        assert!(result.is_ok(), "TXT parsing failed: {:?}", result.err());

        let doc = result.unwrap();
        assert!(!doc.content.is_empty(), "Content should not be empty");
        assert!(!doc.chunks.is_empty(), "Should have at least one chunk");
        assert!(
            doc.content.contains("DocuFinder"),
            "Content should contain 'DocuFinder'"
        );

        // 청크 검증
        for chunk in &doc.chunks {
            assert!(
                !chunk.content.is_empty(),
                "Chunk content should not be empty"
            );
            assert!(
                chunk.end_offset > chunk.start_offset,
                "end_offset should be greater than start_offset"
            );
        }
    }

    #[test]
    fn test_txt_empty_file() {
        use std::fs;
        use std::io::Write;

        // 임시 빈 파일 생성
        let temp_path = Path::new("tests/fixtures/empty.txt");
        let mut file = fs::File::create(temp_path).unwrap();
        file.write_all(b"").unwrap();

        let result = docufinder_lib::parsers::txt::parse(temp_path);
        assert!(result.is_ok());

        let doc = result.unwrap();
        assert!(doc.content.is_empty());
        assert!(doc.chunks.is_empty());

        // 정리
        fs::remove_file(temp_path).ok();
    }
}

// DOCX 파서 테스트
mod docx_tests {
    use super::*;

    #[test]
    fn test_docx_parse() {
        let path = Path::new("tests/fixtures/sample.docx");
        if !path.exists() {
            eprintln!("Skipping DOCX test: {:?} not found", path);
            return;
        }

        let result = docufinder_lib::parsers::docx::parse(path);
        assert!(result.is_ok(), "DOCX parsing failed: {:?}", result.err());

        let doc = result.unwrap();
        assert!(!doc.content.is_empty(), "Content should not be empty");
        assert!(!doc.chunks.is_empty(), "Should have at least one chunk");

        // 청크에 페이지 정보 확인
        for chunk in &doc.chunks {
            assert!(
                chunk.page_number.is_some(),
                "DOCX chunks should have page_number"
            );
            assert!(
                chunk.location_hint.is_some(),
                "DOCX chunks should have location_hint"
            );
        }
    }
}

// PDF 파서 테스트
mod pdf_tests {
    use super::*;

    #[test]
    fn test_pdf_parse() {
        let path = Path::new("tests/fixtures/sample.pdf");
        if !path.exists() {
            eprintln!("Skipping PDF test: {:?} not found", path);
            return;
        }

        let result = docufinder_lib::parsers::pdf::parse(path, None);
        assert!(result.is_ok(), "PDF parsing failed: {:?}", result.err());

        let doc = result.unwrap();
        assert!(!doc.content.is_empty(), "Content should not be empty");
        assert!(doc.metadata.page_count.is_some(), "Should have page count");

        // 청크에 페이지 정보 확인
        for chunk in &doc.chunks {
            assert!(
                chunk.page_number.is_some(),
                "PDF chunks should have page_number"
            );
            assert!(
                chunk
                    .location_hint
                    .as_ref()
                    .map(|h| h.contains("페이지"))
                    .unwrap_or(false),
                "PDF location_hint should contain '페이지'"
            );
        }
    }
}

// HWPX 파서 테스트
mod hwpx_tests {
    use super::*;

    #[test]
    fn test_hwpx_parse() {
        let path = Path::new("tests/fixtures/sample.hwpx");
        if !path.exists() {
            eprintln!("Skipping HWPX test: {:?} not found", path);
            return;
        }

        let result = docufinder_lib::parsers::hwpx::parse(path);
        assert!(result.is_ok(), "HWPX parsing failed: {:?}", result.err());

        let doc = result.unwrap();
        assert!(!doc.content.is_empty(), "Content should not be empty");
        assert!(
            doc.metadata.page_count.is_some(),
            "Should have section count"
        );

        // 청크에 섹션 정보 확인
        for chunk in &doc.chunks {
            assert!(
                chunk.page_number.is_some(),
                "HWPX chunks should have page_number"
            );
            assert!(
                chunk
                    .location_hint
                    .as_ref()
                    .map(|h| h.contains("섹션"))
                    .unwrap_or(false),
                "HWPX location_hint should contain '섹션'"
            );
        }
    }
}

// XLSX 파서 테스트
mod xlsx_tests {
    use super::*;

    #[test]
    fn test_xlsx_parse() {
        let path = Path::new("tests/fixtures/sample.xlsx");
        if !path.exists() {
            eprintln!("Skipping XLSX test: {:?} not found", path);
            return;
        }

        let result = docufinder_lib::parsers::xlsx::parse(path);
        assert!(result.is_ok(), "XLSX parsing failed: {:?}", result.err());

        let doc = result.unwrap();
        assert!(!doc.content.is_empty(), "Content should not be empty");
        assert!(!doc.chunks.is_empty(), "Should have at least one chunk");

        // 청크에 시트/행 정보 확인
        for chunk in &doc.chunks {
            assert!(
                chunk
                    .location_hint
                    .as_ref()
                    .map(|h| h.contains("!행"))
                    .unwrap_or(false),
                "XLSX location_hint should contain sheet and row info"
            );
        }
    }
}

// 청킹 로직 테스트
mod chunking_tests {
    #[test]
    fn test_chunk_text_basic() {
        let text = "a".repeat(1000);
        let chunks = docufinder_lib::parsers::chunk_text(&text, 512, 64);

        assert!(!chunks.is_empty());
        // 첫 번째 청크 크기 확인
        assert_eq!(chunks[0].content.len(), 512);
        // 오프셋 확인
        assert_eq!(chunks[0].start_offset, 0);
        assert_eq!(chunks[0].end_offset, 512);
    }

    #[test]
    fn test_chunk_text_small_content() {
        let text = "짧은 텍스트";
        let chunks = docufinder_lib::parsers::chunk_text(text, 512, 64);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, text);
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = docufinder_lib::parsers::chunk_text("", 512, 64);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_overlap() {
        let text = "a".repeat(1000);
        let chunks = docufinder_lib::parsers::chunk_text(&text, 512, 64);

        // 두 번째 청크는 첫 번째 청크 끝에서 64자 겹침
        if chunks.len() > 1 {
            let expected_start = 512 - 64; // 448
            assert_eq!(chunks[1].start_offset, expected_start);
        }
    }
}
