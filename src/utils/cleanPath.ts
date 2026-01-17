/**
 * Windows Long Path prefix (\\?\) 제거
 * 예: \\?\E:\folder\file.txt → E:\folder\file.txt
 */
export function cleanPath(path: string): string {
  return path.replace(/^\\\\\?\\/, '');
}
