import * as React from "react";
import { cn } from "@/lib/utils";

export interface EmptyStateProps {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: EmptyStateProps) {
  return (
    <div
      data-slot="empty-state"
      className={cn(
        "flex flex-1 flex-col items-center justify-center gap-3 rounded-md border border-dashed border-border bg-surface-sunken/55 p-5 text-center",
        className,
      )}
    >
      {icon && (
        <div
          aria-hidden="true"
          className="flex size-8 shrink-0 items-center justify-center rounded-sm border border-border-subtle bg-surface-panel text-muted-foreground [&_svg:not([class*='size-'])]:size-4"
        >
          {icon}
        </div>
      )}
      <div className="flex flex-col gap-1">
        <p className="text-body font-medium text-foreground">{title}</p>
        {description && (
          <p className="max-w-[38ch] text-caption text-muted-foreground">
            {description}
          </p>
        )}
      </div>
      {action && <div className="mt-0.5 flex items-center gap-2">{action}</div>}
    </div>
  );
}
