/**
 * Unix timestamp를 상대 시간 문자열로 변환
 * 예: "방금", "5분 전", "3시간 전", "어제", "3일 전"
 * compact: "방금", "5분전", "3시간전", "어제", "3/5", "25/3/5"
 */
export function formatRelativeTime(timestamp: number, compact = false): string {
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
    return compact ? `${minutes}m` : `${minutes}분 전`;
  }

  if (hours < 24) {
    return compact ? `${hours}h` : `${hours}시간 전`;
  }

  if (days === 1) {
    return compact ? "1d" : "어제";
  }

  if (days < 7) {
    return compact ? `${days}d` : `${days}일 전`;
  }

  // 7일 이상은 날짜 표시 (연도 항상 포함)
  const date = new Date(timestamp);
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const year = date.getFullYear();
  return compact ? `${year % 100}/${month}/${day}` : `${year}. ${month}. ${day}`;
}
