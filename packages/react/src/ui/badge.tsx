import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "../lib/utils"

const badgeVariants = cva(
  "group/badge inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-xs border border-transparent px-1.5 py-0.5 text-micro font-medium whitespace-nowrap transition-colors duration-150 ease-(--ease-out-quart) focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50 has-data-[icon=inline-end]:pr-1 has-data-[icon=inline-start]:pl-1 aria-invalid:border-status-danger aria-invalid:ring-status-danger/30 [&>svg]:pointer-events-none [&>svg]:size-3!",
  {
    variants: {
      variant: {
        default:
          "border-primary-border bg-primary-subtle text-primary-ink [a&]:hover:bg-primary-subtle/80",
        secondary:
          "border-border-subtle bg-surface-hover text-ink-secondary [a&]:hover:bg-surface-active",
        outline:
          "border-border bg-transparent text-ink-secondary [a&]:hover:bg-surface-hover [a&]:hover:text-foreground",
        destructive:
          "border-status-danger-border bg-status-danger-subtle text-status-danger [a&]:hover:bg-status-danger/22",
        success:
          "border-status-ok-border bg-status-ok-subtle text-status-ok",
        warning:
          "border-status-warn-border bg-status-warn-subtle text-status-warn",
        info:
          "border-status-info-border bg-status-info-subtle text-status-info",
        ghost:
          "text-ink-secondary [a&]:hover:bg-surface-hover [a&]:hover:text-foreground",
        link: "text-primary underline-offset-4 [a&]:hover:underline",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-variant={variant}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
