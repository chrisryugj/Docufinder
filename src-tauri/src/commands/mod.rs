// AI / 수식모델 커맨드는 빌드 flavor 에 따라 실체가 갈린다.
// online(일반 배포): 실제 LLM 호출 · kordoc 모델 다운로드.
// lite(내부망 배포): 이름만 같은 스텁 — 커맨드 목록을 두 빌드가 공유해
//                    `generate_handler!` 를 한 벌로 유지한다. docs/LITE-BUILD.md 참고.
#[cfg(feature = "online")]
pub mod ai;
#[cfg(not(feature = "online"))]
#[path = "ai_lite.rs"]
pub mod ai;

pub mod duplicate;
pub mod export;
pub mod file;

#[cfg(feature = "online")]
pub mod formula;
#[cfg(not(feature = "online"))]
#[path = "formula_lite.rs"]
pub mod formula;

pub mod index;
pub mod lineage;
pub mod maintenance;
pub mod preview;
pub mod search;
pub mod settings;
pub mod system;
pub mod tags;
pub mod telemetry;
pub mod typo;
