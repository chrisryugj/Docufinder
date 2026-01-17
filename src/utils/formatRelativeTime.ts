/**
 * Unix timestamp를 상대 시간 문자열로 변환
 * 예: "방금", "5분 전", "3시간 전", "어제", "3일 전"
 */
export function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;

  // 밀리초 → 분/시간/일 변환
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) {
    return "방금";
  }

  if (minutes < 60) {
    return `${minutes}분 전`;
  }

  if (hours < 24) {
    return `${hours}시간 전`;
  }

  if (days === 1) {
    return "어제";
  }

  if (days < 7) {
    return `${days}일 전`;
  }

  // 7일 이상은 날짜 표시
  const date = new Date(timestamp);
  const month = date.getMonth() + 1;
  const day = date.getDate();

  // 올해면 월/일만, 아니면 년도 포함
  const thisYear = new Date().getFullYear();
  if (date.getFullYear() === thisYear) {
    return `${month}월 ${day}일`;
  }

  return `${date.getFullYear()}. ${month}. ${day}`;
}
