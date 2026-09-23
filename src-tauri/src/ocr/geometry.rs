//! 기하 유틸리티: 윤곽선 검출, 최소 회전 사각형, crop & warp

use image::RgbImage;

/// 바운딩 박스 (4점 좌표, 시계방향: 좌상→우상→우하→좌하)
#[derive(Debug, Clone)]
pub struct Quad {
    pub points: [(f32, f32); 4],
    pub score: f32,
}

/// 이진 맵에서 연결 컴포넌트(텍스트 영역)를 BFS로 검출
pub fn find_connected_components(
    binary: &[u8],
    width: usize,
    height: usize,
    min_area: usize,
) -> Vec<Vec<(usize, usize)>> {
    let mut visited = vec![false; width * height];
    let mut components = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if binary[idx] == 0 || visited[idx] {
                continue;
            }
            // BFS flood fill
            let mut queue = std::collections::VecDeque::new();
            let mut component = Vec::new();
            queue.push_back((x, y));
            visited[idx] = true;

            while let Some((cx, cy)) = queue.pop_front() {
                component.push((cx, cy));
                // 4-연결
                for (dx, dy) in &[(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let ni = ny as usize * width + nx as usize;
                    if !visited[ni] && binary[ni] > 0 {
                        visited[ni] = true;
                        queue.push_back((nx as usize, ny as usize));
                    }
                }
            }

            if component.len() >= min_area {
                components.push(component);
            }
        }
    }

    components
}

/// 연결 컴포넌트에서 축 정렬 바운딩 박스 추출 + 마진 확장
pub fn bounding_box_with_margin(
    component: &[(usize, usize)],
    margin_ratio: f32,
    img_width: usize,
    img_height: usize,
) -> Quad {
    let mut min_x = usize::MAX;
    let mut min_y = usize::MAX;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for &(x, y) in component {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    // 마진 확장
    let w = (max_x - min_x) as f32;
    let h = (max_y - min_y) as f32;
    let mx = (w * margin_ratio).max(2.0);
    let my = (h * margin_ratio).max(2.0);

    let x1 = (min_x as f32 - mx).max(0.0);
    let y1 = (min_y as f32 - my).max(0.0);
    let x2 = (max_x as f32 + mx).min(img_width as f32 - 1.0);
    let y2 = (max_y as f32 + my).min(img_height as f32 - 1.0);

    Quad {
        points: [
            (x1, y1), // 좌상
            (x2, y1), // 우상
            (x2, y2), // 우하
            (x1, y2), // 좌하
        ],
        score: 0.0,
    }
}

/// 확률 맵에서 바운딩 박스 영역 내 평균 점수 계산
pub fn compute_box_score(
    prob_map: &[f32],
    map_width: usize,
    quad: &Quad,
    scale_x: f32,
    scale_y: f32,
) -> f32 {
    let x1 = (quad.points[0].0 * scale_x) as usize;
    let y1 = (quad.points[0].1 * scale_y) as usize;
    let x2 = (quad.points[1].0 * scale_x) as usize;
    let y2 = (quad.points[2].1 * scale_y) as usize;

    let mut sum = 0.0f32;
    let mut count = 0u32;
    for y in y1..=y2 {
        for x in x1..=x2 {
            let idx = y * map_width + x;
            if idx < prob_map.len() {
                sum += prob_map[idx];
                count += 1;
            }
        }
    }

    if count > 0 {
        sum / count as f32
    } else {
        0.0
    }
}

/// Quad 영역을 원본 이미지에서 crop하여 수평 직사각형으로 변환.
/// 원본은 호출자가 RGB 로 한 번만 바꿔 넘긴다 (종전엔 상자마다 전체 이미지를 변환했다).
pub fn crop_quad(rgb: &RgbImage, quad: &Quad) -> RgbImage {
    let (img_w, img_h) = (rgb.width(), rgb.height());

    let x1 = quad.points[0].0.max(0.0) as u32;
    let y1 = quad.points[0].1.max(0.0) as u32;
    let x2 = quad.points[1].0.min(img_w as f32 - 1.0) as u32;
    let y2 = quad.points[2].1.min(img_h as f32 - 1.0) as u32;

    let crop_w = (x2.saturating_sub(x1)).max(1);
    let crop_h = (y2.saturating_sub(y1)).max(1);

    let mut cropped = RgbImage::new(crop_w, crop_h);
    for cy in 0..crop_h {
        for cx in 0..crop_w {
            let sx = x1 + cx;
            let sy = y1 + cy;
            if sx < img_w && sy < img_h {
                cropped.put_pixel(cx, cy, *rgb.get_pixel(sx, sy));
            }
        }
    }

    // 세로가 가로보다 1.5배 이상 길면 90도 회전
    if crop_h as f32 / crop_w as f32 >= 1.5 {
        rotate_90_ccw(&cropped)
    } else {
        cropped
    }
}

/// 90도 반시계 회전
fn rotate_90_ccw(img: &RgbImage) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    let mut rotated = RgbImage::new(h, w);
    for y in 0..h {
        for x in 0..w {
            rotated.put_pixel(y, w - 1 - x, *img.get_pixel(x, y));
        }
    }
    rotated
}

/// 바운딩 박스를 y좌표 → x좌표 순으로 정렬 (읽기 순서)
pub fn sort_boxes_reading_order(boxes: &mut [Quad]) {
    sort_reading_lines(
        boxes,
        |q| q.points[0].1,
        |q| q.points[2].1 - q.points[0].1,
        |q| q.points[0].0,
    );
}

/// 읽기 순서 정렬: 위쪽 좌표순으로 늘어놓고, 줄 첫 상자와 위쪽 차이가 높이(둘 중 작은 쪽)의 50%
/// 미만이면 같은 줄로 묶어 줄 안에서 왼쪽부터 놓는다.
///
/// 종전엔 "두 상자의 차이가 임계 미만이면 x, 아니면 y" 를 비교 함수로 썼는데, 임계가 쌍마다 달라
/// 추이성이 깨진다(A≈B, B≈C 인데 A·C 는 다른 줄). Rust 1.81+ 정렬은 이런 비교 함수에서 패닉할 수 있다.
pub(crate) fn sort_reading_lines<T: Clone>(
    items: &mut [T],
    top: impl Fn(&T) -> f32,
    height: impl Fn(&T) -> f32,
    left: impl Fn(&T) -> f32,
) {
    items.sort_by(|a, b| top(a).total_cmp(&top(b)));
    let mut start = 0;
    while start < items.len() {
        let (anchor_top, anchor_h) = (top(&items[start]), height(&items[start]));
        let mut end = start + 1;
        while end < items.len()
            && (top(&items[end]) - anchor_top).abs() < anchor_h.min(height(&items[end])) * 0.5
        {
            end += 1;
        }
        items[start..end].sort_by(|a, b| left(a).total_cmp(&left(b)));
        start = end;
    }
}

#[cfg(test)]
mod reading_order_tests {
    use super::sort_reading_lines;

    /// (위, 높이, 왼쪽) 상자. 줄 첫 상자 기준으로 묶고 줄 안은 왼쪽부터.
    #[test]
    fn groups_lines_and_orders_left_to_right() {
        let mut boxes = vec![
            (21.0f32, 10.0f32, 5.0f32, "둘째줄-왼"),
            (0.0, 10.0, 50.0, "첫줄-오른"),
            (3.0, 10.0, 10.0, "첫줄-왼"),
            (22.0, 10.0, 40.0, "둘째줄-오른"),
        ];
        sort_reading_lines(&mut boxes, |b| b.0, |b| b.1, |b| b.2);
        let names: Vec<_> = boxes.iter().map(|b| b.3).collect();
        assert_eq!(names, ["첫줄-왼", "첫줄-오른", "둘째줄-왼", "둘째줄-오른"]);
    }

    /// 종전 비교 함수가 추이성을 깨던 배치(높이가 들쭉날쭉한 상자 다수)에서도 패닉하지 않는다.
    #[test]
    fn mixed_heights_do_not_panic() {
        let mut boxes: Vec<(f32, f32, f32)> = (0..400)
            .map(|i| {
                let i = i as f32;
                ((i * 7.3) % 97.0, 1.0 + (i * 3.1) % 13.0, (i * 11.7) % 211.0)
            })
            .collect();
        sort_reading_lines(&mut boxes, |b| b.0, |b| b.1, |b| b.2);
        assert_eq!(boxes.len(), 400);
    }
}
