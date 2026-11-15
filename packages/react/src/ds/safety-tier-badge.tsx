import { ShieldAlertIcon, ShieldCheckIcon } from "lucide-react";
import type { ToolSafetyTier } from "../bindings";
import { Badge } from "../ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../ui/tooltip";
import { cn } from "../lib/utils";

export type ToolSafety = ToolSafetyTier;

export interface SafetyTierBadgeProps {
  safety: ToolSafety;
  mode?: "compact" | "banner";
  withTooltip?: boolean;
  side?: "top" | "bottom" | "left" | "right";
  className?: string;
}

export function SafetyTierBadge({
  safety,
  mode = "compact",
  withTooltip = true,
  side = "top",
  className,
}: SafetyTierBadgeProps) {
  const isSafe = safety === "safe";

  const label =
    mode === "banner"
      ? isSafe
        ? "Safe · 只读放行"
        : "ConfirmRequired · 高危拦截"
      : isSafe
        ? "Safe · 直接放行"
        : "Confirm · 需审批";

  const tooltipText = isSafe
    ? "只读或无副作用探针，框架直接执行并返回结构化结果"
    : "高危操作将挂起 MCP 调用通道并弹出桌面端人工确认网关";

  const badge = (
    <Badge
      variant={isSafe ? "secondary" : "destructive"}
      tabIndex={withTooltip ? 0 : undefined}
      className={cn(
        "select-none",
        mode === "banner" && "gap-1",
        withTooltip &&
          "outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        className,
      )}
    >
      {mode === "banner" &&
        (isSafe ? (
          <ShieldCheckIcon aria-hidden="true" />
        ) : (
          <ShieldAlertIcon aria-hidden="true" />
        ))}
      {label}
    </Badge>
  );

  if (!withTooltip) {
    return badge;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>{badge}</TooltipTrigger>
      <TooltipContent side={side}>{tooltipText}</TooltipContent>
    </Tooltip>
  );
}
