/**
 * 빌드 flavor 플래그 — 프론트에서 lite(내부망) 배포판을 구분하는 유일한 출처.
 *
 * `pnpm build:lite` 가 `VITE_LITE=1` 을 넣고, 그 프론트는 `--no-default-features` 로 빌드한
 * Rust 바이너리와 짝을 이룬다. lite 백엔드에는 LLM·모델 다운로드·업데이터 경로가 아예
 * 컴파일돼 있지 않으므로, 관련 UI 를 노출하면 누르는 족족 실패하는 버튼만 남는다.
 *
 * 리터럴 비교라 Vite 가 빌드 시점에 상수로 접어 죽은 분기를 트리셰이킹한다.
 * 배경과 제외 목록은 docs/LITE-BUILD.md 참고.
 */
export const IS_LITE = import.meta.env.VITE_LITE === "1";
