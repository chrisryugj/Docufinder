/**
 * 런타임 OS 감지 — 같은 빌드 코드를 Windows 와 macOS 에서 모두 쓰기 위해
 * 사용자 노출 텍스트(파일 매니저 이름, 시스템 폴더 예시 등) 를 분기한다.
 * Tauri 의 OS plugin 대신 navigator.userAgent 만 보고 판단 — capability 추가 불필요.
 */
const ua = typeof navigator !== "undefined" ? navigator.userAgent : "";

export const isMac = /Mac/i.test(ua);
export const isWindows = !isMac && /Win/i.test(ua);

/** 파일 매니저 이름 (탐색기 / Finder) */
export const FILE_MANAGER_NAME = isMac ? "Finder" : "탐색기";

/** "{매니저}에서 열기" 라벨 */
export const REVEAL_LABEL = `${FILE_MANAGER_NAME}에서 열기`;

/** 시스템 폴더 예시 안내 (드라이브 인덱싱 / 검색 제외 폴더 등) */
export const SYSTEM_FOLDERS_HINT = isMac
  ? "/System, /Library, /private 등"
  : "Windows, Program Files, AppData 등";

/** 자동 실행 설명 */
export const AUTOSTART_DESCRIPTION = isMac
  ? "macOS 로그인 시 자동 실행"
  : "Windows 시작 시 자동 실행";

/** 드라이브 개념이 있는 OS 인지 (전체 드라이브 인덱싱 / 드라이브 루트 추가 등) */
export const HAS_DRIVES = isWindows;

/** 데이터 저장 경로의 기본 위치 안내 */
export const DEFAULT_DATA_LOCATION = isMac
  ? "기본 위치 (Application Support)"
  : "기본 위치 (AppData)";
