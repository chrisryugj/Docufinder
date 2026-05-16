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
    /// 파일명 필터 (파일명에 포함되어야 할 텍스트)
    pub filename_filter: Option<String>,
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
    LastYear,
    Year(i32),
    /// 올해 N월 (1~12)
    Month(u32),
    RecentDays(u32),
}

pub struct NlQueryParser;

impl NlQueryParser {
    /// 자연어 쿼리를 파싱하여 구조화된 검색 조건으로 변환
    pub fn parse(query: &str) -> ParsedQuery {
        Self::parse_inner(query, None)
    }

    /// 토크나이저를 사용한 파싱 (명사 추출 기반 키워드 생성)
    ///
    /// 형태소 분석으로 명사만 추출하여 의문 표현/조사/어미를 자동 제거.
    /// 패턴 블랙리스트 없이도 "참여자가 몇명이야" → "참여자" 추출 가능.
    pub fn parse_with_tokenizer(
        query: &str,
        tokenizer: &dyn crate::tokenizer::TextTokenizer,
    ) -> ParsedQuery {
        Self::parse_inner(query, Some(tokenizer))
    }

    fn parse_inner(
        query: &str,
        tokenizer: Option<&dyn crate::tokenizer::TextTokenizer>,
    ) -> ParsedQuery {
        let mut remaining = query.trim().to_string();
        let mut parse_log = Vec::new();
        let original = remaining.clone();

        if remaining.is_empty() {
            return ParsedQuery {
                keywords: String::new(),
                exclude_keywords: vec![],
                date_filter: None,
                file_type: None,
                filename_filter: None,
                original_query: original,
                parse_log,
            };
        }

        // 규칙 순서대로 적용 (각 규칙이 매칭 부분을 remaining에서 제거)

        // 1. Intent 제거 (문장 끝의 UI 의도 표현) — 토크나이저 유무와 무관하게 적용
        //    "찾아줘", "보여줘" 등 명확한 패턴만 (명사 추출이 이걸 대체하진 않음)
        remaining = Self::remove_intent(&remaining);

        // 2. 부정어 추출 (날짜/파일타입보다 먼저 — "지난주 빼고" 방지)
        let exclude_keywords = Self::extract_negation(&mut remaining, &mut parse_log);

        // 3. 파일명 필터 추출 ("제목이 X인", "이름에 X 포함" 등)
        let filename_filter = Self::extract_filename_filter(&mut remaining, &mut parse_log);

        // 4. 날짜 추출
        let date_filter = Self::extract_date(&mut remaining, &mut parse_log);

        // 5. 파일타입 추출
        let file_type = Self::extract_file_type(&mut remaining, &mut parse_log);

        // 6. 키워드 추출
        let keywords = if let Some(tok) = tokenizer {
            // 형태소 분석 기반: 명사만 추출 → 의문사/조사/어미 자동 제거
            let nouns = tok.extract_nouns(&remaining);
            nouns.join(" ")
        } else {
            // 기존 방식: 필러 단어만 제거
            let filler_words = ["중에서", "중에", "좀"];
            remaining
                .split_whitespace()
                .filter(|w| !filler_words.contains(w))
                .collect::<Vec<_>>()
                .join(" ")
        };

        if !keywords.is_empty() {
            parse_log.insert(0, format!("검색어: {}", keywords));
        }

        ParsedQuery {
            keywords,
            exclude_keywords,
            date_filter,
            file_type,
            filename_filter,
            original_query: original,
            parse_log,
        }
    }

    /// Intent words 제거 (문장 끝의 UI 의도 표현만)
    fn remove_intent(query: &str) -> String {
        let patterns = [
            "찾아줘",
            "찾아봐",
            "찾아 줘",
            "찾아 봐",
            "검색해줘",
            "검색해 줘",
            "검색해봐",
            "검색해 봐",
            "보여줘",
            "보여 줘",
            "알려줘",
            "알려 줘",
            "좀 줘",
            "줘",
        ];

        let trimmed = query.trim();
        for pat in &patterns {
            if let Some(prefix) = trimmed.strip_suffix(pat) {
                return prefix.trim().to_string();
            }
        }

        // 물음표로 끝나는 패턴: "있어?", "있나?", "있을까?", "얼마야?" 등
        let q_patterns = [
            "있을까?",
            "있을까",
            "있어?",
            "있어",
            "있나?",
            "있나",
            "어디있어?",
            "어디있어",
            // 의문 표현
            "얼마야?",
            "얼마야",
            "얼마예요?",
            "얼마예요",
            "얼마인가요?",
            "얼마인가요",
            "얼마입니까?",
            "얼마입니까",
            "얼마인지",
            // 수량 의문
            "몇명이야?",
            "몇명이야",
            "몇명인가요?",
            "몇명인가요",
            "몇명인지",
            "몇 명이야?",
            "몇 명이야",
            "몇 명인가요?",
            "몇 명인가요",
            "몇 명인지",
            "몇개야?",
            "몇개야",
            "몇 개야?",
            "몇 개야",
            "몇건이야?",
            "몇건이야",
            "몇 건이야?",
            "몇 건이야",
            // 일반 의문
            "뭐야?",
            "뭐야",
            "뭔가요?",
            "뭔가요",
            "어디야?",
            "어디야",
            "언제야?",
            "언제야",
            "인가요?",
            "인가요",
            "인지",
        ];
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
                        *remaining = remaining.split_whitespace().collect::<Vec<_>>().join(" ");
                        found = true;
                        break;
                    } else if words[i].ends_with(suffix) && words[i].len() > suffix.len() {
                        // "부동산아닌" 붙여쓰기 패턴
                        let target = words[i][..words[i].len() - suffix.len()].to_string();
                        if !target.is_empty() {
                            excluded.push(target.clone());
                            parse_log.push(format!("제외: {}", target));
                            *remaining = remaining.replace(&words[i], " ");
                            *remaining = remaining.split_whitespace().collect::<Vec<_>>().join(" ");
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

    /// 파일명 필터 추출: "제목이 X인", "이름에 X 포함", "파일명에 X가 들어간" 등
    fn extract_filename_filter(
        remaining: &mut String,
        parse_log: &mut Vec<String>,
    ) -> Option<String> {
        // 파일명 관련 접두 키워드
        let prefixes: &[&str] = &[
            "제목이",
            "제목에",
            "제목의",
            "이름이",
            "이름에",
            "이름의",
            "파일명이",
            "파일명에",
            "파일명의",
            "파일이름이",
            "파일이름에",
            "파일이름의",
            "파일 이름이",
            "파일 이름에",
            "파일 이름의",
        ];

        // 접미 패턴 (값 뒤에 붙는 표현) — 긴 것부터 매칭
        let suffixes: &[&str] = &[
            "가 포함된",
            "이 포함된",
            "가 들어간",
            "이 들어간",
            "가 포함",
            "이 포함",
            "가 들어간",
            "포함된",
            "들어간",
            "포함",
            "있는",
            "인",
        ];

        let text = remaining.clone();
        let words: Vec<&str> = text.split_whitespace().collect();

        for prefix in prefixes {
            // 띄어쓰기 포함 접두사 처리 ("파일 이름이")
            let prefix_words: Vec<&str> = prefix.split_whitespace().collect();
            let prefix_word_count = prefix_words.len();

            for start_idx in 0..words.len() {
                // 접두사가 여러 단어인 경우 (예: "파일 이름이")
                if start_idx + prefix_word_count > words.len() {
                    continue;
                }

                let matches_prefix = prefix_words
                    .iter()
                    .enumerate()
                    .all(|(j, pw)| words[start_idx + j] == *pw);

                if !matches_prefix {
                    continue;
                }

                // 접두사 다음 단어가 값
                let value_start = start_idx + prefix_word_count;
                if value_start >= words.len() {
                    continue;
                }

                // 값 + 접미사 추출 (값은 1~2 단어)
                // 먼저 1단어 값 + 접미사 시도
                for value_count in (1..=2).rev() {
                    if value_start + value_count > words.len() {
                        continue;
                    }

                    let value_words = &words[value_start..value_start + value_count];
                    let after_value_idx = value_start + value_count;

                    // 마지막 값 단어에 접미사가 붙어있는지 확인
                    let last_val = value_words[value_count - 1];
                    for suffix in suffixes {
                        if let Some(clean_val) = last_val.strip_suffix(suffix) {
                            if clean_val.is_empty() {
                                continue;
                            }
                            let mut val_parts: Vec<&str> = value_words[..value_count - 1].to_vec();
                            val_parts.push(clean_val);
                            let value = val_parts.join(" ");

                            // remaining에서 매칭된 부분 제거
                            let mut new_words: Vec<&str> = Vec::new();
                            for (i, w) in words.iter().enumerate() {
                                if i < start_idx || i >= after_value_idx {
                                    new_words.push(w);
                                }
                            }
                            *remaining = new_words.join(" ");
                            parse_log.push(format!("파일명: {}", value));
                            return Some(value);
                        }
                    }

                    // 접미사가 다음 단어에 별도로 있는지 확인
                    if after_value_idx < words.len() {
                        let next_word = words[after_value_idx];
                        let mut suffix_word_count = 0;

                        // 2단어 접미사 ("가 포함된") 먼저 확인
                        if after_value_idx + 1 < words.len() {
                            let two_word = format!("{} {}", next_word, words[after_value_idx + 1]);
                            if suffixes.contains(&two_word.as_str()) {
                                suffix_word_count = 2;
                            }
                        }
                        // 1단어 접미사
                        if suffix_word_count == 0 && suffixes.contains(&next_word) {
                            suffix_word_count = 1;
                        }

                        if suffix_word_count > 0 {
                            let value = value_words.join(" ");
                            let remove_end = after_value_idx + suffix_word_count;
                            let mut new_words: Vec<&str> = Vec::new();
                            for (i, w) in words.iter().enumerate() {
                                if i < start_idx || i >= remove_end {
                                    new_words.push(w);
                                }
                            }
                            *remaining = new_words.join(" ");
                            parse_log.push(format!("파일명: {}", value));
                            return Some(value);
                        }
                    }

                    // 접미사 필수 — "제목에 관한 보고서" 등 오탐 방지
                    // 접미사 없으면 파일명 필터로 인식하지 않음
                }
            }
        }

        None
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
            DatePattern {
                patterns: &["작년", "작년도", "지난해", "전년", "전년도"],
                filter: DateFilter::LastYear,
                label: "작년",
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
                            start -= '에'.len_utf8();
                        }

                        let mut result = String::new();
                        result.push_str(remaining[..start].trim_end());
                        if !result.is_empty() && end < remaining.len() {
                            result.push(' ');
                        }
                        result.push_str(remaining[end..].trim_start());
                        *remaining = result.split_whitespace().collect::<Vec<_>>().join(" ");

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

        // "N월" 패턴 (올해의 해당 월)
        if let Some(filter) = Self::extract_month_pattern(remaining, parse_log) {
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
                    } else if (0..=99).contains(&year) {
                        2000 + year
                    } else {
                        continue;
                    };

                    if (1990..=2100).contains(&actual_year) {
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

    /// "N월" 패턴 (올해의 해당 월)
    fn extract_month_pattern(
        remaining: &mut String,
        parse_log: &mut Vec<String>,
    ) -> Option<DateFilter> {
        let words: Vec<String> = remaining.split_whitespace().map(String::from).collect();
        for (i, word) in words.iter().enumerate() {
            if let Some(month_str) = word.strip_suffix('월') {
                if let Ok(month) = month_str.parse::<u32>() {
                    if (1..=12).contains(&month) {
                        let mut new_words: Vec<String> = Vec::new();
                        for (j, w) in words.iter().enumerate() {
                            if j != i {
                                new_words.push(w.clone());
                            }
                        }
                        *remaining = new_words.join(" ");
                        parse_log.push(format!("날짜: {}월", month));
                        return Some(DateFilter::Month(month));
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
                let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !num_str.is_empty() {
                    if let Ok(days) = num_str.parse::<u32>() {
                        if days > 0 && days <= 365 {
                            // "일" 접미사 확인
                            let after_num = &after[num_str.len()..];
                            let end_offset =
                                if after_num.starts_with(" 일") || after_num.starts_with("일") {
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
                            *remaining = result.split_whitespace().collect::<Vec<_>>().join(" ");

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
    fn extract_file_type(remaining: &mut String, parse_log: &mut Vec<String>) -> Option<String> {
        struct FileTypePattern {
            patterns: Vec<&'static str>,
            file_type: &'static str,
            label: &'static str,
        }

        let ft_patterns = vec![
            FileTypePattern {
                patterns: vec![
                    "한글 문서",
                    "한글문서",
                    "한글 파일",
                    "한글파일",
                    "한글로 된",
                    "hwpx 문서",
                    "hwpx 파일",
                    "hwpx문서",
                    "hwpx파일",
                    "hwp 문서",
                    "hwp 파일",
                    "hwp문서",
                    "hwp파일",
                    "hwpx",
                    "hwp",
                ],
                file_type: "hwpx",
                label: "한글(hwpx)",
            },
            FileTypePattern {
                patterns: vec![
                    "워드 문서",
                    "워드문서",
                    "워드 파일",
                    "워드파일",
                    "docx 문서",
                    "docx 파일",
                    "docx문서",
                    "docx파일",
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
                    "xlsx 문서",
                    "xlsx 파일",
                    "xlsx문서",
                    "xlsx파일",
                    "xlsx",
                    "xls",
                    "excel",
                    "엑셀",
                ],
                file_type: "xlsx",
                label: "엑셀(xlsx)",
            },
            FileTypePattern {
                patterns: vec![
                    "pdf 문서",
                    "pdf문서",
                    "pdf 파일",
                    "pdf파일",
                    "피디에프",
                    "pdf",
                ],
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
                patterns: vec!["파워포인트", "피피티", "pptx", "ppt"],
                file_type: "pptx",
                label: "파워포인트(pptx)",
            },
        ];

        let lower = remaining.to_lowercase();

        // 긴 패턴부터 매칭 (정확도 우선)
        for ftp in &ft_patterns {
            let mut sorted_patterns = ftp.patterns.clone();
            sorted_patterns.sort_by_key(|b| std::cmp::Reverse(b.len()));

            for pat in &sorted_patterns {
                let pat_lower = pat.to_lowercase();
                if let Some(pos) = lower.find(&pat_lower) {
                    // 단어 경계 확인
                    let before_ok = pos == 0 || remaining[..pos].ends_with(' ');
                    let after_pos = pos + pat.len();
                    let after_ok = after_pos >= remaining.len()
                        || remaining[after_pos..].starts_with(' ')
                        || remaining[after_pos..].starts_with("만")
                        || remaining[after_pos..].starts_with("으로")
                        || remaining[after_pos..].starts_with("로")
                        || remaining[after_pos..].starts_with("에서")
                        || remaining[after_pos..].starts_with("문서")
                        || remaining[after_pos..].starts_with("파일");

                    if before_ok && after_ok {
                        // 패턴 + 뒤의 접미 표현 제거 (긴 패턴부터)
                        let mut end = after_pos;
                        let rest = &remaining[end..];
                        for postfix in &[
                            " 문서",
                            " 파일",
                            "문서",
                            "파일",
                            "으로 된",
                            "으로",
                            "로 된",
                            "로",
                            "만",
                            "에서",
                        ] {
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
                        *remaining = result.split_whitespace().collect::<Vec<_>>().join(" ");

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
mod tests;
