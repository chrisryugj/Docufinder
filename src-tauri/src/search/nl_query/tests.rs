    use super::*;

    // === 기본 동작 ===

    #[test]
    fn test_simple_keywords() {
        let result = NlQueryParser::parse("예산 보고서");
        assert_eq!(result.keywords, "예산 보고서");
        assert!(result.date_filter.is_none());
        assert!(result.file_type.is_none());
        assert!(result.exclude_keywords.is_empty());
    }

    #[test]
    fn test_empty_query() {
        let result = NlQueryParser::parse("");
        assert_eq!(result.keywords, "");
        assert!(result.parse_log.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let result = NlQueryParser::parse("   ");
        assert_eq!(result.keywords, "");
    }

    #[test]
    fn test_no_parsing_needed() {
        // NL 패턴이 없는 일반 쿼리 → 그대로 통과
        let result = NlQueryParser::parse("고용보험료 부과");
        assert_eq!(result.keywords, "고용보험료 부과");
        assert!(result.date_filter.is_none());
        assert!(result.file_type.is_none());
        assert!(result.exclude_keywords.is_empty());
    }

    // === Intent 제거 ===

    #[test]
    fn test_intent_removal_find() {
        let result = NlQueryParser::parse("예산 보고서 찾아줘");
        assert_eq!(result.keywords, "예산 보고서");
    }

    #[test]
    fn test_intent_removal_search() {
        let result = NlQueryParser::parse("계약서 검색해줘");
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_intent_removal_show() {
        let result = NlQueryParser::parse("인사 자료 보여줘");
        assert_eq!(result.keywords, "인사 자료");
    }

    #[test]
    fn test_intent_only_returns_empty() {
        let result = NlQueryParser::parse("찾아줘");
        assert_eq!(result.keywords, "");
    }

    #[test]
    fn test_intent_mid_sentence_preserved() {
        // 중간 위치의 "찾아" 등은 키워드로 보존
        let result = NlQueryParser::parse("찾아 놓은 문서");
        assert_eq!(result.keywords, "찾아 놓은 문서");
    }

    // === 날짜 추출 ===

    #[test]
    fn test_date_today() {
        let result = NlQueryParser::parse("오늘 회의록");
        assert_eq!(result.date_filter, Some(DateFilter::Today));
        assert_eq!(result.keywords, "회의록");
    }

    #[test]
    fn test_date_this_week() {
        let result = NlQueryParser::parse("이번주 보고서");
        assert_eq!(result.date_filter, Some(DateFilter::ThisWeek));
        assert_eq!(result.keywords, "보고서");
    }

    #[test]
    fn test_date_last_week() {
        let result = NlQueryParser::parse("지난주 예산");
        assert_eq!(result.date_filter, Some(DateFilter::LastWeek));
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_date_last_week_with_postposition() {
        let result = NlQueryParser::parse("지난주에 작성된 예산");
        assert_eq!(result.date_filter, Some(DateFilter::LastWeek));
        assert_eq!(result.keywords, "작성된 예산");
    }

    #[test]
    fn test_date_this_month() {
        let result = NlQueryParser::parse("이번달 매출");
        assert_eq!(result.date_filter, Some(DateFilter::ThisMonth));
        assert_eq!(result.keywords, "매출");
    }

    #[test]
    fn test_date_this_year() {
        let result = NlQueryParser::parse("올해 인사평가");
        assert_eq!(result.date_filter, Some(DateFilter::ThisYear));
        assert_eq!(result.keywords, "인사평가");
    }

    #[test]
    fn test_date_last_year() {
        let result = NlQueryParser::parse("작년 집행");
        assert_eq!(result.date_filter, Some(DateFilter::LastYear));
        assert_eq!(result.keywords, "집행");
    }

    #[test]
    fn test_date_last_year_variants() {
        for query in &["지난해 예산", "전년 실적", "전년도 결산", "작년도 보고서"]
        {
            let result = NlQueryParser::parse(query);
            assert_eq!(
                result.date_filter,
                Some(DateFilter::LastYear),
                "failed: {}",
                query
            );
        }
    }

    #[test]
    fn test_date_year_4digit() {
        let result = NlQueryParser::parse("2024년 예산");
        assert_eq!(result.date_filter, Some(DateFilter::Year(2024)));
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_date_year_2digit() {
        let result = NlQueryParser::parse("24년 보고서");
        assert_eq!(result.date_filter, Some(DateFilter::Year(2024)));
        assert_eq!(result.keywords, "보고서");
    }

    #[test]
    fn test_date_recent_days() {
        let result = NlQueryParser::parse("최근 30일 계약서");
        assert_eq!(result.date_filter, Some(DateFilter::RecentDays(30)));
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_date_recent_days_no_space() {
        let result = NlQueryParser::parse("최근30일 문서");
        assert_eq!(result.date_filter, Some(DateFilter::RecentDays(30)));
        // "문서"는 파일타입으로 매칭되지 않음 (단독)
        assert!(result.keywords.contains("문서") || result.keywords.is_empty());
    }

    #[test]
    fn test_date_month_number() {
        // "3월" → 올해 3월 필터
        let result = NlQueryParser::parse("3월 보고서");
        assert_eq!(result.date_filter, Some(DateFilter::Month(3)));
        assert_eq!(result.keywords, "보고서");
    }

    // === 파일타입 추출 ===

    #[test]
    fn test_filetype_hwp() {
        let result = NlQueryParser::parse("한글 문서 예산");
        assert_eq!(result.file_type, Some("hwpx".to_string()));
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_filetype_hwp_compact() {
        let result = NlQueryParser::parse("한글문서 예산");
        assert_eq!(result.file_type, Some("hwpx".to_string()));
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_filetype_pdf() {
        let result = NlQueryParser::parse("pdf 계약서");
        assert_eq!(result.file_type, Some("pdf".to_string()));
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_filetype_word() {
        let result = NlQueryParser::parse("워드 보고서");
        assert_eq!(result.file_type, Some("docx".to_string()));
        assert_eq!(result.keywords, "보고서");
    }

    #[test]
    fn test_filetype_excel() {
        let result = NlQueryParser::parse("엑셀 파일 매출");
        assert_eq!(result.file_type, Some("xlsx".to_string()));
        assert_eq!(result.keywords, "매출");
    }

    #[test]
    fn test_filetype_standalone_document_preserved() {
        // "문서"만 단독 출현 → 제거하지 않음
        let result = NlQueryParser::parse("문서 관리");
        assert!(result.file_type.is_none());
        assert_eq!(result.keywords, "문서 관리");
    }

    // === 부정어 추출 ===

    #[test]
    fn test_exclude_bbego() {
        let result = NlQueryParser::parse("계약서 부동산 빼고");
        assert_eq!(result.exclude_keywords, vec!["부동산"]);
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_exclude_aineen() {
        let result = NlQueryParser::parse("부동산 아닌 계약서");
        assert_eq!(result.exclude_keywords, vec!["부동산"]);
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_exclude_jewae() {
        let result = NlQueryParser::parse("세금 제외 보고서");
        assert_eq!(result.exclude_keywords, vec!["세금"]);
        assert_eq!(result.keywords, "보고서");
    }

    #[test]
    fn test_exclude_multiple() {
        let result = NlQueryParser::parse("부동산 빼고 세금 제외 계약서");
        assert!(result.exclude_keywords.contains(&"부동산".to_string()));
        assert!(result.exclude_keywords.contains(&"세금".to_string()));
        assert_eq!(result.keywords, "계약서");
    }

    // === 복합 쿼리 ===

    #[test]
    fn test_complex_all_features() {
        let result = NlQueryParser::parse("지난주 예산 한글 문서 부동산 빼고 찾아줘");
        assert_eq!(result.date_filter, Some(DateFilter::LastWeek));
        assert_eq!(result.file_type, Some("hwpx".to_string()));
        assert_eq!(result.exclude_keywords, vec!["부동산"]);
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_complex_date_and_filetype() {
        let result = NlQueryParser::parse("2024년 인사팀 워드 문서");
        assert_eq!(result.date_filter, Some(DateFilter::Year(2024)));
        assert_eq!(result.file_type, Some("docx".to_string()));
        assert_eq!(result.keywords, "인사팀");
    }

    #[test]
    fn test_complex_date_and_intent() {
        let result = NlQueryParser::parse("이번달 매출 보고서 보여줘");
        assert_eq!(result.date_filter, Some(DateFilter::ThisMonth));
        assert_eq!(result.keywords, "매출 보고서");
    }

    // === parse_log ===

    #[test]
    fn test_parse_log_content() {
        let result = NlQueryParser::parse("지난주 예산 한글 문서 부동산 빼고 찾아줘");
        // parse_log에 검색어, 날짜, 파일, 제외 포함
        assert!(result.parse_log.iter().any(|l| l.contains("검색어")));
        assert!(result.parse_log.iter().any(|l| l.contains("날짜")));
        assert!(result.parse_log.iter().any(|l| l.contains("파일")));
        assert!(result.parse_log.iter().any(|l| l.contains("제외")));
    }

    #[test]
    fn test_parse_log_empty_for_simple_query() {
        // 패턴 없는 단순 쿼리 → 검색어 로그만
        let result = NlQueryParser::parse("고용보험료");
        assert_eq!(result.parse_log.len(), 1);
        assert!(result.parse_log[0].contains("검색어"));
    }

    // === 엣지 케이스 ===

    #[test]
    fn test_only_filters_empty_keywords() {
        // 필터만 있고 키워드 없음
        let result = NlQueryParser::parse("지난주 한글 문서 찾아줘");
        assert_eq!(result.date_filter, Some(DateFilter::LastWeek));
        assert_eq!(result.file_type, Some("hwpx".to_string()));
        // 키워드가 비어있을 수 있음
        assert!(result.keywords.is_empty());
    }

    #[test]
    fn test_original_query_preserved() {
        let input = "지난주 예산 찾아줘";
        let result = NlQueryParser::parse(input);
        assert_eq!(result.original_query, input);
    }

    // === UX 개선 테스트 ===

    #[test]
    fn test_filetype_with_ro_postposition() {
        // "한글로 된 예산서" → 파일타입 hwpx, 키워드 "예산서"
        let result = NlQueryParser::parse("한글로 된 예산서");
        assert_eq!(result.file_type, Some("hwpx".to_string()));
        assert_eq!(result.keywords, "예산서");
    }

    #[test]
    fn test_filetype_pdf_ro() {
        let result = NlQueryParser::parse("pdf로 된 계약서");
        assert_eq!(result.file_type, Some("pdf".to_string()));
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_filler_removal() {
        // "엑셀 파일 중에서 예산" → 파일타입 xlsx, 키워드 "예산" (중에서 제거)
        let result = NlQueryParser::parse("엑셀 파일 중에서 예산");
        assert_eq!(result.file_type, Some("xlsx".to_string()));
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_intent_question_mark() {
        let result = NlQueryParser::parse("예산 보고서 있을까?");
        assert_eq!(result.keywords, "예산 보고서");
    }

    #[test]
    fn test_month_filter() {
        let result = NlQueryParser::parse("11월 결산 보고서");
        assert_eq!(result.date_filter, Some(DateFilter::Month(11)));
        assert_eq!(result.keywords, "결산 보고서");
    }

    #[test]
    fn test_compound_word_not_parsed() {
        // "결재문서"에서 "문서"가 파일타입으로 잡히면 안 됨
        let result = NlQueryParser::parse("결재문서");
        assert!(result.file_type.is_none());
        assert_eq!(result.keywords, "결재문서");
    }

    // === 플레이스홀더 예시 검증 ===

    #[test]
    fn test_placeholder_natural_1() {
        // "작년 예산 한글 문서"
        let result = NlQueryParser::parse("작년 예산 한글 문서");
        assert_eq!(result.date_filter, Some(DateFilter::LastYear));
        assert_eq!(result.file_type, Some("hwpx".to_string()));
        assert_eq!(result.keywords, "예산");
    }

    #[test]
    fn test_placeholder_natural_2() {
        // "최근 30일 계약서 PDF만"
        let result = NlQueryParser::parse("최근 30일 계약서 PDF만");
        assert_eq!(result.date_filter, Some(DateFilter::RecentDays(30)));
        assert_eq!(result.file_type, Some("pdf".to_string()));
        assert_eq!(result.keywords, "계약서");
    }

    #[test]
    fn test_budget_bill_not_negated() {
        // "예산안"에서 "안"이 부정어로 잡히면 안 됨
        let result = NlQueryParser::parse("예산안");
        assert!(result.exclude_keywords.is_empty());
        assert_eq!(result.keywords, "예산안");
    }

    // === parse_with_tokenizer 통합 테스트 ===

    #[test]
    fn test_tokenizer_rag_queries() {
        use crate::tokenizer::LinderaKoTokenizer;
        let tok = LinderaKoTokenizer::new().unwrap();

        struct Case {
            input: &'static str,
            must_have: &'static [&'static str],
            must_not_have: &'static [&'static str],
        }

        let cases = [
            Case {
                input: "2026년 노인일자리 참여자가 몇명이야",
                must_have: &["노인", "일자리", "참여"],
                must_not_have: &["몇명이야", "이야"],
            },
            Case {
                input: "예산 집행률은 얼마인가요",
                must_have: &["예산", "집행"],
                must_not_have: &["얼마", "인가요"],
            },
            Case {
                input: "작년 hwpx 문서",
                must_have: &[],
                must_not_have: &["문서"],
            },
            Case {
                input: "공무원 복지포인트 사용 기준을 알려줘",
                must_have: &["공무원", "복지", "사용", "기준"],
                must_not_have: &["알려줘"],
            },
            Case {
                input: "올해 사업계획서 어디있어",
                must_have: &["사업", "계획"],
                must_not_have: &["어디있어", "어디"],
            },
        ];

        for (i, case) in cases.iter().enumerate() {
            let result = NlQueryParser::parse_with_tokenizer(case.input, &tok);
            let keywords = &result.keywords;
            println!(
                "[{}] '{}' -> keywords='{}', date={:?}, file_type={:?}",
                i, case.input, keywords, result.date_filter, result.file_type
            );

            for must in case.must_have {
                assert!(
                    keywords.contains(must),
                    "[{}] '{}': 키워드에 '{}' 포함되어야 함 (got: '{}')",
                    i,
                    case.input,
                    must,
                    keywords
                );
            }
            for must_not in case.must_not_have {
                assert!(
                    !keywords.contains(must_not),
                    "[{}] '{}': 키워드에 '{}' 포함되면 안 됨 (got: '{}')",
                    i,
                    case.input,
                    must_not,
                    keywords
                );
            }
        }
    }

    /// 실제 DB 대상 FTS 검색 테스트 (DB가 없으면 자동 스킵)
    #[test]
    fn test_real_db_noun_fts_search() {
        use crate::tokenizer::{LinderaKoTokenizer, TextTokenizer};

        let appdata = match std::env::var("APPDATA") {
            Ok(v) => v,
            Err(_) => {
                println!("SKIP: APPDATA not set");
                return;
            }
        };
        let db_path = std::path::PathBuf::from(appdata)
            .join("com.anything.app")
            .join("docufinder.db");
        if !db_path.exists() {
            println!("SKIP: DB not found at {:?}", db_path);
            return;
        }

        let tok = LinderaKoTokenizer::new().unwrap();
        let conn = rusqlite::Connection::open(&db_path).expect("DB open failed");

        let queries = [
            "2026년 노인일자리 참여자가 몇명이야",
            "예산 집행률은 얼마인가요",
            "공무원 복지포인트 사용 기준을 알려줘",
            "올해 사업계획서 어디있어",
            "보조금 지급 현황 보여줘",
            "인사이동 내역이 궁금해",
            "계약서 검토 결과",
        ];

        println!("\n========== 실제 DB FTS 검색 테스트 ==========\n");

        for query in &queries {
            let parsed = NlQueryParser::parse_with_tokenizer(query, &tok);
            print!("Q: '{}' -> kw='{}' ", query, parsed.keywords);

            if parsed.keywords.is_empty() {
                println!("(키워드 없음)");
                continue;
            }

            let fts_query = tok.tokenize_query(&parsed.keywords);

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH ?",
                    [&fts_query],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            println!("-> {}건", count);

            // 상위 3개
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.name, snippet(chunks_fts, 0, '>>', '<<', '...', 20)
                 FROM chunks_fts fts
                 JOIN chunks c ON c.id = fts.rowid
                 JOIN files f ON f.id = c.file_id
                 WHERE chunks_fts MATCH ?
                 ORDER BY bm25(chunks_fts)
                 LIMIT 3",
            ) {
                if let Ok(rows) = stmt.query_map([&fts_query], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }) {
                    for row in rows.flatten() {
                        let snip = if row.1.chars().count() > 60 {
                            format!("{}...", row.1.chars().take(60).collect::<String>())
                        } else {
                            row.1
                        };
                        println!("   - {} | {}", row.0, snip);
                    }
                }
            }
            println!();

            if count == 0 {
                println!("   ⚠ 0건 (DB에 관련 문서 없을 수 있음)");
            }
        }
    }

    // === 파일명 필터 추출 (50개 종합 테스트) ===

    /// 파일명 필터 + 기존 기능 50개 종합 테스트
    #[test]
    fn test_comprehensive_smart_queries() {
        struct Q {
            input: &'static str,
            keywords: &'static str,
            file_type: Option<&'static str>,
            date: Option<&'static str>,     // 간략 표기
            filename: Option<&'static str>, // filename_filter
            exclude: &'static [&'static str],
        }

        let cases: Vec<Q> = vec![
            // ── A. 파일명 필터 (should trigger) ──
            Q {
                input: "제목이 2025인 pdf",
                keywords: "",
                file_type: Some("pdf"),
                date: None,
                filename: Some("2025"),
                exclude: &[],
            },
            Q {
                input: "제목이 예산인 한글 문서",
                keywords: "",
                file_type: Some("hwpx"),
                date: None,
                filename: Some("예산"),
                exclude: &[],
            },
            Q {
                input: "이름이 계약서인 파일 찾아줘",
                keywords: "파일",
                file_type: None,
                date: None,
                filename: Some("계약서"),
                exclude: &[],
            },
            Q {
                input: "파일명이 보고서인 문서",
                keywords: "문서",
                file_type: None,
                date: None,
                filename: Some("보고서"),
                exclude: &[],
            },
            Q {
                input: "제목에 인사 포함된 엑셀",
                keywords: "",
                file_type: Some("xlsx"),
                date: None,
                filename: Some("인사"),
                exclude: &[],
            },
            Q {
                input: "이름에 회의록 들어간 한글파일",
                keywords: "",
                file_type: Some("hwpx"),
                date: None,
                filename: Some("회의록"),
                exclude: &[],
            },
            Q {
                input: "파일명에 2024 포함 pdf",
                keywords: "",
                file_type: Some("pdf"),
                date: None,
                filename: Some("2024"),
                exclude: &[],
            },
            Q {
                input: "제목이 결산인 문서 보여줘",
                keywords: "문서",
                file_type: None,
                date: None,
                filename: Some("결산"),
                exclude: &[],
            },
            Q {
                input: "제목에 출장 포함된 파일",
                keywords: "파일",
                file_type: None,
                date: None,
                filename: Some("출장"),
                exclude: &[],
            },
            Q {
                input: "이름이 매출인 엑셀파일",
                keywords: "",
                file_type: Some("xlsx"),
                date: None,
                filename: Some("매출"),
                exclude: &[],
            },
            // ── B. 파일명 필터가 트리거되면 안 되는 케이스 ──
            Q {
                input: "제목 작성법",
                keywords: "제목 작성법",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "파일명 검색",
                keywords: "파일명 검색",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "이름표 만들기",
                keywords: "이름표 만들기",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "제목에 관한 보고서",
                keywords: "제목에 관한 보고서",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "이름 변경 방법",
                keywords: "이름 변경 방법",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            // ── C. 날짜 + 파일타입 (기존 동작 검증) ──
            Q {
                input: "2025년 pdf",
                keywords: "",
                file_type: Some("pdf"),
                date: Some("Year"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "올해 한글 문서",
                keywords: "",
                file_type: Some("hwpx"),
                date: Some("ThisYear"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "지난주 워드 파일",
                keywords: "",
                file_type: Some("docx"),
                date: Some("LastWeek"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "작년 엑셀 보고서",
                keywords: "보고서",
                file_type: Some("xlsx"),
                date: Some("LastYear"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "이번달 pdf 매출",
                keywords: "매출",
                file_type: Some("pdf"),
                date: Some("ThisMonth"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "최근 7일 계약서",
                keywords: "계약서",
                file_type: None,
                date: Some("RecentDays"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "3월 보고서",
                keywords: "보고서",
                file_type: None,
                date: Some("Month"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "24년 인사 자료",
                keywords: "인사 자료",
                file_type: None,
                date: Some("Year"),
                filename: None,
                exclude: &[],
            },
            // ── D. 부정어 ──
            Q {
                input: "계약서 부동산 빼고",
                keywords: "계약서",
                file_type: None,
                date: None,
                filename: None,
                exclude: &["부동산"],
            },
            Q {
                input: "세금 제외 올해 보고서",
                keywords: "보고서",
                file_type: None,
                date: Some("ThisYear"),
                filename: None,
                exclude: &["세금"],
            },
            Q {
                input: "예산 부동산 빼고 세금 제외",
                keywords: "예산",
                file_type: None,
                date: None,
                filename: None,
                exclude: &["부동산", "세금"],
            },
            // ── E. Intent 제거 ──
            Q {
                input: "인사팀 자료 찾아줘",
                keywords: "인사팀 자료",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "회의록 보여줘",
                keywords: "회의록",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "예산 보고서 있을까?",
                keywords: "예산 보고서",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "급여 명세서 검색해줘",
                keywords: "급여 명세서",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            // ── F. 복합 쿼리 (파일명 + 날짜 + 파일타입) ──
            Q {
                input: "제목이 예산인 올해 pdf",
                keywords: "",
                file_type: Some("pdf"),
                date: Some("ThisYear"),
                filename: Some("예산"),
                exclude: &[],
            },
            Q {
                input: "이름에 계약 포함된 작년 한글 문서",
                keywords: "",
                file_type: Some("hwpx"),
                date: Some("LastYear"),
                filename: Some("계약"),
                exclude: &[],
            },
            Q {
                input: "제목이 회의록인 이번주 파일 찾아줘",
                keywords: "파일",
                file_type: None,
                date: Some("ThisWeek"),
                filename: Some("회의록"),
                exclude: &[],
            },
            // ── G. 다양한 파일타입 ──
            Q {
                input: "텍스트 파일 로그",
                keywords: "로그",
                file_type: Some("txt"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "pptx 발표 자료",
                keywords: "발표 자료",
                file_type: Some("pptx"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "파워포인트 프레젠테이션",
                keywords: "프레젠테이션",
                file_type: Some("pptx"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "피디에프 스캔 문서",
                keywords: "스캔 문서",
                file_type: Some("pdf"),
                date: None,
                filename: None,
                exclude: &[],
            },
            // ── H. 단순 키워드 (필터 없음) ──
            Q {
                input: "고용보험료 부과",
                keywords: "고용보험료 부과",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "결재문서",
                keywords: "결재문서",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "인사발령 통보",
                keywords: "인사발령 통보",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "예산안",
                keywords: "예산안",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            // ── I. 엣지 케이스 ──
            Q {
                input: "찾아줘",
                keywords: "",
                file_type: None,
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "pdf만 보여줘",
                keywords: "",
                file_type: Some("pdf"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "올해 한글 문서 세금 빼고 찾아줘",
                keywords: "",
                file_type: Some("hwpx"),
                date: Some("ThisYear"),
                filename: None,
                exclude: &["세금"],
            },
            Q {
                input: "제목이 2025인 올해 pdf 부동산 빼고 찾아줘",
                keywords: "",
                file_type: Some("pdf"),
                date: Some("ThisYear"),
                filename: Some("2025"),
                exclude: &["부동산"],
            },
            // ── J. 파일타입 변형 ──
            Q {
                input: "hwp 예산",
                keywords: "예산",
                file_type: Some("hwpx"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "한글로 된 계약서",
                keywords: "계약서",
                file_type: Some("hwpx"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "엑셀 파일 중에서 매출",
                keywords: "매출",
                file_type: Some("xlsx"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "pdf로 된 계약서",
                keywords: "계약서",
                file_type: Some("pdf"),
                date: None,
                filename: None,
                exclude: &[],
            },
            Q {
                input: "최근 30일 계약서 PDF만",
                keywords: "계약서",
                file_type: Some("pdf"),
                date: Some("RecentDays"),
                filename: None,
                exclude: &[],
            },
            Q {
                input: "2024년 인사팀 워드 문서",
                keywords: "인사팀",
                file_type: Some("docx"),
                date: Some("Year"),
                filename: None,
                exclude: &[],
            },
        ];

        for (i, q) in cases.iter().enumerate() {
            let result = NlQueryParser::parse(q.input);
            println!(
                "[{:02}] '{}' -> kw='{}', ft={:?}, date={:?}, fn={:?}, ex={:?}",
                i,
                q.input,
                result.keywords,
                result.file_type,
                result.date_filter,
                result.filename_filter,
                result.exclude_keywords
            );

            // keywords
            assert_eq!(
                result.keywords, q.keywords,
                "[{}] '{}': keywords expected='{}', got='{}'",
                i, q.input, q.keywords, result.keywords
            );

            // file_type
            assert_eq!(
                result.file_type.as_deref(),
                q.file_type,
                "[{}] '{}': file_type expected={:?}, got={:?}",
                i,
                q.input,
                q.file_type,
                result.file_type
            );

            // date_filter (간략 비교: type 이름만)
            match q.date {
                Some(expected_type) => {
                    assert!(
                        result.date_filter.is_some(),
                        "[{}] '{}': date expected Some({}), got None",
                        i,
                        q.input,
                        expected_type
                    );
                    let actual_type = match &result.date_filter {
                        Some(DateFilter::Today) => "Today",
                        Some(DateFilter::ThisWeek) => "ThisWeek",
                        Some(DateFilter::LastWeek) => "LastWeek",
                        Some(DateFilter::ThisMonth) => "ThisMonth",
                        Some(DateFilter::LastMonth) => "LastMonth",
                        Some(DateFilter::ThisYear) => "ThisYear",
                        Some(DateFilter::LastYear) => "LastYear",
                        Some(DateFilter::Year(_)) => "Year",
                        Some(DateFilter::Month(_)) => "Month",
                        Some(DateFilter::RecentDays(_)) => "RecentDays",
                        None => "None",
                    };
                    assert_eq!(
                        actual_type, expected_type,
                        "[{}] '{}': date type expected='{}', got='{}'",
                        i, q.input, expected_type, actual_type
                    );
                }
                None => {
                    assert!(
                        result.date_filter.is_none(),
                        "[{}] '{}': date expected None, got {:?}",
                        i,
                        q.input,
                        result.date_filter
                    );
                }
            }

            // filename_filter
            assert_eq!(
                result.filename_filter.as_deref(),
                q.filename,
                "[{}] '{}': filename expected={:?}, got={:?}",
                i,
                q.input,
                q.filename,
                result.filename_filter
            );

            // exclude_keywords
            let expected_exclude: Vec<String> = q.exclude.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                result.exclude_keywords, expected_exclude,
                "[{}] '{}': exclude expected={:?}, got={:?}",
                i, q.input, q.exclude, result.exclude_keywords
            );
        }
    }
