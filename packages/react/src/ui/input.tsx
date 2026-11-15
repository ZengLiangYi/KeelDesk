import * as React from "react"

import { cn } from "../lib/utils"

export interface InputProps extends React.ComponentProps<"input"> {
  inputSize?: "sm" | "default"
}

function Input({
  className,
  type,
  inputSize = "default",
  ...props
}: InputProps) {
  return (
    <input
      type={type}
      data-slot="input"
      data-size={inputSize}
      className={cn(
        "w-full min-w-0 rounded-sm border border-border bg-surface-sunken px-2.5 py-1 text-body text-foreground transition-colors duration-150 ease-(--ease-out-quart) outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-caption file:font-medium file:text-foreground placeholder:text-muted-foreground hover:border-border-strong focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/35 disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-surface-subtle disabled:opacity-50 aria-invalid:border-status-danger aria-invalid:ring-2 aria-invalid:ring-status-danger/30 data-[size=default]:h-8 data-[size=sm]:h-7 data-[size=sm]:text-caption",
        className
      )}
      {...props}
    />
  )
}

export { Input }
