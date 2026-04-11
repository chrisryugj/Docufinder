import { InputHTMLAttributes, forwardRef, ReactNode } from "react";
import type { CSSPropertiesWithVars } from "../../types/css";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  error?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ leftIcon, rightIcon, error, className = "", id, ...props }, ref) => {
    const errorId = error && id ? `${id}-error` : undefined;
    return (
      <div className="relative">
        {leftIcon && (
          <div className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--color-text-muted)" }}>
            {leftIcon}
          </div>
        )}
        <input
          ref={ref}
          id={id}
          aria-describedby={errorId}
          className={`
            w-full rounded-lg border
            focus:outline-none focus:ring-2 focus:border-transparent
            disabled:opacity-50 disabled:cursor-not-allowed
            ${leftIcon ? "pl-12" : "pl-4"}
            ${rightIcon ? "pr-12" : "pr-4"}
            py-3
            ${error ? "border-[var(--color-error)] focus:ring-[var(--color-error)]" : ""}
            ${className}
          `}
          style={{
            backgroundColor: "var(--color-bg-tertiary)",
            color: "var(--color-text-primary)",
            borderColor: error ? undefined : "var(--color-border)",
            "--tw-ring-color": "var(--color-accent)",
          } as CSSPropertiesWithVars}
          {...props}
        />
        {rightIcon && (
          <div className="absolute right-4 top-1/2 -translate-y-1/2">
            {rightIcon}
          </div>
        )}
        {error && <p id={errorId} className="mt-1 text-sm clr-error">{error}</p>}
      </div>
    );
  }
);

Input.displayName = "Input";
