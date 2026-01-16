interface FileIconProps {
  fileName: string;
  className?: string;
  size?: "sm" | "md" | "lg";
}

const sizeMap = {
  sm: "w-4 h-4",
  md: "w-5 h-5",
  lg: "w-6 h-6",
};

// 파일 타입별 색상
const colorMap: Record<string, string> = {
  hwpx: "text-emerald-400",
  docx: "text-blue-400",
  doc: "text-blue-400",
  xlsx: "text-green-400",
  xls: "text-green-400",
  pdf: "text-red-400",
  txt: "text-gray-400",
  md: "text-gray-400",
};

export function FileIcon({
  fileName,
  className = "",
  size = "md",
}: FileIconProps) {
  const ext = fileName.split(".").pop()?.toLowerCase() || "";
  const color = colorMap[ext] || "text-gray-400";

  return (
    <svg
      className={`${sizeMap[size]} ${color} flex-shrink-0 ${className}`}
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
      aria-hidden="true"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
      />
    </svg>
  );
}

// 파일 타입 라벨
export function getFileTypeLabel(fileName: string): string {
  const ext = fileName.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "hwpx":
      return "한글";
    case "docx":
    case "doc":
      return "워드";
    case "xlsx":
    case "xls":
      return "엑셀";
    case "pdf":
      return "PDF";
    case "txt":
      return "텍스트";
    case "md":
      return "마크다운";
    default:
      return ext?.toUpperCase() || "파일";
  }
}
