import { useEffect, useMemo, useRef, useState } from "react";
import {
  ActivityIcon,
  AlertCircleIcon,
  CheckIcon,
  CopyIcon,
  LayersIcon,
  SendHorizontalIcon,
  ShieldCheckIcon,
  TerminalIcon,
} from "lucide-react";
import {
  commands,
  type KeelServerStatus,
  type Win11HostSummary,
} from "@/bindings";
import { WorkbenchSkeleton } from "@/components/WorkbenchSkeleton";
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
import { useKeelRuntime } from "@/keel-sdk/useKeelRuntime";
import { cn } from "@/lib/utils";
import "./App.css";

type ConnectClientTab = "claude-code" | "cursor" | "powershell";

const FALLBACK_HOST_SUMMARY: Win11HostSummary = {
  edition: "Windows 11 Pro",
  display_version: "24H2",
  build_label: "26300.0",
  hostname: "LOCALHOST",
  arch: "x86_64",
  is_windows_11: true,
};

function statusLabel(status: KeelServerStatus | undefined): string {
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

function formatTimeHms(iso: string): string {
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.getTime())) {
    return "--:--:--";
  }
  return parsed.toLocaleTimeString("zh-CN", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function StatusDot({ status }: { status: KeelServerStatus }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "size-2 shrink-0 rounded-full",
        status === "running" && "bg-(--keel-status-ok)",
        status === "starting" && "bg-(--keel-status-warn)",
        (status === "error" || status === "stopped") && "bg-(--keel-status-err)",
      )}
    />
  );
}

function App() {
  const {
    snapshot,
    isLoading,
    isPinging,
    lastPingResult,
    pingError,
    runSelfTestPing,
  } = useKeelRuntime();

  const [hostSummary, setHostSummary] = useState<Win11HostSummary | null>(null);
  const [isHostLoading, setIsHostLoading] = useState(true);
  const [echoDraft, setEchoDraft] = useState("hello-win11");
  const [activeClientTab, setActiveClientTab] =
    useState<ConnectClientTab>("claude-code");
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const copyResetTimer = useRef<number | null>(null);

  useEffect(() => {
    // Drop inline critical boot styles once React + Tailwind stylesheet take over
    document.getElementById("keel-critical-boot")?.remove();

    const inTauri =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

    if (!inTauri) {
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

  useEffect(() => {
    return () => {
      if (copyResetTimer.current !== null) {
        window.clearTimeout(copyResetTimer.current);
      }
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

  async function copyText(key: string, value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedKey(key);
      if (copyResetTimer.current !== null) {
        window.clearTimeout(copyResetTimer.current);
      }
      copyResetTimer.current = window.setTimeout(() => {
        setCopiedKey(null);
        copyResetTimer.current = null;
      }, 1600);
    } catch {
      // Clipboard unavailable in restricted context
    }
  }

  const isBootstrapping = (isLoading && snapshot === null) || isHostLoading;
  if (isBootstrapping) {
    return <WorkbenchSkeleton />;
  }

  const serverStatus: KeelServerStatus = snapshot?.status ?? "starting";
  const activeSnippet = snippets[activeClientTab];
  const activeAlertMessage = snapshot?.error_message ?? pingError;

  return (
    <TooltipProvider delayDuration={220}>
      <div className="flex h-dvh w-full flex-col overflow-clip bg-background text-foreground">
        {/* Top Workbench Command Bar */}
        <header className="shrink-0 border-b border-border bg-card/65 px-5 py-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <div className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/15 ring-1 ring-primary/30">
                <img
                  src="/logo.svg"
                  alt=""
                  width={20}
                  height={20}
                  aria-hidden="true"
                  className="size-5 object-contain dark:invert"
                />
              </div>

              <div className="flex min-w-0 flex-col gap-1">
                <div className="flex flex-wrap items-center gap-2">
                  <h1 className="text-sm font-semibold tracking-tight text-foreground">
                    龙骨内嵌 MCP 服务工作台
                  </h1>
                  <Badge
                    variant="secondary"
                    className="gap-1.5 font-normal select-none"
                  >
                    <StatusDot status={serverStatus} />
                    {statusLabel(snapshot?.status)}
                  </Badge>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Badge
                        variant="outline"
                        className="gap-1 font-normal text-muted-foreground select-none"
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

                <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
                  <span className="select-none">本地端点</span>
                  <code className="truncate rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-foreground select-all">
                    {endpointUrl}
                  </code>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-xs"
                        aria-label={
                          copiedKey === "endpoint"
                            ? "已复制 MCP 端点地址"
                            : "复制 MCP 端点地址"
                        }
                        onClick={() => void copyText("endpoint", endpointUrl)}
                      >
                        {copiedKey === "endpoint" ? (
                          <CheckIcon className="text-(--keel-status-ok)" />
                        ) : (
                          <CopyIcon />
                        )}
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent side="bottom">
                      {copiedKey === "endpoint"
                        ? "已复制端点 URL"
                        : "复制 MCP 端点 URL"}
                    </TooltipContent>
                  </Tooltip>
                </div>
              </div>
            </div>

            <form
              className="flex shrink-0 items-center gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                void runSelfTestPing(echoDraft);
              }}
            >
              <Input
                value={echoDraft}
                onChange={(event) => setEchoDraft(event.currentTarget.value)}
                placeholder="输入回显测试载荷..."
                aria-label="keel_ping 回显测试载荷"
                className="h-8 w-44 font-mono text-xs"
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

          {activeAlertMessage && (
            <Alert variant="destructive" className="mt-3">
              <AlertCircleIcon />
              <AlertTitle>
                {snapshot?.error_message ? "MCP 监听异常" : "回环自检失败"}
              </AlertTitle>
              <AlertDescription className="font-mono text-xs">
                {activeAlertMessage}
              </AlertDescription>
            </Alert>
          )}
        </header>

        {/* Main Two-Column shadcn/ui Workspace (strictly viewport-bounded, zero page scroll) */}
        <main className="grid min-h-0 flex-1 grid-cols-12 gap-4 p-4">
          {/* Left Column: Client Integration Tabs + Registered Tools Table */}
          <div className="col-span-7 flex min-h-0 min-w-0 flex-col gap-4">
            <Card size="sm" className="shrink-0">
              <CardHeader className="border-b">
                <CardTitle>
                  <h2>外部客户端接入</h2>
                </CardTitle>
                <CardDescription>
                  通过 Streamable HTTP 将本地智能体挂载到当前单实例守护进程
                </CardDescription>
                <CardAction>
                  <Button
                    type="button"
                    variant="secondary"
                    size="xs"
                    onClick={() => void copyText("snippet", activeSnippet)}
                  >
                    {copiedKey === "snippet" ? (
                      <CheckIcon data-icon="inline-start" />
                    ) : (
                      <CopyIcon data-icon="inline-start" />
                    )}
                    {copiedKey === "snippet" ? "已复制命令" : "复制代码"}
                  </Button>
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
                  <div className="border-b border-border pb-1">
                    <TabsList
                      variant="line"
                      aria-label="客户端配置格式"
                      className="w-full flex-nowrap justify-start overflow-x-auto"
                    >
                      <TabsTrigger
                        value="claude-code"
                        className="flex-none px-2.5 text-xs"
                      >
                        Claude Code
                      </TabsTrigger>
                      <TabsTrigger
                        value="cursor"
                        className="flex-none px-2.5 text-xs"
                      >
                        Cursor · mcp.json
                      </TabsTrigger>
                      <TabsTrigger
                        value="powershell"
                        className="flex-none px-2.5 text-xs"
                      >
                        PowerShell 探针
                      </TabsTrigger>
                    </TabsList>
                  </div>

                  {(
                    ["claude-code", "cursor", "powershell"] as const
                  ).map((tabKey) => (
                    <TabsContent key={tabKey} value={tabKey} className="mt-0">
                      <pre className="keel-code-scroll m-0 flex h-24 items-center overflow-auto rounded-lg border border-border bg-background px-3 py-2 font-mono text-xs leading-relaxed text-foreground select-all">
                        <code>{snippets[tabKey]}</code>
                      </pre>
                    </TabsContent>
                  ))}
                </Tabs>
              </CardContent>
            </Card>

            <Card size="sm" className="flex min-h-0 flex-1 flex-col">
              <CardHeader className="border-b">
                <CardTitle>
                  <h2 id="mcp-tools-heading">MCP 工具契约注册表</h2>
                </CardTitle>
                <CardDescription>
                  keel_core 底座与 workbench 业务层解耦注册的工具元数据
                </CardDescription>
                <CardAction>
                  <Badge
                    variant="secondary"
                    className="font-mono tabular-nums select-none"
                  >
                    {snapshot?.tools.length ?? 1} 项已注册
                  </Badge>
                </CardAction>
              </CardHeader>

              <CardContent className="min-h-0 flex-1 p-0">
                <ScrollArea className="h-full">
                  <div
                    role="region"
                    aria-labelledby="mcp-tools-heading"
                    tabIndex={0}
                    className="outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
                  >
                    <Table className="table-fixed">
                      <TableHeader className="sticky top-0 z-10 bg-card">
                        <TableRow className="hover:bg-transparent">
                          <TableHead scope="col" className="w-30 pl-3 text-xs">
                            工具名
                          </TableHead>
                          <TableHead scope="col" className="w-24 text-xs">
                            归属层
                          </TableHead>
                          <TableHead scope="col" className="w-32 text-xs">
                            安全策略
                          </TableHead>
                          <TableHead scope="col" className="pr-3 text-xs">
                            功能说明
                          </TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {(snapshot?.tools ?? []).map((tool) => (
                          <TableRow key={tool.name}>
                            <TableHead
                              scope="row"
                              className="truncate pl-3 font-mono text-xs font-medium text-foreground"
                            >
                              {tool.name}
                            </TableHead>
                            <TableCell>
                              <Badge
                                variant="outline"
                                className="font-mono text-[11px]"
                              >
                                {tool.source}
                              </Badge>
                            </TableCell>
                            <TableCell>
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <Badge
                                    variant={
                                      tool.safety === "safe"
                                        ? "secondary"
                                        : "destructive"
                                    }
                                    className="text-[11px]"
                                  >
                                    {tool.safety === "safe"
                                      ? "Safe · 直接放行"
                                      : "Confirm · 需审批"}
                                  </Badge>
                                </TooltipTrigger>
                                <TooltipContent side="top">
                                  {tool.safety === "safe"
                                    ? "只读或无副作用探针，框架直接执行并返回结构化结果"
                                    : "高危操作将挂起调用通道并唤起系统托盘人工确认"}
                                </TooltipContent>
                              </Tooltip>
                            </TableCell>
                            <TableCell className="truncate pr-3 text-xs text-muted-foreground">
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <span
                                    tabIndex={0}
                                    className="block truncate rounded-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                                  >
                                    {tool.description}
                                  </span>
                                </TooltipTrigger>
                                <TooltipContent
                                  side="top"
                                  aria-hidden="true"
                                  className="max-w-sm"
                                >
                                  {tool.description}
                                </TooltipContent>
                              </Tooltip>
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </div>
                </ScrollArea>
              </CardContent>

              <CardFooter className="justify-between py-2 text-xs text-muted-foreground">
                <span className="inline-flex min-w-0 items-center gap-1.5 truncate">
                  <LayersIcon
                    aria-hidden="true"
                    className="size-3.5 shrink-0 text-primary"
                  />
                  <span>workbench 业务隔离层已就绪</span>
                </span>
                <span className="shrink-0 font-mono text-[11px]">
                  业务工具注册槽已就绪
                </span>
              </CardFooter>
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
                    <Badge
                      variant="outline"
                      className="font-mono text-[11px] tabular-nums text-(--keel-status-ok)"
                    >
                      {lastPingResult.roundtrip_ms} ms
                    </Badge>
                  )}
                  <Badge
                    variant="secondary"
                    className="font-mono tabular-nums select-none"
                  >
                    累计 {snapshot?.total_calls ?? 0}
                  </Badge>
                </div>
              </CardAction>
            </CardHeader>

            <CardContent className="flex min-h-0 flex-1 flex-col p-0">
              {(snapshot?.recent_calls.length ?? 0) === 0 ? (
                <div className="m-3 flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border bg-muted/25 p-5 text-center">
                  <div className="flex size-9 items-center justify-center rounded-full bg-muted text-muted-foreground">
                    <TerminalIcon aria-hidden="true" className="size-4" />
                  </div>
                  <div className="flex flex-col gap-1">
                    <p className="text-sm font-medium text-foreground">
                      尚未捕获 tools/call 调用记录
                    </p>
                    <p className="max-w-[36ch] text-xs leading-relaxed text-muted-foreground">
                      点击下方按钮发起一次真实本地回环握手，或通过左侧配置让外部客户端连接。
                    </p>
                  </div>
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
                </div>
              ) : (
                <ScrollArea className="min-h-0 flex-1">
                  <div className="divide-y divide-border">
                    {snapshot?.recent_calls.map((entry) => (
                      <div
                        key={entry.id}
                        className="flex flex-col gap-1 px-3.5 py-2.5 transition-colors hover:bg-muted/35"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex min-w-0 items-center gap-1.5">
                            <span className="font-mono text-[11px] tabular-nums text-muted-foreground">
                              #{entry.id}
                            </span>
                            <span className="truncate font-mono text-xs font-medium text-foreground">
                              {entry.tool_name}
                            </span>
                            {entry.caller_hint && (
                              <Badge
                                variant="outline"
                                className="max-w-36 truncate font-mono text-[10px] text-muted-foreground"
                              >
                                {entry.caller_hint}
                              </Badge>
                            )}
                          </div>
                          <div className="flex shrink-0 items-center gap-2 font-mono text-[11px] tabular-nums text-muted-foreground">
                            <span
                              className={
                                entry.succeeded
                                  ? "text-(--keel-status-ok)"
                                  : "text-destructive"
                              }
                            >
                              {entry.duration_ms} ms
                            </span>
                            <span>{formatTimeHms(entry.timestamp_iso)}</span>
                          </div>
                        </div>
                        <p className="truncate font-mono text-[11px] text-muted-foreground">
                          {entry.summary}
                        </p>
                      </div>
                    ))}
                  </div>
                </ScrollArea>
              )}
            </CardContent>
          </Card>
        </main>

        {/* Windows 11 Host & Singleton Tray Status Bar */}
        <footer className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border bg-card/55 px-5 text-xs text-muted-foreground select-none">
          <div className="flex min-w-0 items-center gap-2 truncate">
            <span className="font-medium text-foreground/90">
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
