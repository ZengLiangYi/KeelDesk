import * as React from "react";
import type {
  ChildServerStatus,
  KeelServerStatus,
  OdrReadinessState,
} from "@/bindings";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export type StatusTone = "ok" | "warn" | "danger" | "info" | "neutral";

const DOT_TONE_CLASSES: Record<StatusTone, string> = {
  ok: "bg-status-ok",
  warn: "bg-status-warn",
  danger: "bg-status-danger",
  info: "bg-status-info",
  neutral: "bg-ink-muted",
};

const BADGE_VARIANT_MAP = {
  ok: "success",
  warn: "warning",
  danger: "destructive",
  info: "info",
  neutral: "outline",
} as const;

export interface StatusDotProps {
  tone: StatusTone;
  size?: "sm" | "md";
  className?: string;
}

export function StatusDot({
  tone,
  size = "sm",
  className,
}: StatusDotProps) {
  return (
    <span
      data-slot="status-dot"
      aria-hidden="true"
      className={cn(
        "inline-block shrink-0 rounded-full",
        size === "md" ? "size-2" : "size-1.5",
        DOT_TONE_CLASSES[tone],
        className,
      )}
    />
  );
}

export interface StatusBadgeProps
  extends Omit<React.ComponentProps<typeof Badge>, "variant"> {
  tone: StatusTone;
  dotSize?: "sm" | "md";
  mono?: boolean;
}

export function StatusBadge({
  tone,
  dotSize = "sm",
  mono = false,
  className,
  children,
  ...props
}: StatusBadgeProps) {
  return (
    <Badge
      variant={BADGE_VARIANT_MAP[tone]}
      className={cn(
        "gap-1.5 select-none",
        mono && "font-mono tabular-nums",
        className,
      )}
      {...props}
    >
      <StatusDot tone={tone} size={dotSize} />
      {children}
    </Badge>
  );
}

export function serverStatusToTone(
  status: KeelServerStatus | undefined,
): StatusTone {
  switch (status) {
    case "running":
      return "ok";
    case "starting":
      return "warn";
    case "error":
    case "stopped":
      return "danger";
    default:
      return "neutral";
  }
}

export function serverStatusLabel(status: KeelServerStatus | undefined): string {
  switch (status) {
    case "running":
      return "运行中";
    case "starting":
      return "启动中";
    case "error":
      return "绑定异常";
    case "stopped":
      return "已停止";
    default:
      return "初始化";
  }
}

export function childStatusToTone(
  status: ChildServerStatus | undefined,
): StatusTone {
  switch (status) {
    case "running":
      return "ok";
    case "starting":
      return "warn";
    case "failed":
      return "danger";
    case "stopped":
    default:
      return "neutral";
  }
}

export function childStatusLabel(status: ChildServerStatus | undefined): string {
  switch (status) {
    case "running":
      return "运行中";
    case "starting":
      return "启动中";
    case "failed":
      return "异常";
    case "stopped":
      return "已停止";
    default:
      return "未知";
  }
}

export function odrReadinessToTone(
  readiness: OdrReadinessState | undefined,
): StatusTone {
  switch (readiness) {
    case "ready":
      return "ok";
    case "missing_odr_cli":
      return "warn";
    case "unsupported_os":
      return "danger";
    default:
      return "neutral";
  }
}

export function odrReadinessLabel(
  readiness: OdrReadinessState | undefined,
): string {
  switch (readiness) {
    case "ready":
      return "odr.exe 就绪";
    case "missing_odr_cli":
      return "未检出 odr.exe · 回退模式";
    case "unsupported_os":
      return "需要 Windows 11";
    default:
      return "探测中";
  }
}
