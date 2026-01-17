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
      };
    case "secondary":
      return {
        backgroundColor: "var(--color-bg-tertiary)",
        color: "var(--color-text-secondary)",
      };
    case "ghost":
      return {
        backgroundColor: "transparent",
        color: "var(--color-text-muted)",
      };
    case "danger":
      return {
        backgroundColor: "var(--color-error)",
        color: "white",
      };
  }
};

const getHoverStyles = (variant: ButtonVariant): CSSProperties => {
  switch (variant) {
    case "primary":
      return {
        backgroundColor: "var(--color-accent-hover)",
        boxShadow: "0 4px 12px rgba(14, 165, 233, 0.3)",
        transform: "translateY(-1px)",
      };
    case "secondary":
      return {
        backgroundColor: "var(--color-bg-hover)",
        boxShadow: "0 2px 8px rgba(0, 0, 0, 0.08)",
      };
    case "ghost":
      return {
        backgroundColor: "var(--color-bg-tertiary)",
        color: "var(--color-text-secondary)",
      };
    case "danger":
      return {
        backgroundColor: "#B91C1C",
        boxShadow: "0 4px 12px rgba(220, 38, 38, 0.3)",
        transform: "translateY(-1px)",
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
          rounded-lg font-medium transition-all duration-200
          disabled:cursor-not-allowed disabled:opacity-50
          hover:shadow-md active:scale-[0.98]
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
            {children}
          </span>
        ) : (
          children
        )}
      </button>
    );
  }
);

Button.displayName = "Button";
