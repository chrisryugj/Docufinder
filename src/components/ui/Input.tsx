import { InputHTMLAttributes, forwardRef, ReactNode } from "react";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  leftIcon?: ReactNode;
  rightIcon?: ReactNode;
  error?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ leftIcon, rightIcon, error, className = "", ...props }, ref) => {
    return (
      <div className="relative">
        {leftIcon && (
          <div className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--color-text-muted)" }}>
            {leftIcon}
          </div>
        )}
        <input
          ref={ref}
          className={`
            w-full rounded-lg border
            focus:outline-none focus:ring-2 focus:border-transparent
            disabled:opacity-50 disabled:cursor-not-allowed
            ${leftIcon ? "pl-12" : "pl-4"}
            ${rightIcon ? "pr-12" : "pr-4"}
            py-3
            ${error ? "border-red-500 focus:ring-red-500" : ""}
            ${className}
          `}
          style={{
            backgroundColor: "var(--color-bg-tertiary)",
            color: "var(--color-text-primary)",
            borderColor: error ? undefined : "var(--color-border)",
            // @ts-expect-error CSS custom property for placeholder
            "--tw-ring-color": "var(--color-accent)",
          }}
          {...props}
        />
        {rightIcon && (
          <div className="absolute right-4 top-1/2 -translate-y-1/2">
            {rightIcon}
          </div>
        )}
        {error && <p className="mt-1 text-sm text-red-400">{error}</p>}
      </div>
    );
  }
);

Input.displayName = "Input";
