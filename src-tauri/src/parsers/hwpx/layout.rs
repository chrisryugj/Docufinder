use std::collections::HashMap;

use super::models::{DefaultStyle, PageSettings, ParagraphNode, StyleData};
use super::char_weight_units;

/// 가상 레이아웃 시뮬레이터 (Y좌표 추적 기반)
pub(super) struct LayoutSimulator {
    /// 현재 Y 위치 (hwpunit)
    current_y: f32,
    /// 페이지 유효 높이 (hwpunit)
    max_height: f32,
    /// 줄 높이 (hwpunit)
    line_height: f32,
    /// 한 줄 최대 가중치 유닛 수 (ASCII=1.0, 전각=2.0)
    max_units_per_line: f32,
}

impl LayoutSimulator {
    pub fn new(page: &PageSettings, style: &DefaultStyle) -> Self {
        // 유효 높이 = 페이지 높이 - 상단여백 - 하단여백 - 머리말 - 꼬리말
        let max_height = page
            .height
            .saturating_sub(page.top_margin)
            .saturating_sub(page.bottom_margin)
            .saturating_sub(page.header_offset)
            .saturating_sub(page.footer_offset) as f32;

        // 유효 너비 = 페이지 너비 - 좌측여백 - 우측여백
        let effective_width = page
            .width
            .saturating_sub(page.left_margin)
            .saturating_sub(page.right_margin) as f32;

        // 줄 높이 = 글자크기 × (줄간격 / 100)
        let font_size = style.font_size.max(100) as f32;
        let line_height = (font_size * style.line_spacing.max(80) as f32 / 100.0).max(100.0);

        let max_height = max_height.max(line_height);
        let effective_width = effective_width.max(font_size);
        let max_units_per_line = (effective_width / (font_size * 0.5)).max(10.0);

        Self {
            current_y: 0.0,
            max_height,
            line_height,
            max_units_per_line,
        }
    }

    /// 문단 레이아웃 (lineSeg 우선 → 스타일 기반 시뮬레이션 fallback)
    pub fn layout_paragraph(
        &mut self,
        para: &ParagraphNode,
        page_starts: &mut Vec<usize>,
        styles: &HashMap<String, StyleData>,
        default_style: &DefaultStyle,
    ) {
        if para.has_page_break_before {
            self.apply_page_break(page_starts, para.char_offset);
        }

        // 빈 문단은 한 줄 처리
        if para.text.trim().is_empty() && para.object_height <= 0.0 && para.line_segs.is_empty() {
            self.advance_line(page_starts, para.char_offset);
            return;
        }

        // === 경로 1: lineSeg 데이터가 있으면 HWP 렌더러 결과 직접 사용 ===
        if !para.line_segs.is_empty() {
            for seg in &para.line_segs {
                let offset = para.char_offset + seg.text_start_pos;
                let height = seg.line_height.max(100) as f32;
                if self.current_y + height > self.max_height {
                    self.apply_page_break(page_starts, offset);
                }
                self.current_y += height;
            }
        } else {
            // === 경로 2: 스타일 기반 시뮬레이션 ===
            let (line_h, max_units) = self.resolve_style(para, styles, default_style);

            if para.text.trim().is_empty() {
                // 텍스트 없이 객체만 있는 문단
                // (객체 높이는 아래서 처리)
            } else {
                let mut line_units = 0.0_f32;
                let mut line_start_offset = 0usize;

                for (idx, ch) in para.text.chars().enumerate() {
                    if ch == '\n' {
                        self.advance_line_with(
                            page_starts,
                            para.char_offset + line_start_offset,
                            line_h,
                        );
                        line_units = 0.0;
                        line_start_offset = idx + 1;
                        continue;
                    }

                    let weight = char_weight_units(ch);
                    if line_units + weight > max_units {
                        self.advance_line_with(
                            page_starts,
                            para.char_offset + line_start_offset,
                            line_h,
                        );
                        line_units = weight;
                        line_start_offset = idx;
                    } else {
                        line_units += weight;
                    }
                }

                // 마지막 라인 처리
                self.advance_line_with(page_starts, para.char_offset + line_start_offset, line_h);
            }
        }

        // === 객체 높이 반영 (이미지/표 등) ===
        // lineSeg 경로에서도 inline이 아닌 블록 객체 높이는 별도 반영
        // (lineSeg는 글자 취급 객체만 포함하므로 블록 객체는 추가 필요)
        if para.object_height > 0.0 {
            if self.current_y + para.object_height > self.max_height {
                self.apply_page_break(page_starts, para.char_offset);
            }
            self.current_y += para.object_height;
        }
    }

    /// 문단별 스타일 해석 → (line_height, max_units_per_line)
    fn resolve_style(
        &self,
        para: &ParagraphNode,
        styles: &HashMap<String, StyleData>,
        default_style: &DefaultStyle,
    ) -> (f32, f32) {
        let style_data = para.style_id.as_ref().and_then(|id| styles.get(id));

        let font_sz = style_data
            .and_then(|s| s.font_size)
            .unwrap_or(default_style.font_size)
            .max(100) as f32;

        let line_sp = style_data
            .and_then(|s| s.line_spacing)
            .unwrap_or(default_style.line_spacing)
            .max(80) as f32;

        let line_h = (font_sz * line_sp / 100.0).max(100.0);
        let max_units = (self.max_units_per_line * self.line_height / line_h).max(10.0);

        (line_h, max_units)
    }

    fn apply_page_break(&mut self, page_starts: &mut Vec<usize>, offset: usize) {
        if page_starts.last().copied() != Some(offset) {
            page_starts.push(offset);
        }
        self.current_y = 0.0;
    }

    fn advance_line(&mut self, page_starts: &mut Vec<usize>, line_start_offset: usize) {
        if self.current_y + self.line_height > self.max_height {
            self.apply_page_break(page_starts, line_start_offset);
        }
        self.current_y += self.line_height;
    }

    fn advance_line_with(
        &mut self,
        page_starts: &mut Vec<usize>,
        line_start_offset: usize,
        height: f32,
    ) {
        if self.current_y + height > self.max_height {
            self.apply_page_break(page_starts, line_start_offset);
        }
        self.current_y += height;
    }
}

#[derive(Debug, Clone)]
pub(super) struct PageMap {
    pub page_starts: Vec<usize>,
}

impl PageMap {
    pub fn total_pages(&self) -> usize {
        self.page_starts.len().max(1)
    }

    pub fn page_for_offset(&self, char_offset: usize) -> usize {
        let idx = self
            .page_starts
            .partition_point(|&start| start <= char_offset);
        idx.max(1)
    }
}
