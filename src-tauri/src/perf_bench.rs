//! 성능 계측 하니스 — `#[cfg(test)]` 이므로 릴리스 바이너리에는 포함되지 않는다.
//!
//! 실행:
//! ```sh
//! source ~/.cargo/env
//! cargo test --release --manifest-path src-tauri/Cargo.toml --lib perf_ -- --nocapture --test-threads=1
//! ```
//!
//! 최적화 전후로 같은 명령을 돌려 min/median 을 비교한다. 마이크로벤치라
//! 모델·동시성·메모리 성격의 최적화(임베딩 배치, spawn_blocking, resume 피크)는
//! 여기서 잡히지 않는다 — 토큰화·파서 등 CPU 경로 전용이다.

#[cfg(test)]
mod perf {
    use crate::tokenizer::{LinderaKoTokenizer, TextTokenizer};
    use std::hint::black_box;
    use std::time::Instant;

    /// warmup 후 iters 회 측정해 min/median/mean(ms) 을 출력한다.
    fn measure(name: &str, warmup: u32, iters: u32, mut f: impl FnMut()) {
        for _ in 0..warmup {
            f();
        }
        let mut samples = Vec::with_capacity(iters as usize);
        for _ in 0..iters {
            let t = Instant::now();
            f();
            samples.push(t.elapsed().as_nanos());
        }
        samples.sort_unstable();
        let min = samples[0] as f64 / 1e6;
        let median = samples[iters as usize / 2] as f64 / 1e6;
        let mean = samples.iter().sum::<u128>() as f64 / iters as f64 / 1e6;
        eprintln!(
            "[perf] {name:<32} min={min:>9.4}ms  median={median:>9.4}ms  mean={mean:>9.4}ms  (n={iters})"
        );
    }

    /// 실제 공문/정부문서와 유사한 한국어 본문. `repeat` 로 청크/문서 크기를 조절한다.
    /// 명사 중복이 섞여 dedup(contains 스캔) 경로가 실제처럼 동작한다.
    fn korean_text(repeat: usize) -> String {
        const PARA: &str = "고용노동부는 2024년 상반기 사업장 지도점검 결과를 발표하였다. \
            이번 점검은 산업안전보건법 및 근로기준법 위반 여부를 중심으로 진행되었으며, \
            전국 사업장 가운데 제조업과 건설업 분야를 우선 대상으로 선정하였다. \
            점검 결과 임금체불 사업장에 대해서는 시정지시를 내리고 개선계획서를 제출받았다. \
            안전보건관리책임자 미선임 사업장은 과태료 부과 대상에 포함되었으며, \
            재해예방 조치가 미흡한 경우 작업중지 명령을 병행하였다. ";
        PARA.repeat(repeat)
    }

    #[test]
    fn perf_tokenize_chunk() {
        let tok = LinderaKoTokenizer::new().expect("tokenizer init");
        // 인덱싱은 청크(~600자)마다 tokenize 호출 — repeat=3 이 실제 청크 크기에 근접
        let chunk = korean_text(3);
        measure("tokenize 청크(~600자)", 30, 300, || {
            black_box(tok.tokenize(black_box(&chunk)));
        });
    }

    #[test]
    fn perf_tokenize_large() {
        // O(n^2) dedup 확대 관찰용: 큰 텍스트를 한 번에 tokenize (contains 스캔 폭증 구간)
        let tok = LinderaKoTokenizer::new().expect("tokenizer init");
        let big = korean_text(20);
        measure("tokenize 대형(~3800자)", 20, 100, || {
            black_box(tok.tokenize(black_box(&big)));
        });
    }

    #[test]
    fn perf_extract_nouns_doc() {
        // extract_nouns 는 문서 전체 대상 — 토큰 수가 많아 O(n^2) dedup 이 두드러진다
        let tok = LinderaKoTokenizer::new().expect("tokenizer init");
        let doc = korean_text(20);
        measure("extract_nouns 문서(~3800자)", 20, 100, || {
            black_box(tok.extract_nouns(black_box(&doc)));
        });
    }
}
