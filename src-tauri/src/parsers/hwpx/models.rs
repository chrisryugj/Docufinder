/// HWPX 페이지 설정 (단위: hwpunit, 1pt = 100 hwpunit)
#[derive(Debug, Clone)]
pub(super) struct PageSettings {
    /// 페이지 높이 (hwpunit)
    pub height: u32,
    /// 페이지 너비 (hwpunit)
    pub width: u32,
    /// 상단 여백 (hwpunit)
    pub top_margin: u32,
    /// 하단 여백 (hwpunit)
    pub bottom_margin: u32,
    /// 좌측 여백 (hwpunit)
    pub left_margin: u32,
    /// 우측 여백 (hwpunit)
    pub right_margin: u32,
    /// 머리말 영역 (hwpunit)
    pub header_offset: u32,
    /// 꼬리말 영역 (hwpunit)
    pub footer_offset: u32,
}

impl Default for PageSettings {
    fn default() -> Self {
        // A4 기본값 (한글 기본 설정)
        Self {
            height: 84188,       // 약 297mm (A4)
            width: 59528,        // 약 210mm (A4)
            top_margin: 5668,    // 약 20mm
            bottom_margin: 4252, // 약 15mm
            left_margin: 4252,   // 약 15mm
            right_margin: 4252,  // 약 15mm
            header_offset: 4252,
            footer_offset: 4252,
        }
    }
}

/// 기본 스타일 정보
#[derive(Debug, Clone)]
pub(super) struct DefaultStyle {
    /// 기본 글자 크기 (hwpunit, 1pt = 100)
    pub font_size: u32,
    /// 줄간격 (%, 예: 160 = 160%)
    pub line_spacing: u32,
}

impl Default for DefaultStyle {
    fn default() -> Self {
        Self {
            font_size: 1000,   // 10pt
            line_spacing: 160, // 160%
        }
    }
}

/// 스타일별 속성 (styles.xml에서 파싱)
#[derive(Debug, Clone)]
pub(super) struct StyleData {
    pub font_size: Option<u32>,    // hwpunit (1pt = 100)
    pub line_spacing: Option<u32>, // %
}

/// LineSeg (HWP 렌더러가 계산한 줄 레이아웃)
#[derive(Debug, Clone)]
pub(super) struct LineSeg {
    /// 문단 내 시작 글자 위치
    pub text_start_pos: usize,
    /// 줄 높이 (hwpunit, spacing 포함)
    pub line_height: u32,
}

/// 문단 노드 (구조적 파싱용)
#[derive(Debug, Clone)]
pub(super) struct ParagraphNode {
    /// 문단 텍스트
    pub text: String,
    /// 전체 텍스트 내 시작 오프셋 (글자 수 기준)
    pub char_offset: usize,
    /// 이 문단 앞에 강제 쪽 나눔이 있는지
    pub has_page_break_before: bool,
    /// 참조 스타일 ID (styleIDRef)
    pub style_id: Option<String>,
    /// 내장 객체(이미지/표 등)의 총 높이 (hwpunit)
    pub object_height: f32,
    /// HWP 렌더러의 줄 레이아웃 데이터
    pub line_segs: Vec<LineSeg>,
}
