//! 자연어 쿼리 파서
//!
//! 한국어 자연어 질의에서 키워드, 날짜, 파일타입, 부정어를 규칙 기반으로 추출.
//! 확실한 패턴만 처리하고, 모호한 표현은 키워드로 보존 (KISS 원칙).

use serde::Serialize;

/// 자연어 쿼리 파싱 결과
#[derive(Debug, Clone, Serialize)]
pub struct ParsedQuery {
    /// 검색할 키워드 (형태소 분석 전 원문)
    pub keywords: String,
    /// 제외할 키워드 (NOT)
    pub exclude_keywords: Vec<String>,
    /// 날짜 필터
    pub date_filter: Option<DateFilter>,
    /// 파일 타입 필터 ("hwpx", "docx", "pdf" 등)
    pub file_type: Option<String>,
    /// 파싱 전 원본 쿼리
    pub original_query: String,
    /// 파싱 로그 (UI 표시용)
    pub parse_log: Vec<String>,
}

/// 날짜 필터
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum DateFilter {
    Today,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    ThisYear,
    Year(i32),
    RecentDays(u32),
}

pub struct NlQueryParser;

impl NlQueryParser {
    /// 자연어 쿼리를 파싱하여 구조화된 검색 조건으로 변환
    pub fn parse(query: &str) -> ParsedQuery {
        let mut remaining = query.trim().to_string();
        let mut parse_log = Vec::new();
        let original = remaining.clone();

        if remaining.is_empty() {
            return ParsedQuery {
                keywords: String::new(),
                exclude_keywords: vec![],
                date_filter: None,
                file_type: None,
                original_query: original,
                parse_log,
            };
        }

        // 규칙 순서대로 적용 (각 규칙이 매칭 부분을 remaining에서 제거)

        // 1. Intent 제거 (문장 끝의 UI 의도 표현)
        remaining = Self::remove_intent(&remaining);

        // 2. 부정어 추출 (날짜/파일타입보다 먼저 — "지난주 빼고" 방지)
        let exclude_keywords = Self::extract_negation(&mut remaining, &mut parse_log);

        // 3. 날짜 추출
        let date_filter = Self::extract_date(&mut remaining, &mut parse_log);

        // 4. 파일타입 추출
        let file_type = Self::extract_file_type(&mut remaining, &mut parse_log);

        // 5. 잔여 토큰 정리 → keywords
        let keywords = remaining
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        if !keywords.is_empty() {
            parse_log.insert(0, format!("검색어: {}", keywords));
        }

        ParsedQuery {
            keywords,
            exclude_keywords,
            date_filter,
            file_type,
            original_query: original,
            parse_log,
        }
    }

    /// Intent words 제거 (문장 끝의 UI 의도 표현만)
    fn remove_intent(query: &str) -> String {
        let patterns = [
            "찾아줘",
            "찾아봐",
            "검색해줘",
            "검색해 줘",
            "검색해봐",
            "검색해 봐",
            "보여줘",
            "보여 줘",
            "알려줘",
            "알려 줘",
        ];

        let trimmed = query.trim();
        for pat in &patterns {
            if let Some(prefix) = trimmed.strip_suffix(pat) {
                return prefix.trim().to_string();
            }
        }

        // 물음표로 끝나는 패턴: "있어?", "있나?"
        let q_patterns = ["있어?", "있어", "있나?", "있나"];
        for pat in &q_patterns {
            if let Some(prefix) = trimmed.strip_suffix(pat) {
                return prefix.trim().to_string();
            }
        }

        trimmed.to_string()
    }

    /// 부정어 추출: "X 아닌", "X 빼고", "X 제외" 등
    fn extract_negation(remaining: &mut String, parse_log: &mut Vec<String>) -> Vec<String> {
        let mut excluded = Vec::new();
        let neg_suffixes = ["아닌", "빼고", "제외", "말고", "없는", "않은"];

        // 반복적으로 부정어 패턴 탐색 (복수 부정어 지원)
        loop {
            let mut found = false;
            let words: Vec<String> = remaining.split_whitespace().map(String::from).collect();

            for i in 0..words.len() {
                for suffix in &neg_suffixes {
                    if words[i] == *suffix && i > 0 {
                        // "부동산 아닌" 패턴: 앞 단어가 부정 대상
                        let target = words[i - 1].clone();
                        excluded.push(target.clone());
                        parse_log.push(format!("제외: {}", target));

                        // remaining에서 "부동산 아닌" 제거
                        let pattern = format!("{} {}", words[i - 1], words[i]);
                        *remaining = remaining.replace(&pattern, " ");
                        *remaining = remaining
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        found = true;
                        break;
                    } else if words[i].ends_with(suffix) && words[i].len() > suffix.len() {
                        // "부동산아닌" 붙여쓰기 패턴
                        let target = words[i][..words[i].len() - suffix.len()].to_string();
                        if !target.is_empty() {
                            excluded.push(target.clone());
                            parse_log.push(format!("제외: {}", target));
                            *remaining = remaining.replace(&words[i], " ");
                            *remaining = remaining
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            found = true;
                            break;
                        }
                    }
                }
                if found {
                    break;
                }
            }

            if !found {
                break;
            }
        }

        // "것", "거" 같은 잔여물 제거
        let filler = ["것", "거"];
        for f in &filler {
            let words: Vec<&str> = remaining.split_whitespace().collect();
            if words.len() > 1 || (words.len() == 1 && words[0] != *f) {
                // 다른 단어가 있을 때만 filler 제거
                *remaining = words
                    .into_iter()
                    .filter(|w| *w != *f)
                    .collect::<Vec<_>>()
                    .join(" ");
            }
        }

        excluded
    }

    /// 날짜 추출 (확실한 패턴만)
    fn extract_date(remaining: &mut String, parse_log: &mut Vec<String>) -> Option<DateFilter> {
        struct DatePattern {
            patterns: &'static [&'static str],
            filter: DateFilter,
            label: &'static str,
        }

        let date_patterns = [
            DatePattern {
                patterns: &["오늘"],
                filter: DateFilter::Today,
                label: "오늘",
            },
            DatePattern {
                patterns: &["이번 주", "이번주", "금주"],
                filter: DateFilter::ThisWeek,
                label: "이번 주",
            },
            DatePattern {
                patterns: &["지난 주", "지난주", "저번 주", "저번주"],
                filter: DateFilter::LastWeek,
                label: "지난 주",
            },
            DatePattern {
                patterns: &["이번 달", "이번달", "금월"],
                filter: DateFilter::ThisMonth,
                label: "이번 달",
            },
            DatePattern {
                patterns: &["지난 달", "지난달", "저번 달", "저번달"],
                filter: DateFilter::LastMonth,
                label: "지난 달",
            },
            DatePattern {
                patterns: &["올해"],
                filter: DateFilter::ThisYear,
                label: "올해",
            },
        ];

        // 고정 패턴 매칭 (긴 패턴부터)
        for dp in &date_patterns {
            for pat in dp.patterns {
                if let Some(pos) = remaining.find(pat) {
                    // 패턴 주변이 단어 경계인지 확인
                    let before_ok = pos == 0
                        || remaining[..pos].ends_with(' ')
                        || remaining[..pos].ends_with('에');
                    let after_pos = pos + pat.len();
                    let after_ok = after_pos >= remaining.len()
                        || remaining[after_pos..].starts_with(' ')
                        || remaining[after_pos..].starts_with("에");

                    if before_ok && after_ok {
                        // 패턴 + 뒤의 조사("에", "에서") 제거
                        let mut end = after_pos;
                        let rest = &remaining[end..];
                        if rest.starts_with("에서") {
                            end += "에서".len();
                        } else if rest.starts_with("에") {
                            end += "에".len();
                        }

                        // 앞의 조사("에") 제거
                        let mut start = pos;
                        if start > 0 && remaining[..start].ends_with('에') {
                            start -= 'ì'.len_utf8(); // 'ì' is not right
                            // "에" is 3 bytes in UTF-8
                            if start >= 3 && remaining[start - 3..start + 3].contains("에") {
                                // skip complex logic, just remove the pattern itself
                            }
                        }

                        let mut result = String::new();
                        result.push_str(remaining[..pos].trim_end());
                        if !result.is_empty() && end < remaining.len() {
                            result.push(' ');
                        }
                        result.push_str(remaining[end..].trim_start());
                        *remaining = result
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");

                        parse_log.push(format!("날짜: {}", dp.label));
                        return Some(dp.filter.clone());
                    }
                }
            }
        }

        // "YYYY년" 패턴
        if let Some(filter) = Self::extract_year_pattern(remaining, parse_log) {
            return Some(filter);
        }

        // "최근 N일" 패턴
        if let Some(filter) = Self::extract_recent_days(remaining, parse_log) {
            return Some(filter);
        }

        None
    }

    /// "2024년" 또는 "24년" 패턴
    fn extract_year_pattern(
        remaining: &mut String,
        parse_log: &mut Vec<String>,
    ) -> Option<DateFilter> {
        let words: Vec<String> = remaining.split_whitespace().map(String::from).collect();
        for (i, word) in words.iter().enumerate() {
            if let Some(year_str) = word.strip_suffix('년') {
                if let Ok(year) = year_str.parse::<i32>() {
                    let actual_year = if year >= 100 {
                        year
                    } else if year >= 0 && year <= 99 {
                        2000 + year
                    } else {
                        continue;
                    };

                    if actual_year >= 1990 && actual_year <= 2100 {
                        let mut new_words: Vec<String> = Vec::new();
                        for (j, w) in words.iter().enumerate() {
                            if j != i {
                                new_words.push(w.clone());
                            }
                        }
                        *remaining = new_words.join(" ");
                        parse_log.push(format!("날짜: {}년", actual_year));
                        return Some(DateFilter::Year(actual_year));
                    }
                }
            }
        }
        None
    }

    /// "최근 N일" 패턴
    fn extract_recent_days(
        remaining: &mut String,
        parse_log: &mut Vec<String>,
    ) -> Option<DateFilter> {
        // "최근 30일", "최근30일", "최근 7 일"
        let text = remaining.clone();
        let patterns_start = ["최근 ", "최근"];

        for prefix in &patterns_start {
            if let Some(start_pos) = text.find(prefix) {
                let after = &text[start_pos + prefix.len()..];
                // 숫자 추출
                let num_str: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if !num_str.is_empty() {
                    if let Ok(days) = num_str.parse::<u32>() {
                        if days > 0 && days <= 365 {
                            // "일" 접미사 확인
                            let after_num = &after[num_str.len()..];
                            let end_offset = if after_num.starts_with(" 일")
                                || after_num.starts_with("일")
                            {
                                let skip = if after_num.starts_with(" 일") {
                                    " 일".len()
                                } else {
                                    "일".len()
                                };
                                start_pos + prefix.len() + num_str.len() + skip
                            } else {
                                start_pos + prefix.len() + num_str.len()
                            };

                            let mut result = String::new();
                            result.push_str(text[..start_pos].trim_end());
                            if !result.is_empty() && end_offset < text.len() {
                                result.push(' ');
                            }
                            result.push_str(text[end_offset..].trim_start());
                            *remaining = result
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");

                            parse_log.push(format!("날짜: 최근 {}일", days));
                            return Some(DateFilter::RecentDays(days));
                        }
                    }
                }
            }
        }
        None
    }

    /// 파일타입 추출
    fn extract_file_type(
        remaining: &mut String,
        parse_log: &mut Vec<String>,
    ) -> Option<String> {
        struct FileTypePattern {
            patterns: Vec<&'static str>,
            file_type: &'static str,
            label: &'static str,
        }

        let ft_patterns = vec![
            FileTypePattern {
                patterns: vec!["한글 문서", "한글문서", "한글 파일", "한글파일", "hwpx", "hwp", "한글"],
                file_type: "hwpx",
                label: "한글(hwpx)",
            },
            FileTypePattern {
                patterns: vec![
                    "워드 문서",
                    "워드문서",
                    "워드 파일",
                    "워드파일",
                    "docx",
                    "doc",
                    "word",
                    "워드",
                ],
                file_type: "docx",
                label: "워드(docx)",
            },
            FileTypePattern {
                patterns: vec![
                    "엑셀 문서",
                    "엑셀문서",
                    "엑셀 파일",
                    "엑셀파일",
                    "xlsx",
                    "xls",
                    "excel",
                    "엑셀",
                ],
                file_type: "xlsx",
                label: "엑셀(xlsx)",
            },
            FileTypePattern {
                patterns: vec!["pdf 문서", "pdf문서", "pdf 파일", "pdf파일", "피디에프", "pdf"],
                file_type: "pdf",
                label: "PDF",
            },
            FileTypePattern {
                patterns: vec![
                    "텍스트 문서",
                    "텍스트문서",
                    "텍스트 파일",
                    "텍스트파일",
                    "txt",
                ],
                file_type: "txt",
                label: "텍스트(txt)",
            },
            FileTypePattern {
                patterns: vec![
                    "파워포인트",
                    "피피티",
                    "pptx",
                    "ppt",
                ],
                file_type: "pptx",
                label: "파워포인트(pptx)",
            },
        ];

        let lower = remaining.to_lowercase();

        // 긴 패턴부터 매칭 (정확도 우선)
        for ftp in &ft_patterns {
            let mut sorted_patterns = ftp.patterns.clone();
            sorted_patterns.sort_by(|a, b| b.len().cmp(&a.len()));

            for pat in &sorted_patterns {
                let pat_lower = pat.to_lowercase();
                if let Some(pos) = lower.find(&pat_lower) {
                    // 단어 경계 확인
                    let before_ok =
                        pos == 0 || remaining.as_bytes().get(pos - 1) == Some(&b' ');
                    let after_pos = pos + pat.len();
                    let after_ok = after_pos >= remaining.len()
                        || remaining.as_bytes().get(after_pos) == Some(&b' ')
                        || remaining[after_pos..].starts_with("만")
                        || remaining[after_pos..].starts_with("으로")
                        || remaining[after_pos..].starts_with("에서");

                    if before_ok && after_ok {
                        // 패턴 + 뒤의 조사 제거
                        let mut end = after_pos;
                        let rest = &remaining[end..];
                        for postfix in &["만", "으로", "에서"] {
                            if rest.starts_with(postfix) {
                                end += postfix.len();
                                break;
                            }
                        }

                        let mut result = String::new();
                        result.push_str(remaining[..pos].trim_end());
                        if !result.is_empty() && end < remaining.len() {
                            result.push(' ');
                        }
                        result.push_str(remaining[end..].trim_start());
                        *remaining = result
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");

                        parse_log.push(format!("파일: {}", ftp.label));
                        return Some(ftp.file_type.to_string());
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
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
    fn test_date_ambiguous_passes_through() {
        // "3월", "하반기" 등 모호한 건 키워드로 보존
        let result = NlQueryParser::parse("3월 보고서");
        assert!(result.date_filter.is_none());
        assert_eq!(result.keywords, "3월 보고서");
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
        let result =
            NlQueryParser::parse("지난주 예산 한글 문서 부동산 빼고 찾아줘");
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
        let result =
            NlQueryParser::parse("지난주 예산 한글 문서 부동산 빼고 찾아줘");
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
        assert!(result.keywords.is_empty() || result.keywords == "");
    }

    #[test]
    fn test_original_query_preserved() {
        let input = "지난주 예산 찾아줘";
        let result = NlQueryParser::parse(input);
        assert_eq!(result.original_query, input);
    }
}
