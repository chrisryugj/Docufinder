//! XY-Cut 읽기순서 — OCR 텍스트 영역 bbox 를 사람이 읽는 순서로 정렬한다.
//!
//! kordoc `src/pdf/xy-cut.ts` (XY-Cut++, OpenDataLoader-PDF Apache-2.0 포팅)의
//! 핵심 재귀 컷을 이미지 좌표계(y 아래로 증가)로 이식. cross-layout 마스킹(①)은
//! ODL beta=2.0 에서 사실상 비활성이라 생략하고, 좁은요소 아웃라이어 필터(②)와
//! 양방향 컷(③)만 유지 — 2단 스캔본의 좌우 인터리브 방지가 목적.

use super::BBox;

/// 재귀 깊이 제한 — pathological 레이아웃 스택 오버플로 방지
const MAX_DEPTH: usize = 50;
/// 분할 최소 갭(px) — 미세 갭 분할 방지 (ODL MIN_GAP_THRESHOLD)
const MIN_GAP: f32 = 5.0;
/// 좁은 요소 아웃라이어 필터: 영역 폭 대비 비율 (쪽번호·각주 마커)
const NARROW_ELEMENT_WIDTH_RATIO: f32 = 0.1;

struct Cut {
    position: f32,
    gap: f32,
}

/// bbox 집합 → 읽기순서 인덱스 배열. 반환 `order[k]` = k번째로 읽을 원본 인덱스.
pub fn reading_order(boxes: &[BBox]) -> Vec<usize> {
    let items: Vec<usize> = (0..boxes.len()).collect();
    let mut out = Vec::with_capacity(boxes.len());
    xy_cut(boxes, &items, 0, &mut out);
    out
}

fn xy_cut(boxes: &[BBox], items: &[usize], depth: usize, out: &mut Vec<usize>) {
    if items.is_empty() {
        return;
    }
    if items.len() <= 2 || depth >= MAX_DEPTH {
        emit_leaf(boxes, items, out);
        return;
    }

    let h = find_horizontal_cut(boxes, items);
    let v = find_vertical_cut_filtered(boxes, items, MIN_GAP);
    let h_valid = h.gap >= MIN_GAP;
    let v_valid = v.gap >= MIN_GAP;

    // 축 선택: 기본 Y(수평 컷) 우선. 단 수직 갭이 수평 갭보다 명백히 크면(1.5×)
    // 컬럼 분리로 보고 X 우선 — 2단에서 문단 간 수평 갭이 단 사이 수직 갭보다 먼저
    // 잡혀 행 단위로 인터리브되는 문제 방지 (kordoc 보수적 적용).
    let use_horizontal = if h_valid && v_valid {
        v.gap <= h.gap * 1.5
    } else if h_valid {
        true
    } else if v_valid {
        false
    } else {
        emit_leaf(boxes, items, out);
        return;
    };

    if use_horizontal {
        // 이미지 좌표: y 작을수록 위 → 위 밴드 먼저
        let (upper, lower): (Vec<usize>, Vec<usize>) = items
            .iter()
            .copied()
            .partition(|&i| y_center(&boxes[i]) < h.position);
        if !upper.is_empty() && !lower.is_empty() && upper.len() < items.len() {
            xy_cut(boxes, &upper, depth + 1, out);
            xy_cut(boxes, &lower, depth + 1, out);
            return;
        }
    } else {
        let (left, right): (Vec<usize>, Vec<usize>) = items
            .iter()
            .copied()
            .partition(|&i| x_center(&boxes[i]) < v.position);
        if !left.is_empty() && !right.is_empty() && left.len() < items.len() {
            xy_cut(boxes, &left, depth + 1, out);
            xy_cut(boxes, &right, depth + 1, out);
            return;
        }
    }

    emit_leaf(boxes, items, out);
}

fn x_center(b: &BBox) -> f32 {
    (b.x0 + b.x1) / 2.0
}

fn y_center(b: &BBox) -> f32 {
    (b.y0 + b.y1) / 2.0
}

/// 리프 방출 — 같은 행(높이 50% 이내)은 좌→우, 그 외 위→아래
fn emit_leaf(boxes: &[BBox], items: &[usize], out: &mut Vec<usize>) {
    let mut v = items.to_vec();
    v.sort_by(|&a, &b| {
        let ba = &boxes[a];
        let bb = &boxes[b];
        let threshold = (ba.y1 - ba.y0).min(bb.y1 - bb.y0) * 0.5;
        // total_cmp: NaN 전이성 위반 방지 (geometry::sort_boxes_reading_order 와 동일 규약)
        if (ba.y0 - bb.y0).abs() < threshold {
            ba.x0.total_cmp(&bb.x0)
        } else {
            ba.y0.total_cmp(&bb.y0)
        }
    });
    out.extend(v);
}

/// 수평 컷(Y축 분할) — 위→아래 정렬 후 인접 요소 사이 가장 넓은 세로 갭
fn find_horizontal_cut(boxes: &[BBox], items: &[usize]) -> Cut {
    if items.len() < 2 {
        return Cut {
            position: 0.0,
            gap: 0.0,
        };
    }
    let mut sorted = items.to_vec();
    sorted.sort_by(|&a, &b| boxes[a].y0.total_cmp(&boxes[b].y0));
    let mut largest = 0.0f32;
    let mut position = 0.0f32;
    for pair in sorted.windows(2) {
        let prev_bottom = boxes[pair[0]].y1;
        let curr_top = boxes[pair[1]].y0;
        let gap = curr_top - prev_bottom;
        if gap > largest {
            largest = gap;
            position = (prev_bottom + curr_top) / 2.0;
        }
    }
    Cut {
        position,
        gap: largest,
    }
}

/// 수직 컷(X축 분할) — 갭이 안 나오면 좁은 요소(쪽번호류) 제외 후 재시도 (ODL ②)
fn find_vertical_cut_filtered(boxes: &[BBox], items: &[usize], min_gap: f32) -> Cut {
    let edge = find_vertical_cut(boxes, items);
    if edge.gap >= min_gap {
        return edge;
    }
    if items.len() >= 3 {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        for &i in items {
            min_x = min_x.min(boxes[i].x0);
            max_x = max_x.max(boxes[i].x1);
        }
        let narrow = (max_x - min_x) * NARROW_ELEMENT_WIDTH_RATIO;
        let filtered: Vec<usize> = items
            .iter()
            .copied()
            .filter(|&i| (boxes[i].x1 - boxes[i].x0) >= narrow)
            .collect();
        // 아웃라이어는 소수여야 함 — 70% 이상 유지될 때만 재시도 (본문 가짜 컬럼 갭 방지)
        if filtered.len() >= 2
            && filtered.len() < items.len()
            && filtered.len() as f32 >= items.len() as f32 * 0.7
        {
            let fc = find_vertical_cut(boxes, &filtered);
            if fc.gap > edge.gap && fc.gap >= min_gap {
                return fc;
            }
        }
    }
    edge
}

/// 수직 컷 — X 프로젝션 최대 갭 (누적 최대 우변 추적)
fn find_vertical_cut(boxes: &[BBox], items: &[usize]) -> Cut {
    if items.len() < 2 {
        return Cut {
            position: 0.0,
            gap: 0.0,
        };
    }
    let mut sorted = items.to_vec();
    sorted.sort_by(|&a, &b| {
        boxes[a]
            .x0
            .total_cmp(&boxes[b].x0)
            .then(boxes[a].x1.total_cmp(&boxes[b].x1))
    });
    let mut largest = 0.0f32;
    let mut position = 0.0f32;
    let mut prev_right: Option<f32> = None;
    for &i in &sorted {
        let left = boxes[i].x0;
        let right = boxes[i].x1;
        match prev_right {
            Some(pr) => {
                if left > pr {
                    let gap = left - pr;
                    if gap > largest {
                        largest = gap;
                        position = (pr + left) / 2.0;
                    }
                }
                prev_right = Some(pr.max(right));
            }
            None => prev_right = Some(right),
        }
    }
    Cut {
        position,
        gap: largest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bb(x0: f32, y0: f32, x1: f32, y1: f32) -> BBox {
        BBox { x0, y0, x1, y1 }
    }

    #[test]
    fn two_column_descrambles() {
        // 좌단(x0..100)·우단(x200..300) 각 2행. naive(검출) 순서는 L1,R1,L2,R2 로 뒤섞임.
        let boxes = [
            bb(0.0, 0.0, 100.0, 20.0),    // 0: L1
            bb(200.0, 0.0, 300.0, 20.0),  // 1: R1
            bb(0.0, 30.0, 100.0, 50.0),   // 2: L2
            bb(200.0, 30.0, 300.0, 50.0), // 3: R2
        ];
        // 컬럼 순서 = 좌단 위→아래, 우단 위→아래
        assert_eq!(reading_order(&boxes), vec![0, 2, 1, 3]);
    }

    #[test]
    fn single_column_preserves_order() {
        let boxes = [
            bb(0.0, 0.0, 100.0, 20.0),
            bb(0.0, 30.0, 100.0, 50.0),
            bb(0.0, 60.0, 100.0, 80.0),
        ];
        assert_eq!(reading_order(&boxes), vec![0, 1, 2]);
    }

    #[test]
    fn handles_empty_and_single() {
        assert_eq!(reading_order(&[]), Vec::<usize>::new());
        assert_eq!(reading_order(&[bb(0.0, 0.0, 10.0, 10.0)]), vec![0]);
    }
}
