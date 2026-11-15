import * as React from "react"

import { cn } from "@/lib/utils"

function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn(
        "rounded-xs bg-surface-hover motion-safe:animate-pulse",
        className
      )}
      {...props}
    />
  )
}

export { Skeleton }
