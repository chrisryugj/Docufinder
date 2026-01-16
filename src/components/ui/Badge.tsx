import { ReactNode } from "react";

type BadgeVariant =
  | "default"
  | "primary"
  | "success"
  | "warning"
  | "danger"
  | "hwpx"
  | "docx"
  | "xlsx"
  | "pdf"
  | "txt";

interface BadgeProps {
  variant?: BadgeVariant;
  children: ReactNode;
  className?: string;
}

const variantStyles: Record<BadgeVariant, string> = {
  default: "bg-gray-700 text-gray-300",
  primary: "bg-blue-600/30 text-blue-300",
  success: "bg-green-600/30 text-green-300",
  warning: "bg-yellow-600/30 text-yellow-300",
  danger: "bg-red-600/30 text-red-300",
  // 파일 타입별 색상
  hwpx: "bg-emerald-600/30 text-emerald-300",
  docx: "bg-blue-600/30 text-blue-300",
  xlsx: "bg-green-600/30 text-green-300",
  pdf: "bg-red-600/30 text-red-300",
  txt: "bg-gray-600/30 text-gray-300",
};

export function Badge({
  variant = "default",
  children,
  className = "",
}: BadgeProps) {
  return (
    <span
      className={`
        inline-flex items-center px-2 py-0.5 rounded text-xs font-medium
        ${variantStyles[variant]}
        ${className}
      `}
    >
      {children}
    </span>
  );
}

// 파일 확장자에서 Badge variant 추출
export function getFileTypeBadgeVariant(
  fileName: string
): BadgeVariant {
  const ext = fileName.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "hwpx":
      return "hwpx";
    case "docx":
    case "doc":
      return "docx";
    case "xlsx":
    case "xls":
      return "xlsx";
    case "pdf":
      return "pdf";
    case "txt":
    case "md":
      return "txt";
    default:
      return "default";
  }
}
