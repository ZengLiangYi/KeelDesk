import { useEffect, useMemo, useState } from "react";
import {
  AlertCircleIcon,
  CheckIcon,
  ClockIcon,
  KeyRoundIcon,
  ShieldXIcon,
} from "lucide-react";
import type { PendingToolApproval } from "@keeldesk/react";
import {
  CodeWell,
  MetricPill,
  SafetyTierBadge,
} from "@/components/ds";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

export interface ToolApprovalDialogProps {
  activeApproval: PendingToolApproval | null;
  pendingCount: number;
  isSubmitting: boolean;
  error: string | null;
  onApproveOnce: (requestId: string) => Promise<boolean>;
  onApproveForSession: (requestId: string) => Promise<boolean>;
  onDeny: (requestId: string) => Promise<boolean>;
}

function formatPrettyJson(rawJson: string): string {
  const trimmed = rawJson.trim();
  if (!trimmed) {
    return "{}";
  }
  try {
    const parsed: unknown = JSON.parse(trimmed);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return rawJson;
  }
}

export function ToolApprovalDialog({
  activeApproval,
  pendingCount,
  isSubmitting,
  error,
  onApproveOnce,
  onApproveForSession,
  onDeny,
}: ToolApprovalDialogProps) {
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    if (!activeApproval) {
      return;
    }

    setNowMs(Date.now());
    const timer = window.setInterval(() => {
      setNowMs(Date.now());
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [activeApproval?.request_id]);

  const remainingSecs = useMemo(() => {
    if (!activeApproval) {
      return 0;
    }
    const expiresMs = Date.parse(activeApproval.expires_at_iso);
    if (Number.isNaN(expiresMs)) {
      return activeApproval.timeout_secs;
    }
    return Math.max(0, Math.ceil((expiresMs - nowMs) / 1000));
  }, [activeApproval, nowMs]);

  const prettyArguments = useMemo(
    () =>
      activeApproval ? formatPrettyJson(activeApproval.arguments_json) : "{}",
    [activeApproval],
  );

  const callerLabel = activeApproval?.caller_hint ?? "external-mcp-client";

  return (
    <AlertDialog open={activeApproval !== null}>
      <AlertDialogContent
        className="sm:max-w-xl"
        onEscapeKeyDown={(event) => event.preventDefault()}
      >
        <AlertDialogHeader className="gap-2">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap items-center gap-1.5">
              <SafetyTierBadge
                safety="confirm_required"
                mode="banner"
                withTooltip={false}
              />
              {pendingCount > 1 && (
                <MetricPill tone="neutral">
                  1 / {pendingCount} 待审批
                </MetricPill>
              )}
            </div>

            <MetricPill
              tone={remainingSecs <= 10 ? "danger" : "subtle"}
              aria-live="polite"
              className="gap-1"
            >
              <ClockIcon aria-hidden="true" />
              {remainingSecs}s 后超时阻断
            </MetricPill>
          </div>

          <AlertDialogTitle>人工确认：外部智能体请求高危工具调用</AlertDialogTitle>
          <AlertDialogDescription>
            {activeApproval?.description ??
              "当前 MCP 调用涉及 Windows 11 宿主状态变更，通道已挂起并等待人工决策。"}
          </AlertDialogDescription>
        </AlertDialogHeader>

        <Separator />

        {activeApproval && (
          <div className="flex min-w-0 flex-col gap-3">
            <dl className="grid grid-cols-2 gap-x-4 gap-y-2 rounded-md border border-border-subtle bg-surface-sunken px-3 py-2.5 text-caption">
              <div className="flex min-w-0 flex-col gap-0.5">
                <dt className="text-muted-foreground">目标工具</dt>
                <dd className="truncate font-mono font-medium text-foreground">
                  {activeApproval.tool_name}
                </dd>
              </div>

              <div className="flex min-w-0 flex-col gap-0.5">
                <dt className="text-muted-foreground">注册模块</dt>
                <dd className="flex items-center">
                  <MetricPill tone="subtle">
                    {activeApproval.tool_source}
                  </MetricPill>
                </dd>
              </div>

              <div className="flex min-w-0 flex-col gap-0.5">
                <dt className="text-muted-foreground">调用方标识</dt>
                <dd className="truncate font-mono text-foreground">
                  {callerLabel}
                </dd>
              </div>

              <div className="flex min-w-0 flex-col gap-0.5">
                <dt className="text-muted-foreground">会话作用域</dt>
                <dd className="truncate font-mono text-muted-foreground">
                  {activeApproval.session_scope}
                </dd>
              </div>
            </dl>

            <div className="flex min-w-0 flex-col gap-1.5">
              <div className="flex items-center justify-between gap-2 text-caption text-muted-foreground">
                <span>请求参数载荷 (JSON)</span>
                <span className="font-mono text-micro tabular-nums">
                  请求单号 {activeApproval.request_id}
                </span>
              </div>

              <CodeWell
                code={prettyArguments}
                copyValue={prettyArguments}
                copyLabel="复制请求参数"
                copiedLabel="已复制参数"
                wrapLines
                viewportClassName="max-h-36"
              />
            </div>
          </div>
        )}

        {error && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>审批指令提交失败</AlertTitle>
            <AlertDescription className="font-mono text-caption">
              {error}
            </AlertDescription>
          </Alert>
        )}

        <AlertDialogFooter className="pt-1">
          <Button
            type="button"
            variant="destructive"
            className="sm:mr-auto"
            disabled={isSubmitting || !activeApproval}
            onClick={() => {
              if (activeApproval) {
                void onDeny(activeApproval.request_id);
              }
            }}
          >
            <ShieldXIcon data-icon="inline-start" />
            拒绝并阻断
          </Button>

          <Button
            type="button"
            variant="outline"
            disabled={isSubmitting || !activeApproval}
            onClick={() => {
              if (activeApproval) {
                void onApproveForSession(activeApproval.request_id);
              }
            }}
          >
            <KeyRoundIcon data-icon="inline-start" />
            本会话免审放行
          </Button>

          <Button
            type="button"
            variant="default"
            disabled={isSubmitting || !activeApproval}
            onClick={() => {
              if (activeApproval) {
                void onApproveOnce(activeApproval.request_id);
              }
            }}
          >
            <CheckIcon data-icon="inline-start" />
            允许本次执行
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
