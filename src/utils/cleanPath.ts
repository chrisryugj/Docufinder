/**
 * Windows extended-length prefix 제거 — 표시·복사·탐색기 호출 전 공통 정규화.
 *   \\?\E:\folder\file.txt          → E:\folder\file.txt
 *   \\?\UNC\server\share\file.txt   → \\server\share\file.txt
 *
 * UNC verbatim 은 `\\?\` 만 떼면 `UNC\server\share\…` 라는 깨진 경로가 되어 파일 열기·위치
 * 열기·경로 복사가 전부 실패한다(이슈 #46). 백엔드도 같은 수정부터 `\\server\share\…` 로 저장하지만
 * 구버전 DB·기타 경로 원천이 남아 있으므로 프론트도 여기 한 곳에서 같은 규칙으로 방어한다.
 * 슬래시 변형(`//?/`)은 formatPathTail 이 다루던 입력을 그대로 흡수한다.
 */
export function cleanPath(path: string): string {
  return path
    .replace(/^\\\\\?\\UNC\\/i, "\\\\")
    .replace(/^\\\\\?\\/, "")
    .replace(/^\/\/\?\/UNC\//i, "//")
    .replace(/^\/\/\?\//, "");
}
