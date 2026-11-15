import { useCallback, useEffect, useState } from "react";
import {
  ActivityIcon,
  AlertCircleIcon,
  EraserIcon,
  HeartPulseIcon,
  PauseIcon,
  PlayIcon,
  SearchIcon,
  TerminalIcon,
  XIcon,
} from "lucide-react";
import {
  commands,
  type KeelHealthCheckResult,
  type McpProtocolTraceEntry,
  type McpSessionSummary,
  type McpTraceApprovalOutcome,
  type McpTraceStatus,
} from "@/bindings";
import {
  CodeWell,
  EmptyState,
  MetricPill,
  SafetyTierBadge,
  StatusBadge,
  StatusDot,
  TruncatedText,
  formatTimeHms,
  type MetricPillTone,
  type StatusTone,
} from "@/components/ds";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetTitle,
} from "@/components/ui/sheet";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import {
  useKeelDevTools,
  type DevToolsStatusFilter,
  type DevToolsTransportFilter,
  type UseKeelDevToolsResult,
} from "./useKeelDevTools";
import { isTauriRuntime } from "./useMcpServerState";

export interface KeelDevToolsProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  defaultOpen?: boolean;
  devTools?: UseKeelDevToolsResult;
  activeSessions?: McpSessionSummary[];
  lastHealthCheck?: KeelHealthCheckResult | null;
  isCheckingHealth?: boolean;
  onCheckHealth?: () => void;
  enableShortcut?: boolean;
}

type InspectorDetailTab = "split" | "request" | "response" | "meta";

function traceStatusToTone(status: McpTraceStatus): StatusTone {
  switch (status) {
    case "succeeded":
      return "ok";
    case "tool_error":
      return "warn";
    case "protocol_error":
    case "blocked":
      return "danger";
    default:
      return "neutral";
  }
}

function traceStatusLabel(status: McpTraceStatus): string {
  switch (status) {
    case "succeeded":
      return "Succeeded";
    case "tool_error":
      return "Tool Error";
    case "protocol_error":
      return "Protocol Error";
    case "blocked":
      return "Blocked";
    default:
      return status;
  }
}

function approvalOutcomeInfo(
  outcome: McpTraceApprovalOutcome,
): { label: string; tone: MetricPillTone } | null {
  switch (outcome) {
    case "not_required":
      return null;
    case "approved_once":
      return { label: "单次放行", tone: "ok" };
    case "approved_for_session":
      return { label: "会话放行", tone: "ok" };
    case "session_allowlisted":
      return { label: "白名单免审", tone: "info" };
    case "denied":
      return { label: "人工拒绝", tone: "danger" };
    case "timed_out":
      return { label: "审批超时", tone: "danger" };
    case "cancelled":
      return { label: "已取消", tone: "warn" };
    default:
      return { label: outcome, tone: "subtle" };
  }
}

function diagnosticBannerTitle(trace: McpProtocolTraceEntry): string {
  switch (trace.status) {
    case "blocked":
      return "安全网关拦截或拒绝执行";
    case "protocol_error":
      return "JSON-RPC / HTTP 协议异常";
    case "tool_error":
      return "MCP 工具返回执行错误";
    default:
      return "调用诊断摘要";
  }
}

export function KeelDevTools({
  open: controlledOpen,
  onOpenChange,
  defaultOpen = false,
  devTools: externalDevTools,
  activeSessions = [],
  lastHealthCheck: controlledHealthCheck,
  isCheckingHealth: controlledCheckingHealth,
  onCheckHealth,
  enableShortcut = true,
}: KeelDevToolsProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen);
  const [detailTab, setDetailTab] = useState<InspectorDetailTab>("split");
  const [internalHealthCheck, setInternalHealthCheck] =
    useState<KeelHealthCheckResult | null>(null);
  const [internalCheckingHealth, setInternalCheckingHealth] = useState(false);

  const internalDevTools = useKeelDevTools({ enabled: !externalDevTools });
  const controller = externalDevTools ?? internalDevTools;

  const isOpen =
    controlledOpen !== undefined ? controlledOpen : uncontrolledOpen;

  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (controlledOpen === undefined) {
        setUncontrolledOpen(next);
      }
      onOpenChange?.(next);
    },
    [controlledOpen, onOpenChange],
  );

  useEffect(() => {
    if (!enableShortcut) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || event.isComposing) {
        return;
      }
      const isModifier = (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey;
      const isBackquote = event.code === "Backquote" || event.key === "`";
      if (isModifier && isBackquote) {
        event.preventDefault();
        handleOpenChange(!isOpen);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [enableShortcut, handleOpenChange, isOpen]);

  const handleStandaloneHealthCheck = useCallback(async () => {
    setInternalCheckingHealth(true);
    try {
      if (!isTauriRuntime()) {
        setInternalHealthCheck({
          ok: true,
          status_code: 200,
          latency_ms: 1,
          endpoint_url: "http://127.0.0.1:19527/mcp",
          healthz_url: "http://127.0.0.1:19527/healthz",
          bound_port: 19527,
          active_session_count: activeSessions.length,
          total_calls: controller.stats.totalCount,
          checked_at_iso: new Date().toISOString(),
        });
        return;
      }

      const res = await commands.checkKeelHealthz();
      if (res.status === "ok") {
        setInternalHealthCheck(res.data);
      }
    } finally {
      setInternalCheckingHealth(false);
    }
  }, [activeSessions.length, controller.stats.totalCount]);

  const effectiveHealthCheck =
    controlledHealthCheck !== undefined
      ? controlledHealthCheck
      : internalHealthCheck;
  const effectiveCheckingHealth =
    controlledCheckingHealth ?? internalCheckingHealth;
  const triggerCheckHealth = onCheckHealth ?? (() => void handleStandaloneHealthCheck());

  const selectedTrace = controller.selectedTrace;
  const hasActiveFilters =
    controller.searchQuery.trim().length > 0 ||
    controller.transportFilter !== "all" ||
    controller.statusFilter !== "all";

  const transportOptions: Array<{
    value: DevToolsTransportFilter;
    label: string;
  }> = [
    { value: "all", label: "全部" },
    { value: "streamable_http", label: "HTTP Spine" },
    { value: "stdio_child", label: "Stdio 子进程" },
  ];

  const statusOptions: Array<{
    value: DevToolsStatusFilter;
    label: string;
  }> = [
    { value: "all", label: "全部状态" },
    { value: "attention", label: "异常与拦截" },
    { value: "confirm_required", label: "ConfirmRequired" },
  ];

  // Latency bar ratios
  const totalMs = selectedTrace ? Math.max(1, selectedTrace.latency.total_ms) : 1;
  const waitMs = selectedTrace?.latency.approval_wait_ms ?? 0;
  const execMs = selectedTrace?.latency.execution_ms ?? 0;
  const waitPct =
    waitMs > 0 ? Math.min(100, Math.max(2, Math.round((waitMs / totalMs) * 100))) : 0;
  const execPct =
    execMs > 0
      ? Math.min(
          100 - waitPct,
          Math.max(waitPct === 0 ? 4 : 2, Math.round((execMs / totalMs) * 100)),
        )
      : 0;

  return (
    <Sheet open={isOpen} onOpenChange={handleOpenChange}>
      <SheetContent
        side="bottom"
        showCloseButton={false}
        className="flex h-[72dvh] max-h-140 flex-col gap-0 overflow-hidden p-0 data-[side=bottom]:h-[72dvh] data-[side=bottom]:max-h-140"
      >
        <TooltipProvider delayDuration={220}>
          {/* Header Bar */}
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border-subtle px-3 py-2">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <div className="flex items-center gap-1.5">
                <ActivityIcon
                  aria-hidden="true"
                  className="size-4 shrink-0 text-primary"
                />
                <SheetTitle className="text-body font-semibold text-foreground">
                  KeelDevTools · MCP 协议调试器
                </SheetTitle>
              </div>
              <SheetDescription className="sr-only">
                实时观测 Streamable HTTP 与 Stdio 子进程 JSON-RPC 协议帧、审批耗时分布与活跃会话
              </SheetDescription>

              <Kbd>{"Ctrl + `"}</Kbd>

              <div className="flex flex-wrap items-center gap-1.5">
                <MetricPill tone="neutral">
                  {controller.stats.totalCount} 帧
                </MetricPill>
                <MetricPill
                  tone={activeSessions.length > 0 ? "ok" : "subtle"}
                >
                  {activeSessions.length} 活跃会话
                </MetricPill>
                {controller.stats.errorOrBlockedCount > 0 && (
                  <MetricPill tone="danger">
                    {controller.stats.errorOrBlockedCount} 异常/拦截
                  </MetricPill>
                )}
                {effectiveHealthCheck && (
                  <MetricPill
                    tone={effectiveHealthCheck.ok ? "ok" : "danger"}
                  >
                    {effectiveHealthCheck.status_code}{" "}
                    {effectiveHealthCheck.ok ? "OK" : "ERR"} ·{" "}
                    {effectiveHealthCheck.latency_ms} ms
                  </MetricPill>
                )}
              </div>
            </div>

            <div className="flex shrink-0 items-center gap-1.5">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="xs"
                    variant="outline"
                    disabled={effectiveCheckingHealth}
                    onClick={triggerCheckHealth}
                  >
                    <HeartPulseIcon
                      data-icon="inline-start"
                      className={cn(
                        effectiveCheckingHealth && "animate-pulse",
                      )}
                    />
                    {effectiveCheckingHealth ? "探活中..." : "探活 /healthz"}
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  向本地回环 /healthz 端点发起真实健康探针检测
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="xs"
                    variant={controller.isPaused ? "default" : "outline"}
                    onClick={controller.togglePause}
                    className="font-mono tabular-nums"
                  >
                    {controller.isPaused ? (
                      <>
                        <PlayIcon data-icon="inline-start" />
                        继续捕获
                        {controller.pausedCount > 0
                          ? ` (+${controller.pausedCount})`
                          : ""}
                      </>
                    ) : (
                      <>
                        <PauseIcon data-icon="inline-start" />
                        暂停捕获
                      </>
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  {controller.isPaused
                    ? "恢复实时捕获并将暂存帧合入观测列表"
                    : "暂停列表刷新并暂存新帧，便于稳态检查选中载荷"}
                </TooltipContent>
              </Tooltip>

              <Button
                type="button"
                size="xs"
                variant="ghost"
                disabled={
                  controller.stats.totalCount === 0 &&
                  controller.pausedCount === 0
                }
                onClick={() => void controller.clearTraces()}
              >
                <EraserIcon data-icon="inline-start" />
                清空
              </Button>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="icon-xs"
                    variant="ghost"
                    aria-label="关闭 KeelDevTools"
                    onClick={() => handleOpenChange(false)}
                  >
                    <XIcon />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">关闭协议调试器</TooltipContent>
              </Tooltip>
            </div>
          </div>

          {/* Filter Toolbar */}
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border-subtle bg-surface-subtle px-3 py-1.5">
            <div className="relative flex min-w-0 flex-1 items-center sm:max-w-xs">
              <SearchIcon
                aria-hidden="true"
                className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground"
              />
              <Input
                inputSize="sm"
                value={controller.searchQuery}
                onChange={(event) =>
                  controller.setSearchQuery(event.currentTarget.value)
                }
                placeholder="搜索 method / 工具名 / 载荷..."
                aria-label="搜索 MCP 协议追踪帧"
                className="pl-7.5 pr-7 font-mono"
              />
              {controller.searchQuery && (
                <Button
                  type="button"
                  size="icon-xs"
                  variant="ghost"
                  aria-label="清除搜索条件"
                  onClick={() => controller.setSearchQuery("")}
                  className="absolute right-1"
                >
                  <XIcon />
                </Button>
              )}
            </div>

            <div className="flex flex-wrap items-center gap-1.5">
              <div
                role="group"
                aria-label="传输通道过滤"
                className="flex items-center gap-1"
              >
                {transportOptions.map((option) => {
                  const active = controller.transportFilter === option.value;
                  return (
                    <Button
                      key={option.value}
                      type="button"
                      size="xs"
                      variant={active ? "secondary" : "ghost"}
                      aria-pressed={active}
                      onClick={() => controller.setTransportFilter(option.value)}
                      className={cn(
                        active &&
                          "ring-1 ring-inset ring-primary/45 text-foreground",
                      )}
                    >
                      {option.label}
                    </Button>
                  );
                })}
              </div>

              <Separator orientation="vertical" className="h-4" />

              <div
                role="group"
                aria-label="状态与安全等级过滤"
                className="flex items-center gap-1"
              >
                {statusOptions.map((option) => {
                  const active = controller.statusFilter === option.value;
                  return (
                    <Button
                      key={option.value}
                      type="button"
                      size="xs"
                      variant={active ? "secondary" : "ghost"}
                      aria-pressed={active}
                      onClick={() => controller.setStatusFilter(option.value)}
                      className={cn(
                        active &&
                          "ring-1 ring-inset ring-primary/45 text-foreground",
                      )}
                    >
                      {option.label}
                    </Button>
                  );
                })}
              </div>
            </div>
          </div>

          {controller.error && (
            <div className="shrink-0 px-3 pt-2">
              <Alert variant="destructive">
                <AlertCircleIcon />
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <AlertTitle>协议追踪读取异常</AlertTitle>
                    <AlertDescription className="font-mono text-caption">
                      {controller.error}
                    </AlertDescription>
                  </div>
                  <Button
                    type="button"
                    size="icon-xs"
                    variant="ghost"
                    aria-label="关闭追踪异常提示"
                    onClick={controller.clearError}
                  >
                    <XIcon />
                  </Button>
                </div>
              </Alert>
            </div>
          )}

          {/* Master-Detail Split */}
          <div className="grid min-h-0 min-w-0 flex-1 grid-cols-12 overflow-hidden">
            {/* Left Column: Trace List */}
            <div className="col-span-5 flex min-h-0 min-w-0 flex-col border-r border-border-subtle">
              {controller.filteredTraces.length === 0 ? (
                <div className="flex min-h-0 flex-1 p-3">
                  <EmptyState
                    icon={<TerminalIcon />}
                    title={
                      hasActiveFilters
                        ? "未找到匹配的协议帧"
                        : "尚未捕获 MCP 协议追踪帧"
                    }
                    description={
                      hasActiveFilters
                        ? "调整搜索关键词或切换通道 / 状态过滤条件以查看已有协议报文。"
                        : "执行回环自检、触发审批演练或调用 Stdio 子进程工具后，完整 JSON-RPC 帧将实时呈现。"
                    }
                    action={
                      hasActiveFilters ? (
                        <Button
                          type="button"
                          size="xs"
                          variant="secondary"
                          onClick={() => {
                            controller.setSearchQuery("");
                            controller.setTransportFilter("all");
                            controller.setStatusFilter("all");
                          }}
                        >
                          重置筛选条件
                        </Button>
                      ) : undefined
                    }
                  />
                </div>
              ) : (
                <ScrollArea className="min-h-0 flex-1">
                  <div
                    role="listbox"
                    aria-label="MCP 协议追踪帧列表"
                    className="divide-y divide-border-subtle"
                  >
                    {controller.filteredTraces.map((entry) => {
                      const isSelected = selectedTrace?.seq === entry.seq;
                      const outcome = approvalOutcomeInfo(
                        entry.approval_outcome,
                      );

                      return (
                        <button
                          key={entry.seq}
                          type="button"
                          role="option"
                          aria-selected={isSelected}
                          onClick={() => controller.setSelectedSeq(entry.seq)}
                          className={cn(
                            "flex w-full min-w-0 flex-col gap-1 px-3 py-2 text-left transition-colors duration-150 ease-(--ease-out-quart) outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset",
                            isSelected
                              ? "bg-surface-active ring-1 ring-inset ring-primary/45"
                              : "hover:bg-surface-hover/65",
                          )}
                        >
                          <div className="flex min-w-0 items-center justify-between gap-2">
                            <div className="flex min-w-0 items-center gap-1.5">
                              <StatusDot
                                tone={traceStatusToTone(entry.status)}
                              />
                              <span className="shrink-0 font-mono text-micro tabular-nums text-muted-foreground">
                                #{entry.seq}
                              </span>
                              <span className="truncate font-mono text-caption font-medium text-foreground">
                                {entry.tool_name
                                  ? `${entry.method} · ${entry.tool_name}`
                                  : entry.method}
                              </span>
                            </div>

                            <span
                              className={cn(
                                "shrink-0 font-mono text-micro tabular-nums",
                                entry.status === "succeeded"
                                  ? "text-ink-secondary"
                                  : "text-status-danger",
                              )}
                            >
                              {entry.latency.total_ms} ms
                            </span>
                          </div>

                          <div className="flex min-w-0 items-center justify-between gap-2">
                            <div className="flex min-w-0 items-center gap-1.5">
                              <MetricPill
                                tone={
                                  entry.transport === "streamable_http"
                                    ? "info"
                                    : "subtle"
                                }
                              >
                                {entry.transport === "streamable_http"
                                  ? "HTTP"
                                  : "STDIO"}
                              </MetricPill>
                              {outcome && (
                                <MetricPill tone={outcome.tone}>
                                  {outcome.label}
                                </MetricPill>
                              )}
                              <span className="truncate font-mono text-micro text-muted-foreground">
                                {entry.caller_hint ?? entry.target}
                              </span>
                            </div>

                            <span className="shrink-0 font-mono text-micro tabular-nums text-muted-foreground">
                              {formatTimeHms(entry.timestamp_iso)}
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                </ScrollArea>
              )}
            </div>

            {/* Right Column: Payload & Latency Inspector */}
            <div className="col-span-7 flex min-h-0 min-w-0 flex-col overflow-hidden">
              {!selectedTrace ? (
                <div className="flex min-h-0 flex-1 p-3">
                  <EmptyState
                    icon={<ActivityIcon />}
                    title="未选中协议帧"
                    description="从左侧追踪列表选择任意 JSON-RPC 请求帧，查看双栏请求/响应载荷与审批耗时拆解。"
                  />
                </div>
              ) : (
                <>
                  {/* Selected Frame Summary Strip + Latency Breakdown Bar */}
                  <div className="flex shrink-0 flex-col gap-2 border-b border-border-subtle px-3 py-2.5">
                    <div className="flex min-w-0 flex-wrap items-center justify-between gap-2">
                      <div className="flex min-w-0 flex-wrap items-center gap-1.5">
                        <span className="font-mono text-micro tabular-nums text-muted-foreground">
                          #{selectedTrace.seq}
                        </span>
                        <span className="truncate font-mono text-body font-semibold text-foreground">
                          {selectedTrace.tool_name
                            ? `${selectedTrace.method} · ${selectedTrace.tool_name}`
                            : selectedTrace.method}
                        </span>
                        <StatusBadge
                          tone={traceStatusToTone(selectedTrace.status)}
                          mono
                        >
                          {traceStatusLabel(selectedTrace.status)}
                        </StatusBadge>
                        {selectedTrace.safety_tier && (
                          <SafetyTierBadge
                            safety={selectedTrace.safety_tier}
                            mode="compact"
                          />
                        )}
                        <MetricPill tone="subtle">
                          {selectedTrace.transport === "streamable_http"
                            ? "HTTP"
                            : "STDIO"}{" "}
                          · {selectedTrace.direction.toUpperCase()}
                        </MetricPill>
                        {selectedTrace.status_code !== null && (
                          <MetricPill
                            tone={
                              selectedTrace.status_code >= 200 &&
                              selectedTrace.status_code < 300
                                ? "ok"
                                : "danger"
                            }
                          >
                            {selectedTrace.status_code}
                          </MetricPill>
                        )}
                      </div>

                      <div className="flex shrink-0 items-center gap-2 font-mono text-micro tabular-nums text-muted-foreground">
                        {selectedTrace.linked_call_id !== null && (
                          <span>Call #{selectedTrace.linked_call_id}</span>
                        )}
                        <span>{formatTimeHms(selectedTrace.timestamp_iso)}</span>
                      </div>
                    </div>

                    {/* Latency Breakdown Bar */}
                    <div className="flex flex-col gap-1.5 rounded-md border border-border-subtle bg-surface-sunken px-3 py-2">
                      <div className="flex flex-wrap items-center justify-between gap-2 text-caption">
                        <span className="font-mono tabular-nums text-foreground">
                          总耗时 {selectedTrace.latency.total_ms} ms
                          <span className="text-muted-foreground"> · </span>
                          <span className="text-status-warn">
                            审批等待 {selectedTrace.latency.approval_wait_ms} ms
                          </span>
                          <span className="text-muted-foreground"> · </span>
                          <span className="text-status-ok">
                            执行耗时 {selectedTrace.latency.execution_ms} ms
                          </span>
                        </span>

                        <span className="font-mono text-micro tabular-nums text-muted-foreground">
                          Req {selectedTrace.request_bytes} B / Resp{" "}
                          {selectedTrace.response_bytes} B
                        </span>
                      </div>

                      <div
                        aria-hidden="true"
                        className="flex h-1.5 w-full overflow-hidden rounded-xs bg-surface-hover"
                      >
                        {waitPct > 0 && (
                          <div
                            className="h-full bg-status-warn transition-all duration-150 ease-(--ease-out-quart)"
                            style={{ width: `${waitPct}%` }}
                          />
                        )}
                        {execPct > 0 && (
                          <div
                            className={cn(
                              "h-full transition-all duration-150 ease-(--ease-out-quart)",
                              selectedTrace.status === "succeeded"
                                ? "bg-status-ok"
                                : "bg-status-danger",
                            )}
                            style={{ width: `${execPct}%` }}
                          />
                        )}
                      </div>
                    </div>
                  </div>

                  {/* Conditional Diagnostic Banner */}
                  {(selectedTrace.status !== "succeeded" ||
                    selectedTrace.error_message) && (
                    <div className="shrink-0 px-3 pt-2">
                      <Alert
                        variant={
                          selectedTrace.status === "tool_error"
                            ? "warning"
                            : "destructive"
                        }
                      >
                        <AlertCircleIcon />
                        <AlertTitle>
                          {diagnosticBannerTitle(selectedTrace)}
                        </AlertTitle>
                        <AlertDescription className="font-mono text-caption">
                          {selectedTrace.error_message ??
                            `协议状态: ${selectedTrace.status}`}
                        </AlertDescription>
                      </Alert>
                    </div>
                  )}

                  {/* Payload & Metadata Tabs */}
                  <Tabs
                    value={detailTab}
                    onValueChange={(value) =>
                      setDetailTab(value as InspectorDetailTab)
                    }
                    className="flex min-h-0 min-w-0 flex-1 flex-col gap-0"
                  >
                    <div className="shrink-0 border-b border-border-subtle px-3 pt-1 pb-1">
                      <TabsList
                        variant="line"
                        aria-label="协议载荷详情视图"
                        className="w-full flex-nowrap justify-start overflow-x-auto"
                      >
                        <TabsTrigger
                          value="split"
                          className="flex-none px-2.5 text-caption"
                        >
                          双栏报文
                        </TabsTrigger>
                        <TabsTrigger
                          value="request"
                          className="flex-none px-2.5 text-caption"
                        >
                          Request 载荷
                        </TabsTrigger>
                        <TabsTrigger
                          value="response"
                          className="flex-none px-2.5 text-caption"
                        >
                          Response 载荷
                        </TabsTrigger>
                        <TabsTrigger
                          value="meta"
                          className="flex-none px-2.5 text-caption"
                        >
                          会话与元数据
                        </TabsTrigger>
                      </TabsList>
                    </div>

                    <TabsContent
                      value="split"
                      className="mt-0 grid min-h-0 min-w-0 flex-1 grid-cols-2 gap-2.5 overflow-hidden p-3 data-[state=inactive]:hidden"
                    >
                      <div className="flex min-h-0 min-w-0 flex-col gap-1.5">
                        <div className="flex shrink-0 items-center justify-between gap-2 text-caption">
                          <span className="font-medium text-ink-secondary">
                            Request
                          </span>
                          <div className="flex items-center gap-1.5 font-mono text-micro tabular-nums text-muted-foreground">
                            {selectedTrace.request_truncated && (
                              <MetricPill tone="warn">已截断</MetricPill>
                            )}
                            <span>{selectedTrace.request_bytes} B</span>
                          </div>
                        </div>
                        <CodeWell
                          code={selectedTrace.request_json}
                          copyValue={selectedTrace.request_json}
                          copyLabel="复制 Request 载荷"
                          copiedLabel="已复制 Request"
                          wrapLines
                          className="flex min-h-0 flex-1 flex-col"
                          scrollAreaClassName="min-h-0 flex-1"
                          viewportClassName="h-full"
                        />
                      </div>

                      <div className="flex min-h-0 min-w-0 flex-col gap-1.5">
                        <div className="flex shrink-0 items-center justify-between gap-2 text-caption">
                          <span className="font-medium text-ink-secondary">
                            Response
                          </span>
                          <div className="flex items-center gap-1.5 font-mono text-micro tabular-nums text-muted-foreground">
                            {selectedTrace.response_truncated && (
                              <MetricPill tone="warn">已截断</MetricPill>
                            )}
                            <span>{selectedTrace.response_bytes} B</span>
                          </div>
                        </div>
                        <CodeWell
                          code={selectedTrace.response_json}
                          copyValue={selectedTrace.response_json}
                          copyLabel="复制 Response 载荷"
                          copiedLabel="已复制 Response"
                          wrapLines
                          className="flex min-h-0 flex-1 flex-col"
                          scrollAreaClassName="min-h-0 flex-1"
                          viewportClassName="h-full"
                        />
                      </div>
                    </TabsContent>

                    <TabsContent
                      value="request"
                      className="mt-0 flex min-h-0 min-w-0 flex-1 flex-col gap-1.5 overflow-hidden p-3 data-[state=inactive]:hidden"
                    >
                      <div className="flex shrink-0 items-center justify-between gap-2 text-caption">
                        <span className="font-medium text-ink-secondary">
                          JSON-RPC Request 载荷
                        </span>
                        <div className="flex items-center gap-1.5 font-mono text-micro tabular-nums text-muted-foreground">
                          {selectedTrace.request_truncated && (
                            <MetricPill tone="warn">16 KiB 上限截断</MetricPill>
                          )}
                          <span>{selectedTrace.request_bytes} B</span>
                        </div>
                      </div>
                      <CodeWell
                        code={selectedTrace.request_json}
                        copyValue={selectedTrace.request_json}
                        copyLabel="复制 Request 载荷"
                        copiedLabel="已复制 Request"
                        wrapLines
                        className="flex min-h-0 flex-1 flex-col"
                        scrollAreaClassName="min-h-0 flex-1"
                        viewportClassName="h-full"
                      />
                    </TabsContent>

                    <TabsContent
                      value="response"
                      className="mt-0 flex min-h-0 min-w-0 flex-1 flex-col gap-1.5 overflow-hidden p-3 data-[state=inactive]:hidden"
                    >
                      <div className="flex shrink-0 items-center justify-between gap-2 text-caption">
                        <span className="font-medium text-ink-secondary">
                          JSON-RPC Response 载荷
                        </span>
                        <div className="flex items-center gap-1.5 font-mono text-micro tabular-nums text-muted-foreground">
                          {selectedTrace.response_truncated && (
                            <MetricPill tone="warn">16 KiB 上限截断</MetricPill>
                          )}
                          <span>{selectedTrace.response_bytes} B</span>
                        </div>
                      </div>
                      <CodeWell
                        code={selectedTrace.response_json}
                        copyValue={selectedTrace.response_json}
                        copyLabel="复制 Response 载荷"
                        copiedLabel="已复制 Response"
                        wrapLines
                        className="flex min-h-0 flex-1 flex-col"
                        scrollAreaClassName="min-h-0 flex-1"
                        viewportClassName="h-full"
                      />
                    </TabsContent>

                    <TabsContent
                      value="meta"
                      className="mt-0 min-h-0 min-w-0 flex-1 overflow-hidden p-3 data-[state=inactive]:hidden"
                    >
                      <ScrollArea className="h-full">
                        <div className="flex flex-col gap-3">
                          <dl className="grid grid-cols-2 gap-x-4 gap-y-2.5 rounded-md border border-border-subtle bg-surface-sunken p-3 text-caption">
                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">帧序号</dt>
                              <dd className="font-mono tabular-nums text-foreground">
                                #{selectedTrace.seq}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">时间戳</dt>
                              <dd className="font-mono tabular-nums text-foreground">
                                <TruncatedText
                                  text={selectedTrace.timestamp_iso}
                                  mono
                                />
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                传输通道 / 方向
                              </dt>
                              <dd className="font-mono text-foreground">
                                {selectedTrace.transport} ·{" "}
                                {selectedTrace.direction}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">目标端</dt>
                              <dd className="truncate font-mono text-foreground">
                                {selectedTrace.target}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                JSON-RPC ID
                              </dt>
                              <dd className="font-mono tabular-nums text-foreground">
                                {selectedTrace.jsonrpc_id ?? "—"}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                HTTP 状态码
                              </dt>
                              <dd className="font-mono tabular-nums text-foreground">
                                {selectedTrace.status_code !== null
                                  ? selectedTrace.status_code
                                  : "—"}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                审批决策
                              </dt>
                              <dd className="font-mono text-foreground">
                                {selectedTrace.approval_outcome}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                关联调用 ID
                              </dt>
                              <dd className="font-mono tabular-nums text-foreground">
                                {selectedTrace.linked_call_id !== null
                                  ? `#${selectedTrace.linked_call_id}`
                                  : "—"}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                会话 ID
                              </dt>
                              <dd className="min-w-0">
                                {selectedTrace.session_id ? (
                                  <TruncatedText
                                    text={selectedTrace.session_id}
                                    mono
                                    className="text-micro text-foreground"
                                  />
                                ) : (
                                  <span className="font-mono text-muted-foreground">
                                    —
                                  </span>
                                )}
                              </dd>
                            </div>

                            <div className="flex min-w-0 flex-col gap-0.5">
                              <dt className="text-muted-foreground">
                                调用方标识
                              </dt>
                              <dd className="min-w-0">
                                {selectedTrace.caller_hint ? (
                                  <TruncatedText
                                    text={selectedTrace.caller_hint}
                                    mono
                                    className="text-micro text-foreground"
                                  />
                                ) : (
                                  <span className="font-mono text-muted-foreground">
                                    —
                                  </span>
                                )}
                              </dd>
                            </div>
                          </dl>

                          {/* Active Streamable HTTP Sessions */}
                          <div className="flex flex-col gap-1.5">
                            <div className="flex items-center justify-between text-caption">
                              <span className="font-medium text-ink-secondary">
                                活跃 Streamable HTTP 会话
                              </span>
                              <MetricPill
                                tone={
                                  activeSessions.length > 0 ? "ok" : "subtle"
                                }
                              >
                                {activeSessions.length} 个会话
                              </MetricPill>
                            </div>

                            {activeSessions.length === 0 ? (
                              <div className="rounded-md border border-dashed border-border bg-surface-sunken/55 px-3 py-2.5 text-caption text-muted-foreground">
                                当前无活跃 Streamable HTTP 会话。
                              </div>
                            ) : (
                              <div className="divide-y divide-border-subtle rounded-md border border-border-subtle bg-surface-sunken">
                                {activeSessions.map((session) => (
                                  <div
                                    key={session.session_id}
                                    className="flex flex-col gap-1 px-3 py-2 text-caption"
                                  >
                                    <div className="flex items-center justify-between gap-2">
                                      <div className="flex min-w-0 items-center gap-1.5">
                                        <StatusDot tone="ok" />
                                        <span className="truncate font-mono font-medium text-foreground">
                                          {session.client_name
                                            ? `${session.client_name}${session.client_version ? ` ${session.client_version}` : ""}`
                                            : session.user_agent ??
                                              "Unknown MCP Client"}
                                        </span>
                                        {session.protocol_version && (
                                          <MetricPill tone="subtle">
                                            MCP {session.protocol_version}
                                          </MetricPill>
                                        )}
                                      </div>

                                      <span className="shrink-0 font-mono text-micro tabular-nums text-muted-foreground">
                                        {session.request_count} reqs ·{" "}
                                        {formatTimeHms(
                                          session.last_active_at_iso,
                                        )}
                                      </span>
                                    </div>

                                    <TruncatedText
                                      text={session.session_id}
                                      mono
                                      muted
                                      className="text-micro"
                                    />
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        </div>
                      </ScrollArea>
                    </TabsContent>
                  </Tabs>
                </>
              )}
            </div>
          </div>
        </TooltipProvider>
      </SheetContent>
    </Sheet>
  );
}
