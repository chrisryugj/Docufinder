import type { IndexStatus } from "../../types/index";

interface StatusBarProps {
  status: IndexStatus | null;
}

export function StatusBar({ status }: StatusBarProps) {
  return (
    <footer className="border-t border-gray-700 bg-gray-800 px-4 py-2">
      <div className="flex justify-between text-sm text-gray-400">
        <span>인덱싱된 문서: {status?.total_files ?? 0}개</span>
        <span>
          {status?.watched_folders.length
            ? `폴더: ${status.watched_folders.length}개`
            : "폴더를 추가하세요"}
        </span>
      </div>
    </footer>
  );
}
