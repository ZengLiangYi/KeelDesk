import { useEffect, useRef, useState } from "react";
import { CheckIcon, CopyIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export interface CopyButtonProps {
  value: string;
  label?: string;
  copiedLabel?: string;
  variant?: "ghost" | "secondary" | "outline";
  size?: "xs" | "icon-xs";
  showText?: boolean;
  side?: "top" | "bottom" | "left" | "right";
  className?: string;
}

export function CopyButton({
  value,
  label = "复制",
  copiedLabel = "已复制",
  variant = "ghost",
  size,
  showText = false,
  side = "bottom",
  className,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
    };
  }, []);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
      resetTimerRef.current = window.setTimeout(() => {
        setCopied(false);
        resetTimerRef.current = null;
      }, 1600);
    } catch {
      // Clipboard unavailable in restricted context
    }
  }

  const resolvedSize = size ?? (showText ? "xs" : "icon-xs");
  const activeText = copied ? copiedLabel : label;

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant={variant}
            size={resolvedSize}
            aria-label={activeText}
            onClick={() => void handleCopy()}
            className={className}
          >
            {copied ? (
              <CheckIcon
                data-icon={showText ? "inline-start" : undefined}
                className="text-status-ok"
              />
            ) : (
              <CopyIcon data-icon={showText ? "inline-start" : undefined} />
            )}
            {showText && <span>{activeText}</span>}
          </Button>
        </TooltipTrigger>
        <TooltipContent side={side}>{activeText}</TooltipContent>
      </Tooltip>
      <span role="status" aria-live="polite" className="sr-only">
        {copied ? copiedLabel : ""}
      </span>
    </>
  );
}

export interface CodeWellProps {
  code: string;
  copyValue?: string;
  copyLabel?: string;
  copiedLabel?: string;
  wrapLines?: boolean;
  className?: string;
  scrollAreaClassName?: string;
  viewportClassName?: string;
  preClassName?: string;
}

export function CodeWell({
  code,
  copyValue,
  copyLabel = "复制代码",
  copiedLabel = "已复制",
  wrapLines = false,
  className,
  scrollAreaClassName,
  viewportClassName,
  preClassName,
}: CodeWellProps) {
  return (
    <div
      data-slot="code-well"
      className={cn(
        "relative overflow-hidden rounded-md border border-border-subtle bg-surface-sunken",
        className,
      )}
    >
      {copyValue !== undefined && (
        <div className="absolute top-1.5 right-1.5 z-(--z-sticky)">
          <CopyButton
            value={copyValue}
            label={copyLabel}
            copiedLabel={copiedLabel}
            variant="ghost"
            size="icon-xs"
            side="left"
            className="bg-surface-panel/85 hover:bg-surface-hover"
          />
        </div>
      )}
      <ScrollArea
        className={cn("w-full", scrollAreaClassName)}
        viewportClassName={viewportClassName}
      >
        <pre
          className={cn(
            "m-0 px-3 py-2.5 font-mono text-code text-foreground select-all",
            wrapLines
              ? "break-all whitespace-pre-wrap"
              : "whitespace-pre",
            copyValue !== undefined && "pr-9",
            preClassName,
          )}
        >
          <code>{code}</code>
        </pre>
      </ScrollArea>
    </div>
  );
}
