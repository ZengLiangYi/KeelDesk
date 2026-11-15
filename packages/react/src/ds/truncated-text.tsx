import * as React from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../ui/tooltip";
import { cn } from "../lib/utils";

export interface TruncatedTextProps {
  text: string;
  children?: React.ReactNode;
  tooltip?: React.ReactNode;
  mono?: boolean;
  muted?: boolean;
  side?: "top" | "bottom" | "left" | "right";
  className?: string;
  tooltipClassName?: string;
}

export function TruncatedText({
  text,
  children,
  tooltip,
  mono = false,
  muted = false,
  side = "top",
  className,
  tooltipClassName,
}: TruncatedTextProps) {
  const tooltipContent = tooltip ?? text;
  const isIdenticalTooltip = tooltip === undefined || tooltip === text;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          tabIndex={0}
          className={cn(
            children
              ? "inline-flex max-w-full items-center gap-1.5 truncate"
              : "block max-w-full truncate",
            "rounded-xs outline-none focus-visible:ring-1 focus-visible:ring-ring",
            mono && "font-mono",
            muted && "text-muted-foreground",
            className,
          )}
        >
          {children ?? text}
        </span>
      </TooltipTrigger>
      <TooltipContent
        side={side}
        aria-hidden={isIdenticalTooltip ? true : undefined}
        className={cn(
          "max-w-md break-all",
          mono && "font-mono text-micro",
          tooltipClassName,
        )}
      >
        {tooltipContent}
      </TooltipContent>
    </Tooltip>
  );
}
