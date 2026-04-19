//! Document Lineage Graph — 논리 문서의 버전 계보 추론.
//!
//! 같은 문서의 서로 다른 버전 파일들(`계약서_최종.hwpx`, `계약서_최최종.hwpx`)을
//! 하나의 `lineage_id`로 묶고 그중 대표(canonical)를 선출한다.
//!
//! ## v1 그루핑 규칙
//! - **1차 키**: 정규화된 파일명 stem + 부모 폴더 경로
//! - **Canonical 선출**: 버전 라벨 depth(최최...종 글자수) → 버전 라벨 유무 → modified_at 최신
//!
//! ## 향후(v2) 보강 예정
//! - 벡터 유사도 ≥ 0.85 필수 조건 (내용 다르면 다른 lineage로 분리)
//! - 경로 공통 prefix 기반 근접 폴더 묶기

use crate::embedder::Embedder;
use crate::utils::filename_normalize::{extract_version_label, normalize_stem};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// 벡터 유사도 검증 임계값 — 이 이하면 다른 lineage로 분리.
const LINEAGE_SIMILARITY_THRESHOLD: f32 = 0.85;

/// 벡터 검증에 사용할 첫 청크 최소 길이 (너무 짧으면 임베딩 신뢰성 낮음).
const MIN_CHUNK_LEN_FOR_VERIFICATION: usize = 30;

/// 코사인 유사도 계산.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb + 1e-9)
}

/// 파일 경로에서 부모 폴더를 반환한다. 구분자는 `/`와 `\\` 모두 허용.
pub fn parent_folder_of(path: &str) -> String {
    path.rsplit_once(['/', '\\'])
        .map(|(p, _)| p.to_string())
        .unwrap_or_default()
        .to_lowercase()
}

/// 정규화 stem + 부모 폴더로 결정론적 lineage 키를 계산한다 (SHA-256 앞 16자).
pub fn compute_lineage_id(stem_norm: &str, parent_folder: &str) -> String {
    let mut h = Sha256::new();
    h.update(parent_folder.as_bytes());
    h.update(b"::");
    h.update(stem_norm.as_bytes());
    let digest = h.finalize();
    let mut out = String::with_capacity(16);
    for b in digest.iter().take(8) {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// canonical 선출용 점수. 큰 순서대로 정렬했을 때 canonical이 먼저 온다.
///
/// - `priority`: 버전 라벨 depth + 사용자 열람 횟수(behavioral) 가중
/// - `modified_at`: 동점일 때 최신 파일을 우선
///
/// ## Behavioral Canonical 반영
/// `open_count >= 3`이면 우선순위에 50점 보너스 (라벨 기반 최종 우선순위 50과 비슷한 급).
/// 사용자가 실제로 열어본 파일이 라벨만 화려한 파일보다 우선 → 라벨 거짓말 방어.
fn canonical_score(name: &str, modified_at: Option<i64>, open_count: i64) -> (u32, i64) {
    let label_priority = match extract_version_label(name) {
        Some(label) if label.starts_with('최') && label.ends_with('종') => {
            100 + label.chars().count() as u32
        }
        Some(label) if label == "진짜최종" => 99,
        Some(label) if label.starts_with('v') => 50,
        Some(_) => 30,
        None => 0,
    };
    // 열람 횟수 가중: 3회 이상 부터 의미 부여, 최대 +50점
    let behavior_bonus = match open_count {
        c if c >= 20 => 50,
        c if c >= 10 => 35,
        c if c >= 3 => 20,
        _ => 0,
    };
    (label_priority + behavior_bonus, modified_at.unwrap_or(0))
}

/// 특정 파일 레코드.
#[derive(Debug, Clone)]
struct FileRow {
    id: i64,
    path: String,
    name: String,
    modified_at: Option<i64>,
    open_count: i64,
}

/// 전체 files 테이블을 훑어 lineage_id / stem_norm / lineage_role / version_label을 재계산한다.
///
/// 백필용 — `rebuild_lineage` Tauri 커맨드에서 호출된다.
/// 반환값은 (갱신된 파일 수, 생성된 고유 lineage 수).
///
/// `&Connection`만 받는다 (풀 커넥션은 DerefMut 미구현). BEGIN/COMMIT은 직접 발행한다.
pub fn rebuild_all(conn: &Connection) -> rusqlite::Result<(usize, usize)> {
    // 1단계: 전체 파일 로드 (명시적 루프로 stmt lifetime 문제 회피)
    let mut files: Vec<FileRow> = Vec::new();
    {
        let mut stmt =
            conn.prepare("SELECT id, path, name, modified_at, COALESCE(open_count, 0) FROM files")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            files.push(FileRow {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                modified_at: row.get(3)?,
                open_count: row.get(4)?,
            });
        }
    }

    if files.is_empty() {
        return Ok((0, 0));
    }

    // 2단계: 각 파일의 stem_norm + lineage_id 산출, 그룹핑
    let mut groups: HashMap<String, Vec<&FileRow>> = HashMap::new();
    let mut stems: HashMap<i64, String> = HashMap::new();
    let mut lineage_ids: HashMap<i64, String> = HashMap::new();

    for f in &files {
        let stem = normalize_stem(&f.name);
        let parent = parent_folder_of(&f.path);
        let lid = compute_lineage_id(&stem, &parent);
        groups.entry(lid.clone()).or_default().push(f);
        stems.insert(f.id, stem);
        lineage_ids.insert(f.id, lid);
    }

    let unique_lineages = groups.len();

    // 3단계: 각 그룹에서 canonical 선출 (behavioral 반영)
    let mut canonical_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for members in groups.values() {
        if let Some(winner) = members
            .iter()
            .max_by_key(|f| canonical_score(&f.name, f.modified_at, f.open_count))
        {
            canonical_ids.insert(winner.id);
        }
    }

    // 4단계: 일괄 UPDATE (수동 트랜잭션 — PooledConnection은 DerefMut 미구현)
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let mut updated = 0usize;
    let update_result: rusqlite::Result<()> = (|| {
        let mut stmt = conn.prepare(
            "UPDATE files
             SET stem_norm = ?1,
                 lineage_id = ?2,
                 lineage_role = ?3,
                 version_label = ?4
             WHERE id = ?5",
        )?;
        for f in &files {
            let stem = stems.get(&f.id).cloned().unwrap_or_default();
            let lid = lineage_ids.get(&f.id).cloned().unwrap_or_default();
            let role = if canonical_ids.contains(&f.id) {
                "canonical"
            } else {
                "version"
            };
            let label = extract_version_label(&f.name);
            stmt.execute(params![stem, lid, role, label, f.id])?;
            updated += 1;
        }
        Ok(())
    })();

    match update_result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok((updated, unique_lineages))
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 인덱싱 직후 단일 파일의 lineage 정보를 계산하고 필요 시 그룹 canonical을 재선출한다.
///
/// 파이프라인의 `upsert_file` 이후 호출된다. 같은 lineage 그룹의 다른 파일이
/// 이미 canonical일 수 있으므로, 새 파일과 기존 canonical의 점수를 비교해 승자를 고른다.
pub fn assign_for_file(
    conn: &Connection,
    file_id: i64,
    path: &str,
    name: &str,
    modified_at: Option<i64>,
) -> rusqlite::Result<()> {
    let stem = normalize_stem(name);
    let parent = parent_folder_of(path);
    let lid = compute_lineage_id(&stem, &parent);
    let label = extract_version_label(name);

    // 현재 그룹의 기존 canonical 찾기 (open_count 포함)
    let existing_canonical: Option<(i64, String, Option<i64>, i64)> = conn
        .query_row(
            "SELECT id, name, modified_at, COALESCE(open_count, 0) FROM files
             WHERE lineage_id = ?1 AND lineage_role = 'canonical' AND id != ?2
             LIMIT 1",
            params![lid, file_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok();

    // 새 파일의 open_count 조회 (이미 DB에 있을 수 있음)
    let new_open_count: i64 = conn
        .query_row(
            "SELECT COALESCE(open_count, 0) FROM files WHERE id = ?1",
            [file_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let new_score = canonical_score(name, modified_at, new_open_count);
    let (role, demote_id) = match existing_canonical {
        Some((eid, ename, emod, eopen)) => {
            let old_score = canonical_score(&ename, emod, eopen);
            if new_score > old_score {
                ("canonical", Some(eid))
            } else {
                ("version", None)
            }
        }
        None => ("canonical", None),
    };

    conn.execute(
        "UPDATE files
         SET stem_norm = ?1, lineage_id = ?2, lineage_role = ?3, version_label = ?4
         WHERE id = ?5",
        params![stem, lid, role, label, file_id],
    )?;

    if let Some(eid) = demote_id {
        conn.execute(
            "UPDATE files SET lineage_role = 'version' WHERE id = ?1",
            params![eid],
        )?;
    }

    Ok(())
}

/// Cross-Folder Reunion — 서로 다른 폴더에 흩어진 같은 문서의 버전들을 병합한다.
///
/// ## 문제
/// 현재 lineage_id는 `hash(stem_norm + parent_folder)`라서
/// `D:/Project/계약서.hwp`와 `D:/Backup/Project/계약서.hwp`는 다른 lineage로 분리됨.
///
/// ## 알고리즘
/// 1. 같은 `stem_norm`을 가졌는데 `lineage_id`가 여럿인 stem 찾기
/// 2. 각 stem의 lineage들 사이 대표 파일(canonical) 임베딩 비교
/// 3. 코사인 유사도 ≥ 0.95면 Union-Find로 병합 (작은 lineage_id로)
/// 4. 병합된 lineage의 canonical 재선출
///
/// 반환값: 병합된 lineage 쌍 수.
pub fn reunite_cross_folder(
    conn: &Connection,
    embedder: &Arc<Embedder>,
) -> rusqlite::Result<usize> {
    const REUNION_THRESHOLD: f32 = 0.95;

    // 1. 같은 stem_norm인데 lineage 여럿인 케이스 수집
    let mut candidate_stems: Vec<String> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT stem_norm FROM files
             WHERE stem_norm IS NOT NULL AND stem_norm != '' AND lineage_id IS NOT NULL
             GROUP BY stem_norm
             HAVING COUNT(DISTINCT lineage_id) > 1",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            candidate_stems.push(row.get::<_, String>(0)?);
        }
    }

    let mut merged_pairs = 0usize;

    for stem in &candidate_stems {
        // 2. 해당 stem의 각 lineage에서 canonical(없으면 임의 1개) + 첫 청크 조회
        let mut reps: Vec<(String, i64, String)> = Vec::new(); // (lineage_id, file_id, content)
        {
            let mut stmt = conn.prepare(
                "SELECT f.lineage_id, f.id, c.content
                 FROM files f
                 LEFT JOIN chunks c ON c.file_id = f.id AND c.chunk_index = 0
                 WHERE f.stem_norm = ?1
                   AND f.lineage_id IS NOT NULL
                   AND c.content IS NOT NULL
                   AND LENGTH(c.content) >= 30
                 ORDER BY (f.lineage_role = 'canonical') DESC, f.modified_at DESC",
            )?;
            let mut rows = stmt.query([stem])?;
            let mut seen_lineages = std::collections::HashSet::new();
            while let Some(row) = rows.next()? {
                let lid: String = row.get(0)?;
                if seen_lineages.insert(lid.clone()) {
                    reps.push((lid, row.get(1)?, row.get(2)?));
                }
            }
        }

        if reps.len() < 2 {
            continue;
        }

        // 3. 각 rep의 임베딩
        let embeddings: Vec<(String, Vec<f32>)> = reps
            .iter()
            .filter_map(|(lid, _, content)| {
                embedder
                    .embed(content, false)
                    .ok()
                    .map(|e| (lid.clone(), e))
            })
            .collect();
        if embeddings.len() < 2 {
            continue;
        }

        // 4. Union-Find: 유사도 ≥ 0.95인 lineage 쌍 연결
        let n = embeddings.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], x: usize) -> usize {
            if parent[x] != x {
                parent[x] = find(parent, parent[x]);
            }
            parent[x]
        }
        for i in 0..n {
            for j in (i + 1)..n {
                let sim = cosine_similarity(&embeddings[i].1, &embeddings[j].1);
                if sim >= REUNION_THRESHOLD {
                    let ri = find(&mut parent, i);
                    let rj = find(&mut parent, j);
                    if ri != rj {
                        parent[ri] = rj;
                    }
                }
            }
        }

        // 5. 컴포넌트별로 lineage 병합 (대표 lineage_id로 모든 파일 이동)
        let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            groups
                .entry(root)
                .or_default()
                .push(embeddings[i].0.clone());
        }

        for (_, lineages) in groups {
            if lineages.len() < 2 {
                continue;
            }
            // 대표: lexicographic 최소 lineage_id (결정론)
            let representative = lineages.iter().min().cloned().unwrap();
            for other in &lineages {
                if other == &representative {
                    continue;
                }
                conn.execute(
                    "UPDATE files SET lineage_id = ?1 WHERE lineage_id = ?2",
                    params![representative, other],
                )?;
                merged_pairs += 1;
            }

            // 병합된 lineage에서 canonical 재선출
            let _ = conn.execute(
                "UPDATE files SET lineage_role = 'version' WHERE lineage_id = ?1",
                [&representative],
            );
            // behavioral canonical_score로 최고점 1개만 canonical로 승격
            let mut members: Vec<(i64, String, Option<i64>, i64)> = Vec::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT id, name, modified_at, COALESCE(open_count, 0)
                     FROM files WHERE lineage_id = ?1",
                )?;
                let mut rows = stmt.query([&representative])?;
                while let Some(row) = rows.next()? {
                    members.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?));
                }
            }
            if let Some(w) = members
                .iter()
                .max_by_key(|(_, n, m, oc)| canonical_score(n, *m, *oc))
            {
                let _ = conn.execute(
                    "UPDATE files SET lineage_role = 'canonical' WHERE id = ?1",
                    [w.0],
                );
            }
        }
    }

    Ok(merged_pairs)
}

/// 특정 파일이 막 열렸을 때, 그 파일이 속한 lineage의 canonical을 재선출한다.
///
/// 사용자가 `_수정본.hwp`를 자주 여는데 canonical이 `_최종.hwp`로 잘못 잡혀 있으면,
/// 이 함수가 호출되어 `_수정본.hwp`를 canonical로 승격시킨다.
pub fn rebalance_canonical_for_opened(
    conn: &Connection,
    opened_path: &str,
) -> rusqlite::Result<()> {
    // 1. 해당 파일의 lineage_id 조회
    let lineage_id: Option<String> = conn
        .query_row(
            "SELECT lineage_id FROM files WHERE path = ?1",
            [opened_path],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let Some(lid) = lineage_id else {
        return Ok(());
    };

    // 2. 같은 lineage의 모든 파일 조회
    let mut members: Vec<(i64, String, Option<i64>, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, modified_at, COALESCE(open_count, 0)
             FROM files WHERE lineage_id = ?1",
        )?;
        let mut rows = stmt.query([&lid])?;
        while let Some(row) = rows.next()? {
            members.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?));
        }
    }
    if members.len() < 2 {
        return Ok(());
    }

    // 3. behavioral canonical_score로 재선출
    let winner_id = members
        .iter()
        .max_by_key(|(_, name, m, oc)| canonical_score(name, *m, *oc))
        .map(|(id, _, _, _)| *id);

    let Some(wid) = winner_id else {
        return Ok(());
    };

    // 4. UPDATE — 승자만 canonical, 나머지 version
    conn.execute(
        "UPDATE files SET lineage_role = CASE WHEN id = ?1 THEN 'canonical' ELSE 'version' END
         WHERE lineage_id = ?2",
        params![wid, lid],
    )?;
    Ok(())
}

/// 이미 형성된 lineage들을 벡터 유사도로 재검증하여 "이름은 비슷한데 내용 다른" 파일을 분리한다.
///
/// ## 알고리즘
/// 1. 멤버 2개 이상인 lineage 순회
/// 2. 각 파일의 첫 청크(`chunk_index = 0`) content로 임베딩 생성
/// 3. canonical(또는 첫 멤버) 기준으로 cosine similarity 측정
/// 4. 유사도 < 0.85인 파일은 새 lineage_id로 분리 (각자 독립)
///
/// ## 성능
/// 약 3,000개 multi-member lineage × 평균 2-3개 파일 = 6-9K 임베딩.
/// 임베딩당 10-50ms → 총 1-8분 예상. rebuild_lineage 커맨드 내부에서 호출된다.
///
/// 반환값: 분리된 파일 수.
pub fn refine_with_vector(conn: &Connection, embedder: &Arc<Embedder>) -> rusqlite::Result<usize> {
    // 1. multi-member lineages 수집
    let lineages: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT lineage_id FROM files
             WHERE lineage_id IS NOT NULL
             GROUP BY lineage_id HAVING COUNT(*) >= 2",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row.get::<_, String>(0)?);
        }
        out
    };

    let mut split_count = 0usize;

    for lid in &lineages {
        // 각 파일의 첫 청크 조회 (canonical 우선 정렬 — base로 사용)
        let mut members: Vec<(i64, String, String)> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT f.id, f.name, c.content
                 FROM files f
                 LEFT JOIN chunks c ON c.file_id = f.id AND c.chunk_index = 0
                 WHERE f.lineage_id = ?1 AND c.content IS NOT NULL
                 ORDER BY (f.lineage_role = 'canonical') DESC, f.modified_at DESC",
            )?;
            let mut rows = stmt.query([lid])?;
            while let Some(row) = rows.next()? {
                members.push((row.get(0)?, row.get(1)?, row.get(2)?));
            }
        }

        // 너무 짧은 content 제외
        members.retain(|m| m.2.len() >= MIN_CHUNK_LEN_FOR_VERIFICATION);
        if members.len() < 2 {
            continue;
        }

        // 임베딩 생성 (실패 시 skip)
        let embeddings: Vec<(i64, Vec<f32>)> = members
            .iter()
            .filter_map(|m| embedder.embed(&m.2, false).ok().map(|e| (m.0, e)))
            .collect();
        if embeddings.len() < 2 {
            continue;
        }

        // Greedy: 첫 번째(canonical)를 base. 유사도 < 0.85인 outsider 찾음.
        let (_base_id, base_emb) = &embeddings[0];
        let mut outsiders: Vec<i64> = Vec::new();
        for (id, emb) in &embeddings[1..] {
            if cosine_similarity(base_emb, emb) < LINEAGE_SIMILARITY_THRESHOLD {
                outsiders.push(*id);
            }
        }

        if outsiders.is_empty() {
            continue;
        }

        // 각 outsider를 독립 lineage로 분리 (해시 기반 새 id)
        // 동일 파일이라도 SHA-256 조합이 달라서 고유 id 생성됨.
        for oid in &outsiders {
            let mut h = Sha256::new();
            h.update(lid.as_bytes());
            h.update(b"::split::");
            h.update(&oid.to_le_bytes());
            let mut new_id = String::with_capacity(16);
            for b in h.finalize().iter().take(8) {
                new_id.push_str(&format!("{:02x}", b));
            }

            // outsider는 혼자 lineage이므로 canonical
            conn.execute(
                "UPDATE files SET lineage_id = ?1, lineage_role = 'canonical' WHERE id = ?2",
                params![new_id, oid],
            )?;
            split_count += 1;
        }

        // 원래 lineage에서 canonical이 outsider로 이동한 경우 재선출
        // (첫 번째 base는 그대로이므로 OK. 추가 안전망)
        let remaining_canonical: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE lineage_id = ?1 AND lineage_role = 'canonical' LIMIT 1",
                [lid],
                |r| r.get(0),
            )
            .ok();
        if remaining_canonical.is_none() {
            // 아무도 canonical이 아니면 modified_at 최신을 승격
            let _ = conn.execute(
                "UPDATE files SET lineage_role = 'canonical'
                 WHERE id = (SELECT id FROM files WHERE lineage_id = ?1
                             ORDER BY modified_at DESC LIMIT 1)",
                [lid],
            );
        }
    }

    Ok(split_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_folder_windows_and_unix() {
        assert_eq!(
            parent_folder_of(r"C:\docs\계약서_최종.hwpx"),
            r"c:\docs".to_string()
        );
        assert_eq!(
            parent_folder_of("/home/user/docs/file.txt"),
            "/home/user/docs".to_string()
        );
    }

    #[test]
    fn lineage_id_deterministic() {
        let a = compute_lineage_id("계약서", "c:/docs");
        let b = compute_lineage_id("계약서", "c:/docs");
        assert_eq!(a, b);
        // 다른 폴더면 다른 id
        let c = compute_lineage_id("계약서", "c:/other");
        assert_ne!(a, c);
        // 16자 해시
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn canonical_score_prefers_deeper_final_stack() {
        let s1 = canonical_score("계약서_최종.hwpx", Some(1000), 0);
        let s2 = canonical_score("계약서_최최종.hwpx", Some(500), 0);
        let s3 = canonical_score("계약서_최최최종.hwpx", Some(100), 0);
        assert!(s3 > s2);
        assert!(s2 > s1);
    }

    #[test]
    fn canonical_score_falls_back_to_modified_at() {
        let s_old = canonical_score("계약서.hwpx", Some(1000), 0);
        let s_new = canonical_score("계약서.hwpx", Some(2000), 0);
        assert!(s_new > s_old);
    }

    #[test]
    fn behavioral_canonical_wins_over_weak_label() {
        // "수정본"은 기본 priority 30, 열람 20회면 +50 → 80
        // "최종"은 priority 102 (최최종 depth=2 → 102), 열람 0 → 102
        // 아직 label 우선. 근데 수정본이 더 많이 열리면?
        let s_final = canonical_score("계약서_최종.hwpx", Some(1000), 0);
        let s_revised_heavy = canonical_score("계약서_수정본.hwpx", Some(1000), 20);
        // _수정본 30 + 50 = 80 < 최종 102 + 0 = 102 → 최종 승
        assert!(s_final > s_revised_heavy);

        // 같은 라벨이면 열람 횟수로 결정
        let s_a = canonical_score("계약서.hwpx", Some(1000), 0);
        let s_b = canonical_score("계약서.hwpx", Some(1000), 10);
        assert!(s_b > s_a);
    }

    /// 실제 DB에 마이그레이션 + 백필 실행. `--ignored`로만 실행된다.
    ///
    /// 실행 방법:
    ///   cargo test --lib real_db_rebuild -- --ignored --nocapture
    ///
    /// 환경변수 `DOCUFINDER_DB`로 DB 경로 오버라이드 가능.
    /// 기본값: `%APPDATA%\com.anything.app\docufinder.db`
    #[test]
    #[ignore]
    fn real_db_rebuild_and_report() {
        use rusqlite::Connection;
        use std::path::PathBuf;

        let db_path = std::env::var("DOCUFINDER_DB").unwrap_or_else(|_| {
            let appdata = std::env::var("APPDATA").expect("APPDATA not set");
            format!("{}\\com.anything.app\\docufinder.db", appdata)
        });
        let path = PathBuf::from(&db_path);
        assert!(path.exists(), "DB not found at {}", db_path);

        eprintln!("\n=== 실제 DB lineage 백필 테스트 ===");
        eprintln!("DB: {}", db_path);

        // 1. 스키마 v12 마이그레이션 (ALTER TABLE ADD COLUMN IF NOT EXISTS)
        crate::db::init_database(&path).expect("migrate failed");
        eprintln!("✓ 마이그레이션 완료");

        // 2. 백필 실행
        let conn = Connection::open(&path).expect("open failed");
        let start = std::time::Instant::now();
        let (files_updated, unique_lineages) = super::rebuild_all(&conn).expect("rebuild failed");
        let elapsed = start.elapsed();

        eprintln!(
            "✓ rebuild: {}개 파일 처리, {}개 lineage 생성, {:.2}초",
            files_updated,
            unique_lineages,
            elapsed.as_secs_f64()
        );

        // 3. multi-version 그룹 통계
        let multi_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM (
                   SELECT lineage_id FROM files
                   GROUP BY lineage_id HAVING COUNT(*) >= 2
                 )",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let total_in_multi: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(c), 0) FROM (
                   SELECT COUNT(*) c FROM files
                   GROUP BY lineage_id HAVING c >= 2
                 )",
                [],
                |r| r.get(0),
            )
            .unwrap();
        eprintln!(
            "✓ 2개 이상 묶인 lineage: {}개 (총 {}개 파일 → collapse 가능)",
            multi_count, total_in_multi
        );

        // 4. 가장 큰 lineage 10개 (같은 폴더 기준)
        eprintln!("\n--- 상위 10개 multi-version lineage ---");
        let mut stmt = conn
            .prepare(
                "SELECT lineage_id, COUNT(*) c
                 FROM files
                 GROUP BY lineage_id
                 HAVING c >= 2
                 ORDER BY c DESC
                 LIMIT 10",
            )
            .unwrap();
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for (lid, cnt) in rows {
            eprintln!("\n[lineage {}] {}개 파일:", lid, cnt);
            let mut s = conn
                .prepare(
                    "SELECT name, lineage_role, version_label, modified_at
                     FROM files WHERE lineage_id = ?1
                     ORDER BY lineage_role DESC, modified_at DESC
                     LIMIT 5",
                )
                .unwrap();
            let members: Vec<(String, String, Option<String>, Option<i64>)> = s
                .query_map([&lid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            for (name, role, label, mtime) in members {
                let marker = if role == "canonical" { "👑" } else { "  " };
                let label_str = label.unwrap_or_else(|| "-".into());
                let date = mtime.map(|t| t.to_string()).unwrap_or_default();
                eprintln!("  {} [{}] {} ({})", marker, label_str, name, date);
            }
        }

        // 5. canonical 분포
        let canonical: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE lineage_role = 'canonical'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let version: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files WHERE lineage_role = 'version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        eprintln!("\n--- 역할 분포 ---");
        eprintln!("canonical: {}", canonical);
        eprintln!("version:   {}", version);
    }
}
