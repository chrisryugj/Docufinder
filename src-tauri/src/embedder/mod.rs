//! 텍스트 임베딩 모듈 (KoSimCSE-roberta-multitask ONNX)

use ort::session::Session;
use ort::value::Value;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use thiserror::Error;
use tokenizers::Tokenizer;

pub const EMBEDDING_DIM: usize = 768;
const MAX_LENGTH: usize = 512;
/// 배치 임베딩 서브배치 크기.
/// 인덱싱 배치(32건)가 Session Mutex를 통째로 점유하면 쿼리 임베딩(1건)이
/// 배치 전체(~400ms)를 기다린다 → 서브배치 단위로 락을 잡았다 놓아
/// 보유 시간을 추론 1회(~100ms)로 제한하고, 사이에 쿼리가 끼어들 수 있게 한다.
const SUB_BATCH_SIZE: usize = 8;

#[derive(Error, Debug)]
pub enum EmbedderError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    #[error("ONNX Runtime error: {0}")]
    OrtError(String),

    #[error("Invalid embedding dimension")]
    InvalidDimension,
}

impl From<ort::Error> for EmbedderError {
    fn from(e: ort::Error) -> Self {
        EmbedderError::OrtError(e.to_string())
    }
}

/// 텍스트 임베딩 생성기
///
/// Session은 &mut self를 필요로 하므로 내부 Mutex 사용
/// 토큰화는 병렬 가능, ONNX 추론만 직렬화
pub struct Embedder {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    /// Session 락 대기 중인 쿼리(레이턴시 민감) 수.
    /// 배치 임베딩이 서브배치 사이에 이 값을 보고 양보한다.
    query_waiting: AtomicUsize,
}

/// query_waiting 카운터 RAII 가드 — 에러/패닉 경로에서도 감소 보장
struct QueryWaitGuard<'a>(&'a AtomicUsize);

impl Drop for QueryWaitGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

impl Embedder {
    /// 새 Embedder 생성
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self, EmbedderError> {
        // 모델 파일 확인
        if !model_path.exists() {
            return Err(EmbedderError::ModelNotFound(
                model_path.to_string_lossy().to_string(),
            ));
        }

        if !tokenizer_path.exists() {
            return Err(EmbedderError::ModelNotFound(
                tokenizer_path.to_string_lossy().to_string(),
            ));
        }

        // 동적 스레드 수 감지 (최소 2, 최대 4 — 다른 워커와 경합 방지)
        let num_threads = std::thread::available_parallelism()
            .map(|p| p.get().clamp(2, 4))
            .unwrap_or(2);

        tracing::debug!("Embedder using {} intra-op threads", num_threads);

        // ONNX 세션 생성 (최적화 적용)
        // - CPU EP arena 비활성화: 선점 할당 대신 호출별 할당으로 전환 (RAM 50-100MB 절감)
        // - parallel_execution 제거: 단일 쿼리에 inter-op 병렬 불필요, intra_threads로 충분
        let session = Session::builder()?
            .with_execution_providers([ort::ep::CPU::default().build()])?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)?
            .with_intra_threads(num_threads)?
            .commit_from_file(model_path)?;

        // Tokenizer 로드
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            query_waiting: AtomicUsize::new(0),
        })
    }

    /// 단일 텍스트 임베딩
    pub fn embed(&self, text: &str, is_query: bool) -> Result<Vec<f32>, EmbedderError> {
        // 쿼리는 레이턴시 민감 — 대기 카운터를 올려 백그라운드 배치 임베딩이
        // 서브배치 사이에 양보(락 해제 유지)하도록 신호한다
        let _guard = is_query.then(|| {
            self.query_waiting.fetch_add(1, Ordering::Release);
            QueryWaitGuard(&self.query_waiting)
        });
        let embeddings = self.embed_batch(&[self.prepare_text(text, is_query)])?;
        embeddings
            .into_iter()
            .next()
            .ok_or(EmbedderError::InvalidDimension)
    }

    /// 배치 임베딩 (불변 참조 - 락 없이 병렬 호출 가능)
    ///
    /// SUB_BATCH_SIZE 단위로 나눠 추론한다: Session Mutex 보유 시간이 서브배치
    /// 1회로 제한되어, 백그라운드 벡터 인덱싱(32건) 중에도 쿼리 임베딩(1건)이
    /// 락 사이에 끼어들 수 있다.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        if texts.len() <= SUB_BATCH_SIZE {
            return self.run_inference(texts);
        }

        let mut embeddings = Vec::with_capacity(texts.len());
        for sub in texts.chunks(SUB_BATCH_SIZE) {
            embeddings.extend(self.run_inference(sub)?);
            // 쿼리 대기 중이면 1ms 양보 — std Mutex는 불공정해서 yield만으로는
            // 같은 스레드가 락을 재획득할 수 있으므로 sleep으로 확실히 넘겨준다
            if self.query_waiting.load(Ordering::Acquire) > 0 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        Ok(embeddings)
    }

    /// 단일 서브배치 추론 — 호출 동안 Session Mutex 보유
    fn run_inference(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        // 토큰화
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| EmbedderError::TokenizerError(e.to_string()))?;

        let batch_size = encodings.len();
        let seq_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(MAX_LENGTH))
            .max()
            .unwrap_or(0);

        // 입력 텐서 생성 (Array2 중간 복사 제거 - 직접 Vec 구축)
        let total = batch_size * seq_len;
        let mut input_ids_vec = vec![0i64; total];
        let mut attention_mask_vec = vec![0i64; total];

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(seq_len);
            let offset = i * seq_len;

            for j in 0..len {
                input_ids_vec[offset + j] = ids[j] as i64;
                attention_mask_vec[offset + j] = mask[j] as i64;
            }
        }

        let shape = [batch_size as i64, seq_len as i64];

        // ONNX 추론 (Session은 &mut self 필요 → Mutex 사용)
        // KoSimCSE-roberta INT8 모델은 input_ids, attention_mask 2개 입력만 필요
        let input_ids_value = Value::from_array((shape, input_ids_vec))?;
        // attention_mask_vec는 mean pooling에서 재사용 → clone 후 텐서에 전달
        let attention_mask_value = Value::from_array((shape, attention_mask_vec.clone()))?;

        let embeddings = {
            // Poison recovery: ONNX Session은 stateless (입력→출력)이므로 이전 panic이 내부 상태를 오염시키지 않음
            let mut session = self.session.lock().unwrap_or_else(|poisoned| {
                tracing::warn!("Embedder ONNX session mutex was poisoned, recovering inner value");
                poisoned.into_inner()
            });

            // 먼저 출력 이름들 수집 (borrow 충돌 방지)
            let output_names: Vec<String> = session
                .outputs()
                .iter()
                .map(|o| o.name().to_string())
                .collect();

            let outputs = session.run(ort::inputs![
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value,
            ])?;

            // 출력에서 임베딩 추출 (모델에 따라 출력 이름이 다를 수 있음)
            let output = outputs
                .get("last_hidden_state")
                .or_else(|| outputs.get("output"))
                .or_else(|| outputs.get("sentence_embedding"))
                .or_else(|| outputs.get("token_embeddings"))
                .or_else(|| {
                    // 첫 번째 출력 사용 (fallback)
                    output_names
                        .first()
                        .and_then(|name| outputs.get(name.as_str()))
                })
                .ok_or_else(|| {
                    EmbedderError::OrtError(format!(
                        "No embedding output found. Available: {:?}",
                        output_names
                    ))
                })?;

            let (out_shape, out_data) = output.try_extract_tensor::<f32>()?;

            let dims = out_shape.len();

            if dims == 2 {
                // 2D: [batch, hidden_dim] - 이미 pooling된 sentence embedding
                let hidden_dim = out_shape
                    .get(1)
                    .map(|&d| d as usize)
                    .unwrap_or(EMBEDDING_DIM);
                let mut embeddings = Vec::with_capacity(batch_size);

                for i in 0..batch_size {
                    let mut emb = vec![0.0f32; EMBEDDING_DIM];
                    let offset = i * hidden_dim;
                    for k in 0..EMBEDDING_DIM.min(hidden_dim) {
                        if offset + k < out_data.len() {
                            emb[k] = out_data[offset + k];
                        }
                    }
                    // L2 normalize
                    let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > 0.0 {
                        for v in &mut emb {
                            *v /= norm;
                        }
                    }
                    embeddings.push(emb);
                }
                embeddings
            } else {
                // 3D: [batch, seq_len, hidden_dim] - mean pooling 필요
                let model_seq_len = out_shape.get(1).map(|&d| d as usize).unwrap_or(seq_len);
                let hidden_dim = out_shape
                    .get(2)
                    .map(|&d| d as usize)
                    .unwrap_or(EMBEDDING_DIM);

                let mut embeddings = Vec::with_capacity(batch_size);
                for i in 0..batch_size {
                    let mut sum = vec![0.0f32; EMBEDDING_DIM];
                    let mut count = 0.0f32;

                    for j in 0..model_seq_len.min(seq_len) {
                        if j < seq_len && attention_mask_vec[i * seq_len + j] == 1 {
                            let offset = i * model_seq_len * hidden_dim + j * hidden_dim;
                            for k in 0..EMBEDDING_DIM.min(hidden_dim) {
                                if offset + k < out_data.len() {
                                    sum[k] += out_data[offset + k];
                                }
                            }
                            count += 1.0;
                        }
                    }

                    // Average
                    if count > 0.0 {
                        for v in &mut sum {
                            *v /= count;
                        }
                    }

                    // L2 normalize
                    let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm > 0.0 {
                        for v in &mut sum {
                            *v /= norm;
                        }
                    }

                    embeddings.push(sum);
                }
                embeddings
            }
        };

        Ok(embeddings)
    }

    /// 텍스트 전처리 (KoSimCSE는 접두사 불필요).
    /// 인덱싱 청크(parse_file→normalize_text)와 쿼리 임베딩이 동일 정규형을 보도록
    /// 정규화한다. 이미 정규화된 청크엔 idempotent(NFC 빠른 경로), 쿼리만 실질 변환.
    fn prepare_text(&self, text: &str, _is_query: bool) -> String {
        crate::utils::normalize_text(text)
    }
}

// SAFETY: ort 2.0+ Session은 내부적으로 thread-safe (Session::run이 &self 사용).
// - Session: Mutex<Session>으로 감싸서 동시 접근 직렬화 → &self에서 mutation 불가
// - Tokenizer: tokenizers 크레이트에서 Send+Sync 이미 구현
// - 실행 프로바이더: CPU EP만 사용 (CUDA/DirectML 미사용 → thread-affinity 문제 없음)
// - ort 버전: =2.0.0-rc.11 (정식 릴리스 시 unsafe 제거 가능 여부 재검토 필요)
// 참조: https://github.com/pykeio/ort - Session is thread-safe in ort 2.0+
unsafe impl Send for Embedder {}
unsafe impl Sync for Embedder {}
