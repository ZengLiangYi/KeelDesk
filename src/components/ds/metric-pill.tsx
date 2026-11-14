import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export type MetricPillTone =
  | "neutral"
  | "subtle"
  | "ok"
  | "warn"
  | "danger"
  | "info"
  | "primary";

const TONE_TO_VARIANT = {
  neutral: "secondary",
  subtle: "outline",
  ok: "success",
  warn: "warning",
  danger: "destructive",
  info: "info",
  primary: "default",
} as const;

export interface MetricPillProps
  extends Omit<React.ComponentProps<typeof Badge>, "variant"> {
  tone?: MetricPillTone;
}

export function MetricPill({
  tone = "neutral",
  className,
  children,
  ...props
}: MetricPillProps) {
  return (
    <Badge
      variant={TONE_TO_VARIANT[tone]}
      className={cn(
        "font-mono text-micro tabular-nums select-none",
        className,
      )}
      {...props}
    >
      {children}
    </Badge>
  );
}
