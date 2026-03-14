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

/** variant별 hover CSS 클래스 */
const variantHoverClasses: Record<ButtonVariant, string> = {
  primary: "hover-btn-primary",
  secondary: "hover-btn-secondary",
  ghost: "hover-btn-ghost",
  danger: "hover-btn-danger",
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
      ...props
    },
    ref
  ) => {
    const baseStyles = getVariantStyles(variant);

    return (
      <button
        ref={ref}
        disabled={disabled || isLoading}
        className={`
          rounded-lg font-semibold
          disabled:cursor-not-allowed disabled:opacity-50
          ${sizeStyles[size]}
          ${variantHoverClasses[variant]}
          ${className}
        `}
        style={{
          ...baseStyles,
          ...style,
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
