import { ButtonHTMLAttributes, forwardRef, CSSProperties } from "react";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "lg";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  isLoading?: boolean;
}

const getVariantStyles = (variant: ButtonVariant): CSSProperties => {
  switch (variant) {
    case "primary":
      return {
        backgroundColor: "var(--color-accent)",
        color: "white",
        border: "1px solid var(--color-accent)",
      };
    case "secondary":
      return {
        backgroundColor: "var(--color-bg-secondary)",
        color: "var(--color-text-secondary)",
        border: "1px solid var(--color-border)",
      };
    case "ghost":
      return {
        backgroundColor: "transparent",
        color: "var(--color-text-muted)",
        border: "1px solid transparent",
      };
    case "danger":
      return {
        backgroundColor: "var(--color-error)",
        color: "white",
        border: "1px solid var(--color-error)",
      };
  }
};

const getHoverStyles = (variant: ButtonVariant): CSSProperties => {
  switch (variant) {
    case "primary":
      return {
        backgroundColor: "var(--color-accent-hover)",
        borderColor: "var(--color-accent-hover)",
      };
    case "secondary":
      return {
        backgroundColor: "var(--color-bg-tertiary)",
        borderColor: "var(--color-border-hover)",
        color: "var(--color-text-primary)",
      };
    case "ghost":
      return {
        backgroundColor: "var(--color-bg-tertiary)",
        color: "var(--color-text-primary)",
      };
    case "danger":
      return {
        backgroundColor: "#991B1B",
        borderColor: "#991B1B",
      };
  }
};

const sizeStyles: Record<ButtonSize, string> = {
  sm: "px-2.5 py-1.5 text-xs",
  md: "px-4 py-2 text-sm",
  lg: "px-6 py-3 text-base",
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = "primary",
      size = "md",
      isLoading = false,
      disabled,
      className = "",
      children,
      style,
      onMouseEnter,
      onMouseLeave,
      ...props
    },
    ref
  ) => {
    const baseStyles = getVariantStyles(variant);
    const hoverStyles = getHoverStyles(variant);

    return (
      <button
        ref={ref}
        disabled={disabled || isLoading}
        className={`
          rounded-md font-medium transition-colors duration-100
          disabled:cursor-not-allowed disabled:opacity-50
          ${sizeStyles[size]}
          ${className}
        `}
        style={{
          ...baseStyles,
          ...style,
        }}
        onMouseEnter={(e) => {
          if (!disabled && !isLoading) {
            Object.assign(e.currentTarget.style, hoverStyles);
          }
          onMouseEnter?.(e);
        }}
        onMouseLeave={(e) => {
          Object.assign(e.currentTarget.style, baseStyles);
          onMouseLeave?.(e);
        }}
        {...props}
      >
        {isLoading ? (
          <span className="flex items-center justify-center gap-2">
            <span
              className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"
              style={{ borderTopColor: "transparent" }}
            />
            {isLoading && children ? "로딩 중..." : null}
          </span>
        ) : (
          children
        )}
      </button>
    );
  }
);

Button.displayName = "Button";
