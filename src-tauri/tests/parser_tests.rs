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

// 스캔/이미지 기반 PDF 래스터화 + OCR 테스트 (pdfium fallback)
mod scanned_pdf_ocr_tests {
    use super::*;
    use std::path::PathBuf;

    /// 합성 이미지 기반 PDF(CCITTFax 인코딩 → 임베디드 이미지 추출 불가)를 pdfium 으로
    /// 페이지 통째 래스터화한 뒤 PaddleOCR 로 한국어를 복원하는지 검증한다.
    ///
    /// 아래 조건을 모두 만족할 때만 실행하고, 하나라도 없으면 skip 한다(CI 는 동적 라이브러리
    /// 부재로 자동 skip — 회귀 게이트를 깨지 않음):
    ///   - PDFIUM_DYLIB_PATH: libpdfium 동적 라이브러리 경로(존재하는 파일)
    ///   - ORT_DYLIB_PATH: onnxruntime 동적 라이브러리 경로(ort load-dynamic)
    ///   - tests/fixtures/scanned_korean.pdf (합성 픽스처)
    ///   - resources/paddleocr/{det.onnx,rec.onnx,dict.txt}
    #[test]
    fn test_scanned_pdf_rasterize_ocr() {
        let fixture = Path::new("tests/fixtures/scanned_korean.pdf");
        let ocr_dir = Path::new("resources/paddleocr");

        let env_dylib_ok = |key: &str| {
            std::env::var_os(key)
                .map(|p| PathBuf::from(p).exists())
                .unwrap_or(false)
        };
        let pdfium_ok = env_dylib_ok("PDFIUM_DYLIB_PATH");
        let ort_ok = env_dylib_ok("ORT_DYLIB_PATH");
        let models_ok = ocr_dir.join("det.onnx").exists()
            && ocr_dir.join("rec.onnx").exists()
            && ocr_dir.join("dict.txt").exists();

        if !fixture.exists() || !pdfium_ok || !ort_ok || !models_ok {
            eprintln!(
                "Skipping scanned-PDF OCR test (fixture={}, pdfium={}, ort={}, models={})",
                fixture.exists(),
                pdfium_ok,
                ort_ok,
                models_ok
            );
            return;
        }

        let ocr = docufinder_lib::ocr::OcrEngine::new(ocr_dir).expect("OCR engine init failed");

        let doc = docufinder_lib::parsers::pdf::parse(fixture, Some(&ocr))
            .expect("scanned PDF parse failed");

        eprintln!(
            "--- Recovered OCR text ---\n{}\n--------------------------",
            doc.content
        );

        assert!(
            !doc.content.trim().is_empty(),
            "OCR recovered no text from the scanned image PDF"
        );
        // 픽스처에 렌더한 한국어 토큰 중 하나 이상이 복원돼야 함 (OCR 오차 허용)
        let expected = ["문서", "검색", "스캔", "한글", "인식", "복원"];
        assert!(
            expected.iter().any(|w| doc.content.contains(w)),
            "OCR output contained no expected Korean token: {:?}",
            doc.content
        );
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
