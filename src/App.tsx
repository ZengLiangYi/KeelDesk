import { useEffect, useMemo, useState } from "react";
import {
  ActivityIcon,
  AlertCircleIcon,
  CheckCircle2Icon,
  KeyRoundIcon,
  LayersIcon,
  SendHorizontalIcon,
  ShieldAlertIcon,
  ShieldCheckIcon,
  ShieldXIcon,
  TerminalIcon,
  XIcon,
} from "lucide-react";
import {
  commands,
  type KeelServerStatus,
  type Win11HostSummary,
} from "@/bindings";
import { DevEnvPanel } from "@/components/DevEnvPanel";
import { PortsGovernancePanel } from "@/components/PortsGovernancePanel";
import { ToolApprovalDialog } from "@/components/ToolApprovalDialog";
import { WorkbenchSkeleton } from "@/components/WorkbenchSkeleton";
import {
  CodeWell,
  CopyButton,
  EmptyState,
  MetricPill,
  SafetyTierBadge,
  StatusBadge,
  TruncatedText,
  formatTimeHms,
  serverStatusLabel,
  serverStatusToTone,
} from "@/components/ds";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { isTauriRuntime, useKeelRuntime, useToolApproval } from "@/keel-sdk";
import { cn } from "@/lib/utils";
import "./App.css";

type ConnectClientTab = "claude-code" | "cursor" | "powershell";
type WorkbenchTab = "tools" | "ports" | "dev-env";

interface DrillFeedbackNotice {
  tone: "approved" | "blocked" | "info" | "error";
  text: string;
}

const FALLBACK_HOST_SUMMARY: Win11HostSummary = {
  edition: "Windows 11 Pro",
  display_version: "24H2",
  build_label: "26300.0",
  hostname: "LOCALHOST",
  arch: "x86_64",
  is_windows_11: true,
};

function App() {
  const {
    snapshot,
    isLoading,
    isPinging,
    lastPingResult,
    pingError,
    runSelfTestPing,
  } = useKeelRuntime();

  const {
    pendingApprovals,
    activeApproval,
    sessionAllowlist,
    isSubmitting: isApprovalSubmitting,
    error: approvalError,
    approveOnce,
    approveForSession,
    deny,
    clearSessionAllowlist,
    simulatePreviewApproval,
  } = useToolApproval(snapshot);

  const [hostSummary, setHostSummary] = useState<Win11HostSummary | null>(null);
  const [isHostLoading, setIsHostLoading] = useState(true);
  const [echoDraft, setEchoDraft] = useState("hello-win11");
  const [activeClientTab, setActiveClientTab] =
    useState<ConnectClientTab>("claude-code");
  const [activeWorkbenchTab, setActiveWorkbenchTab] =
    useState<WorkbenchTab>("tools");
  const [isDrilling, setIsDrilling] = useState(false);
  const [drillFeedback, setDrillFeedback] =
    useState<DrillFeedbackNotice | null>(null);

  useEffect(() => {
    // Drop inline critical boot styles once React + Tailwind stylesheet take over
    document.getElementById("keel-critical-boot")?.remove();

    if (!isTauriRuntime()) {
      setHostSummary(FALLBACK_HOST_SUMMARY);
      setIsHostLoading(false);
      return;
    }

    let cancelled = false;
    void commands
      .getWin11HostSummary()
      .then((summary) => {
        if (!cancelled) {
          setHostSummary(summary);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setHostSummary(FALLBACK_HOST_SUMMARY);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsHostLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const endpointUrl = snapshot?.endpoint_url ?? "http://127.0.0.1:19527/mcp";
  const healthzUrl = useMemo(
    () => endpointUrl.replace(/\/mcp$/, "/healthz"),
    [endpointUrl],
  );

  const snippets = useMemo<Record<ConnectClientTab, string>>(
    () => ({
      "claude-code": `claude mcp add --transport http keeldesk ${endpointUrl}`,
      cursor: `{
  "mcpServers": {
    "keeldesk": { "url": "${endpointUrl}" }
  }
}`,
      powershell: `Invoke-RestMethod -Uri "${healthzUrl}"`,
    }),
    [endpointUrl, healthzUrl],
  );

  async function handleRunApprovalDrill(
    port: number | null,
    reason: string,
    processName?: string,
  ) {
    setIsDrilling(true);
    setDrillFeedback(null);

    try {
      if (!isTauriRuntime()) {
        const previewRes = await simulatePreviewApproval({
          port,
          processName,
          reason,
        });
        const targetLabel =
          previewRes.process_names.join(", ") || "无活跃监听进程";
        setDrillFeedback({
          tone: "approved",
          text: `演练放行通过（dry-run 未终止进程）· 端口 :${previewRes.port} · 目标 ${targetLabel}`,
        });
        return;
      }

      const res = await commands.drillKillPortApproval(port, reason);
      if (res.status === "ok") {
        const targetLabel =
          res.data.process_names.join(", ") || "无活跃监听进程";
        setDrillFeedback({
          tone: "approved",
          text: `演练放行通过（dry-run 未终止进程）· 端口 :${res.data.port} · 目标 ${targetLabel}`,
        });
      } else if (res.error.includes("User denied")) {
        setDrillFeedback({
          tone: "blocked",
          text: `高危拦截生效 · 已人工拒绝${port ? `端口 :${port} 的 ` : " "}kill_port_process 调用`,
        });
      } else if (res.error.includes("timed out")) {
        setDrillFeedback({
          tone: "blocked",
          text: "高危拦截生效 · 审批倒计时到期，系统已自动拒绝执行",
        });
      } else {
        setDrillFeedback({
          tone: "error",
          text: `演练调用异常：${res.error}`,
        });
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message.includes("User denied")) {
        setDrillFeedback({
          tone: "blocked",
          text: `高危拦截生效 · 已人工拒绝${port ? `端口 :${port} 的 ` : " "}kill_port_process 调用`,
        });
      } else if (message.includes("timed out")) {
        setDrillFeedback({
          tone: "blocked",
          text: "高危拦截生效 · 审批倒计时到期，系统已自动拒绝执行",
        });
      } else {
        setDrillFeedback({
          tone: "error",
          text: `演练调用异常：${message}`,
        });
      }
    } finally {
      setIsDrilling(false);
    }
  }

  async function handleRevokeSessionAllowlist() {
    const ok = await clearSessionAllowlist();
    if (ok) {
      setDrillFeedback({
        tone: "info",
        text: "已撤销本会话全部免审白名单，后续高危工具调用将重新触发人工确认",
      });
    }
  }

  const isBootstrapping = (isLoading && snapshot === null) || isHostLoading;
  if (isBootstrapping) {
    return <WorkbenchSkeleton />;
  }

  const serverStatus: KeelServerStatus = snapshot?.status ?? "starting";
  const activeSnippet = snippets[activeClientTab];
  const activeAlertMessage = snapshot?.error_message ?? pingError;
  const toolCount = snapshot?.tools.length ?? 4;

  return (
    <TooltipProvider delayDuration={220}>
      <ToolApprovalDialog
        activeApproval={activeApproval}
        pendingCount={pendingApprovals.length}
        isSubmitting={isApprovalSubmitting}
        error={approvalError}
        onApproveOnce={approveOnce}
        onApproveForSession={approveForSession}
        onDeny={deny}
      />

      <div className="flex h-dvh w-full flex-col overflow-clip bg-background text-foreground">
        {/* Top Workbench Command Bar */}
        <header className="shrink-0 border-b border-border-subtle bg-surface-panel px-4 py-2.5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <div className="flex size-8 shrink-0 items-center justify-center rounded-md border border-primary-border bg-primary-subtle">
                <img
                  src="/logo.svg"
                  alt=""
                  width={18}
                  height={18}
                  aria-hidden="true"
                  className="size-4.5 object-contain dark:invert"
                />
              </div>

              <div className="flex min-w-0 flex-col gap-1">
                <div className="flex flex-wrap items-center gap-2">
                  <h1 className="text-title font-semibold text-foreground">
                    龙骨内嵌 MCP 服务工作台
                  </h1>
                  <StatusBadge tone={serverStatusToTone(serverStatus)}>
                    {serverStatusLabel(snapshot?.status)}
                  </StatusBadge>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Badge
                        variant="outline"
                        tabIndex={0}
                        className="gap-1 font-normal text-muted-foreground outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50"
                      >
                        <ShieldCheckIcon aria-hidden="true" />
                        Loopback 防重绑定
                      </Badge>
                    </TooltipTrigger>
                    <TooltipContent side="bottom">
                      仅监听 127.0.0.1 并校验 Host / Origin 白名单，阻断跨站 DNS
                      Rebinding 请求
                    </TooltipContent>
                  </Tooltip>
                </div>

                <div className="flex min-w-0 flex-wrap items-center gap-2 text-caption text-muted-foreground">
                  <span className="select-none">本地端点</span>
                  <code className="truncate rounded-xs border border-border-subtle bg-surface-sunken px-1.5 py-0.5 font-mono text-caption text-foreground select-all">
                    {endpointUrl}
                  </code>
                  <CopyButton
                    value={endpointUrl}
                    label="复制 MCP 端点地址"
                    copiedLabel="已复制端点 URL"
                    side="bottom"
                  />
                </div>
              </div>
            </div>

            <div className="flex shrink-0 flex-wrap items-center gap-2">
              {sessionAllowlist.length > 0 && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="font-mono tabular-nums"
                      onClick={() => void handleRevokeSessionAllowlist()}
                    >
                      <KeyRoundIcon
                        data-icon="inline-start"
                        className="text-primary"
                      />
                      已免审 {sessionAllowlist.length} 项
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom" className="max-w-sm">
                    <div className="flex flex-col gap-1">
                      <span className="font-medium">
                        当前会话免审白名单（点击立即撤销）：
                      </span>
                      {sessionAllowlist.map((entry) => (
                        <code
                          key={`${entry.session_scope}:${entry.tool_name}`}
                          className="font-mono text-micro"
                        >
                          {entry.tool_name} @ {entry.session_scope}
                        </code>
                      ))}
                    </div>
                  </TooltipContent>
                </Tooltip>
              )}

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={isDrilling || serverStatus !== "running"}
                    onClick={() =>
                      void handleRunApprovalDrill(
                        null,
                        "桌面端人工确认网关演练 (dry-run)",
                      )
                    }
                  >
                    <ShieldAlertIcon
                      data-icon="inline-start"
                      className="text-status-warn"
                    />
                    {isDrilling ? "等待确认..." : "演练高危拦截"}
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  通过本地回环发起 kill_port_process (dry_run: true) 调用，安全演练人工审批弹窗
                </TooltipContent>
              </Tooltip>

              <Separator
                orientation="vertical"
                className="hidden h-5 sm:block"
              />

              <form
                className="flex shrink-0 items-center gap-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  void runSelfTestPing(echoDraft);
                }}
              >
                <Input
                  inputSize="sm"
                  value={echoDraft}
                  onChange={(event) => setEchoDraft(event.currentTarget.value)}
                  placeholder="输入回显测试载荷..."
                  aria-label="keel_ping 回显测试载荷"
                  className="w-40 font-mono"
                />
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="submit"
                      size="sm"
                      disabled={isPinging || serverStatus !== "running"}
                    >
                      <SendHorizontalIcon data-icon="inline-start" />
                      {isPinging ? "握手中..." : "回环自检"}
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    通过真实 HTTP 客户端执行 initialize 与 keel_ping 工具调用
                  </TooltipContent>
                </Tooltip>
              </form>
            </div>
          </div>

          {/* Persistent polite live region for approval drill & allowlist status feedback */}
          <div role="status" aria-live="polite" className="min-w-0">
            {drillFeedback && (
              <div
                className={cn(
                  "mt-2.5 flex items-center justify-between gap-2 rounded-md border px-3 py-1.5 text-caption",
                  drillFeedback.tone === "approved" &&
                    "border-status-ok-border bg-status-ok-subtle text-foreground",
                  drillFeedback.tone === "blocked" &&
                    "border-status-danger-border bg-status-danger-subtle text-foreground",
                  drillFeedback.tone === "info" &&
                    "border-status-info-border bg-status-info-subtle text-ink-secondary",
                  drillFeedback.tone === "error" &&
                    "border-status-danger-border bg-status-danger-subtle text-status-danger",
                )}
              >
                <div className="flex min-w-0 items-center gap-2">
                  {drillFeedback.tone === "approved" && (
                    <CheckCircle2Icon
                      aria-hidden="true"
                      className="size-3.5 shrink-0 text-status-ok"
                    />
                  )}
                  {drillFeedback.tone === "blocked" && (
                    <ShieldXIcon
                      aria-hidden="true"
                      className="size-3.5 shrink-0 text-status-danger"
                    />
                  )}
                  {drillFeedback.tone === "info" && (
                    <ShieldCheckIcon
                      aria-hidden="true"
                      className="size-3.5 shrink-0 text-status-info"
                    />
                  )}
                  {drillFeedback.tone === "error" && (
                    <AlertCircleIcon
                      aria-hidden="true"
                      className="size-3.5 shrink-0 text-status-danger"
                    />
                  )}
                  <span className="truncate font-mono text-caption">
                    {drillFeedback.text}
                  </span>
                </div>

                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label="关闭演练反馈提示"
                  onClick={() => setDrillFeedback(null)}
                >
                  <XIcon />
                </Button>
              </div>
            )}
          </div>

          {activeAlertMessage && (
            <Alert variant="destructive" className="mt-2.5">
              <AlertCircleIcon />
              <AlertTitle>
                {snapshot?.error_message ? "MCP 监听异常" : "回环自检失败"}
              </AlertTitle>
              <AlertDescription className="font-mono text-caption">
                {activeAlertMessage}
              </AlertDescription>
            </Alert>
          )}
        </header>

        {/* Main Two-Column Workbench (strictly viewport-bounded, zero page scroll) */}
        <main className="grid min-h-0 flex-1 grid-cols-12 gap-3 p-3">
          {/* Left Column: Client Integration Tabs + Tabbed Host Workbench */}
          <div className="col-span-7 flex min-h-0 min-w-0 flex-col gap-3">
            <Card size="sm" className="shrink-0">
              <CardHeader className="border-b">
                <CardTitle>
                  <h2>外部客户端接入</h2>
                </CardTitle>
                <CardDescription>
                  通过 Streamable HTTP 将本地智能体挂载到当前单实例守护进程
                </CardDescription>
                <CardAction>
                  <CopyButton
                    value={activeSnippet}
                    label="复制代码"
                    copiedLabel="已复制命令"
                    variant="secondary"
                    size="xs"
                    showText
                    side="bottom"
                  />
                </CardAction>
              </CardHeader>

              <CardContent>
                <Tabs
                  value={activeClientTab}
                  onValueChange={(value) =>
                    setActiveClientTab(value as ConnectClientTab)
                  }
                  className="gap-2.5"
                >
                  <div className="border-b border-border-subtle pb-1">
                    <TabsList
                      variant="line"
                      aria-label="客户端配置格式"
                      className="w-full flex-nowrap justify-start overflow-x-auto"
                    >
                      <TabsTrigger
                        value="claude-code"
                        className="flex-none px-2.5 text-caption"
                      >
                        Claude Code
                      </TabsTrigger>
                      <TabsTrigger
                        value="cursor"
                        className="flex-none px-2.5 text-caption"
                      >
                        Cursor · mcp.json
                      </TabsTrigger>
                      <TabsTrigger
                        value="powershell"
                        className="flex-none px-2.5 text-caption"
                      >
                        PowerShell 探针
                      </TabsTrigger>
                    </TabsList>
                  </div>

                  {(
                    ["claude-code", "cursor", "powershell"] as const
                  ).map((tabKey) => (
                    <TabsContent key={tabKey} value={tabKey} className="mt-0">
                      <CodeWell
                        code={snippets[tabKey]}
                        viewportClassName="h-24"
                      />
                    </TabsContent>
                  ))}
                </Tabs>
              </CardContent>
            </Card>

            {/* Bottom-Left Tabbed Workbench Card */}
            <Card size="sm" className="flex min-h-0 min-w-0 flex-1 flex-col">
              <Tabs
                defaultValue="tools"
                value={activeWorkbenchTab}
                onValueChange={(value) =>
                  setActiveWorkbenchTab(value as WorkbenchTab)
                }
                className="flex min-h-0 min-w-0 flex-1 flex-col gap-0"
              >
                <CardHeader className="shrink-0 border-b">
                  <CardTitle className="min-w-0 truncate">
                    <h2 id="workbench-host-heading">Windows 11 宿主治理工作台</h2>
                  </CardTitle>
                  <CardDescription className="min-w-0 truncate">
                    MCP 契约元数据、Win32 活跃监听端口与本机开发工具链观测
                  </CardDescription>
                  <CardAction>
                    <TabsList aria-label="宿主工作台视图切换">
                      <TabsTrigger
                        value="tools"
                        className="px-2.5 text-caption tabular-nums"
                      >
                        工具契约 ({toolCount})
                      </TabsTrigger>
                      <TabsTrigger value="ports" className="px-2.5 text-caption">
                        端口治理
                      </TabsTrigger>
                      <TabsTrigger value="dev-env" className="px-2.5 text-caption">
                        本机环境
                      </TabsTrigger>
                    </TabsList>
                  </CardAction>
                </CardHeader>

                <TabsContent
                  value="tools"
                  className="mt-0 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden data-[state=inactive]:hidden"
                >
                  <CardContent className="min-h-0 min-w-0 flex-1 overflow-hidden p-0">
                    <ScrollArea className="h-full">
                      <div
                        role="region"
                        aria-labelledby="workbench-host-heading"
                        tabIndex={0}
                        className="outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
                      >
                        <Table className="table-fixed">
                          <TableHeader className="sticky top-0 z-(--z-sticky) bg-surface-subtle">
                            <TableRow className="hover:bg-transparent">
                              <TableHead scope="col" className="w-36 pl-3">
                                工具名
                              </TableHead>
                              <TableHead scope="col" className="w-24">
                                归属层
                              </TableHead>
                              <TableHead scope="col" className="w-32">
                                安全策略
                              </TableHead>
                              <TableHead scope="col" className="pr-3">
                                功能说明
                              </TableHead>
                            </TableRow>
                          </TableHeader>
                          <TableBody>
                            {(snapshot?.tools ?? []).map((tool) => (
                              <TableRow key={tool.name}>
                                <TableHead
                                  scope="row"
                                  className="truncate pl-3 font-mono text-caption font-medium text-foreground"
                                >
                                  {tool.name}
                                </TableHead>
                                <TableCell>
                                  <MetricPill tone="subtle">
                                    {tool.source}
                                  </MetricPill>
                                </TableCell>
                                <TableCell>
                                  <SafetyTierBadge
                                    safety={tool.safety}
                                    mode="compact"
                                  />
                                </TableCell>
                                <TableCell className="truncate pr-3 text-caption text-muted-foreground">
                                  <TruncatedText
                                    text={tool.description}
                                    muted
                                  />
                                </TableCell>
                              </TableRow>
                            ))}
                          </TableBody>
                        </Table>
                      </div>
                    </ScrollArea>
                  </CardContent>

                  <CardFooter className="shrink-0 justify-between gap-2">
                    <span className="inline-flex min-w-0 items-center gap-1.5 truncate">
                      <LayersIcon
                        aria-hidden="true"
                        className="size-3.5 shrink-0 text-primary"
                      />
                      <span className="truncate">
                        keel_core 与 workbench 单向解耦边界已锁定
                      </span>
                    </span>
                    <span className="inline-flex shrink-0 items-center gap-1 font-mono text-micro">
                      <ShieldCheckIcon
                        aria-hidden="true"
                        className="size-3.5 text-status-ok"
                      />
                      ConfirmRequired 人工审批保护生效中
                    </span>
                  </CardFooter>
                </TabsContent>

                <TabsContent
                  value="ports"
                  forceMount
                  className="mt-0 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden data-[state=inactive]:hidden"
                >
                  <PortsGovernancePanel
                    isActive={activeWorkbenchTab === "ports"}
                    isServerRunning={serverStatus === "running"}
                    isDrilling={isDrilling}
                    onDrillPort={(port, processName) =>
                      void handleRunApprovalDrill(
                        port,
                        `端口 :${port} (${processName}) 人工确认网关演练 (dry-run)`,
                        processName,
                      )
                    }
                  />
                </TabsContent>

                <TabsContent
                  value="dev-env"
                  forceMount
                  className="mt-0 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden data-[state=inactive]:hidden"
                >
                  <DevEnvPanel isActive={activeWorkbenchTab === "dev-env"} />
                </TabsContent>
              </Tabs>
            </Card>
          </div>

          {/* Right Column: Live MCP Call Stream */}
          <Card
            size="sm"
            className="col-span-5 flex min-h-0 min-w-0 flex-col"
          >
            <CardHeader className="border-b">
              <CardTitle>
                <h2 className="flex items-center gap-2">
                  <ActivityIcon
                    aria-hidden="true"
                    className="size-4 text-primary"
                  />
                  <span>实时调用观测流</span>
                </h2>
              </CardTitle>
              <CardDescription>
                按时间倒序展示最近 MCP 会话工具调用耗时与载荷摘要
              </CardDescription>
              <CardAction>
                <div className="flex items-center gap-1.5">
                  {lastPingResult && !pingError && (
                    <MetricPill tone="ok">
                      {lastPingResult.roundtrip_ms} ms
                    </MetricPill>
                  )}
                  <MetricPill tone="neutral">
                    累计 {snapshot?.total_calls ?? 0}
                  </MetricPill>
                </div>
              </CardAction>
            </CardHeader>

            <CardContent className="flex min-h-0 flex-1 flex-col p-0">
              {(snapshot?.recent_calls.length ?? 0) === 0 ? (
                <div className="flex flex-1 p-3">
                  <EmptyState
                    icon={<TerminalIcon />}
                    title="尚未捕获 tools/call 调用记录"
                    description="点击下方按钮发起一次真实本地回环握手，或通过左侧配置让外部客户端连接。"
                    action={
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        disabled={isPinging || serverStatus !== "running"}
                        onClick={() => void runSelfTestPing(echoDraft)}
                      >
                        <SendHorizontalIcon data-icon="inline-start" />
                        发送首条 keel_ping 自检
                      </Button>
                    }
                  />
                </div>
              ) : (
                <ScrollArea className="min-h-0 flex-1 overflow-hidden">
                  <div className="divide-y divide-border-subtle">
                    {snapshot?.recent_calls.map((entry) => (
                      <div
                        key={entry.id}
                        className="flex flex-col gap-1 px-3 py-2.5 transition-colors duration-150 ease-(--ease-out-quart) hover:bg-surface-hover/65"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex min-w-0 items-center gap-1.5">
                            <span className="font-mono text-micro tabular-nums text-muted-foreground">
                              #{entry.id}
                            </span>
                            <span className="truncate font-mono text-caption font-medium text-foreground">
                              {entry.tool_name}
                            </span>
                            {entry.caller_hint && (
                              <MetricPill
                                tone="subtle"
                                className="max-w-36 truncate text-muted-foreground"
                              >
                                {entry.caller_hint}
                              </MetricPill>
                            )}
                          </div>
                          <div className="flex shrink-0 items-center gap-2 font-mono text-micro tabular-nums text-muted-foreground">
                            <span
                              className={
                                entry.succeeded
                                  ? "text-status-ok"
                                  : "text-status-danger"
                              }
                            >
                              {entry.duration_ms} ms
                            </span>
                            <span>{formatTimeHms(entry.timestamp_iso)}</span>
                          </div>
                        </div>
                        <TruncatedText
                          text={entry.summary}
                          mono
                          muted
                          className="text-micro"
                        />
                      </div>
                    ))}
                  </div>
                </ScrollArea>
              )}
            </CardContent>
          </Card>
        </main>

        {/* Windows 11 Host & Singleton Tray Status Bar */}
        <footer className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border-subtle bg-surface-subtle px-4 text-caption text-muted-foreground select-none">
          <div className="flex min-w-0 items-center gap-2 truncate">
            <span className="font-medium text-ink-secondary">
              {hostSummary
                ? `${hostSummary.edition} ${hostSummary.display_version}`
                : "Windows 11"}
            </span>
            {hostSummary && (
              <>
                <Separator orientation="vertical" className="my-1.5" />
                <span className="font-mono tabular-nums">
                  Build {hostSummary.build_label}
                </span>
                <Separator orientation="vertical" className="my-1.5" />
                <span className="font-mono">{hostSummary.arch}</span>
                <Separator orientation="vertical" className="my-1.5" />
                <span className="truncate font-mono">
                  {hostSummary.hostname}
                </span>
              </>
            )}
          </div>

          <div className="hidden shrink-0 items-center gap-2 sm:flex">
            <span>关闭窗口自动收起至系统托盘（MCP 持续常驻）</span>
            <Separator orientation="vertical" className="my-1.5" />
            <span className="font-mono tabular-nums">
              v{snapshot?.server_version ?? "0.1.0"}
            </span>
          </div>
        </footer>
      </div>
    </TooltipProvider>
  );
}

export default App;
