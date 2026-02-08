import { memo, ReactNode, CSSProperties } from "react";

type BadgeVariant =
  | "default"
  | "primary"
  | "secondary"
  | "success"
  | "warning"
  | "danger"
  | "keyword"
  | "semantic"
  | "hybrid"
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

// CSS 변수 기반 스타일 반환
const getVariantStyle = (variant: BadgeVariant): CSSProperties => {
  switch (variant) {
    case "hwpx":
      return {
        backgroundColor: "var(--color-file-hwpx-bg)",
        color: "var(--color-file-hwpx)",
      };
    case "docx":
      return {
        backgroundColor: "var(--color-file-docx-bg)",
        color: "var(--color-file-docx)",
      };
    case "xlsx":
      return {
        backgroundColor: "var(--color-file-xlsx-bg)",
        color: "var(--color-file-xlsx)",
      };
    case "pdf":
      return {
        backgroundColor: "var(--color-file-pdf-bg)",
        color: "var(--color-file-pdf)",
      };
    case "txt":
      return {
        backgroundColor: "var(--color-file-txt-bg)",
        color: "var(--color-file-txt)",
      };
    case "primary":
      return {
        backgroundColor: "var(--color-accent-light)",
        color: "var(--color-accent)",
      };
    case "secondary":
      return {
        backgroundColor: "var(--color-bg-tertiary)",
        color: "var(--color-text-secondary)",
      };
    case "success":
      return {
        backgroundColor: "rgba(34, 197, 94, 0.15)",
        color: "var(--color-success)",
      };
    case "warning":
      return {
        backgroundColor: "rgba(245, 158, 11, 0.15)",
        color: "var(--color-warning)",
      };
    case "keyword":
      return {
        backgroundColor: "var(--color-accent-subtle)",
        color: "var(--color-accent)",
      };
    case "semantic":
      return {
        backgroundColor: "rgba(245, 158, 11, 0.15)",
        color: "var(--color-warning)",
      };
    case "hybrid":
      return {
        backgroundColor: "rgba(14, 165, 233, 0.15)",
        color: "var(--color-info)",
      };
    case "danger":
      return {
        backgroundColor: "rgba(239, 68, 68, 0.15)",
        color: "var(--color-error)",
      };
    default:
      return {
        backgroundColor: "var(--color-bg-tertiary)",
        color: "var(--color-text-muted)",
      };
  }
};

export const Badge = memo(function Badge({
  variant = "default",
  children,
  className = "",
}: BadgeProps) {
  const variantStyle = getVariantStyle(variant);

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${className}`}
      style={variantStyle}
    >
      {children}
    </span>
  );
});

// 파일 확장자에서 Badge variant 추출
export function getFileTypeBadgeVariant(fileName: string): BadgeVariant {
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
