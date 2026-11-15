import { useMemo, useState } from "react";
import {
  AlertCircleIcon,
  InfoIcon,
  PlayIcon,
  PlusIcon,
  RefreshCwIcon,
  ServerIcon,
  ShieldCheckIcon,
  SquareIcon,
  Trash2Icon,
  WrenchIcon,
  XIcon,
} from "lucide-react";
import {
  buildBuiltinEchoProbeSpec,
  type ChildEnvVar,
  type ChildServerRecord,
  type ChildToolCallResult,
  type ProcessHostSnapshot,
  type SpawnChildServerSpec,
} from "@keeldesk/react";
import {
  CodeWell,
  EmptyState,
  MetricPill,
  StatusBadge,
  StatusDot,
  TruncatedText,
  childStatusLabel,
  childStatusToTone,
} from "@/components/ds";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { CardFooter } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export interface ProcessHostPanelProps {
  hostSnapshot: ProcessHostSnapshot;
  isLoading: boolean;
  isStarting: boolean;
  busyServerIds: Record<string, boolean>;
  callingServerIds?: Record<string, boolean>;
  error: string | null;
  onStartServer: (
    spec: SpawnChildServerSpec,
  ) => Promise<ChildServerRecord | null>;
  onStopServer: (serverId: string) => Promise<ChildServerRecord | null>;
  onRemoveServer: (serverId: string) => Promise<boolean>;
  onRestartServer: (serverId: string) => Promise<ChildServerRecord | null>;
  onCallTool?: (
    serverId: string,
    toolName: string,
    argumentsJson?: string | null,
  ) => Promise<ChildToolCallResult | null>;
  onInspectTraceSeq?: (traceSeq: number) => void;
  onRefresh: () => Promise<ProcessHostSnapshot | null>;
  onClearError: () => void;
}

const SUNKEN_TEXTAREA_CLASS =
  "w-full resize-y rounded-sm border border-input bg-surface-sunken px-2.5 py-1.5 font-mono text-code text-foreground transition-colors duration-150 ease-(--ease-out-quart) outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50";

function parseArgsInput(raw: string): { args: string[]; error?: string } {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { args: [] };
  }

  if (trimmed.startsWith("[")) {
    try {
      const parsed: unknown = JSON.parse(trimmed);
      if (
        Array.isArray(parsed) &&
        parsed.every((item) => typeof item === "string")
      ) {
        return { args: parsed };
      }
      return { args: [], error: "JSON 参数格式必须为字符串数组。" };
    } catch {
      return { args: [], error: "无法解析 JSON 字符串数组参数。" };
    }
  }

  const tokens: string[] = [];
  let current = "";
  let inToken = false;
  let quote: '"' | "'" | null = null;

  for (let i = 0; i < trimmed.length; i += 1) {
    const ch = trimmed[i];
    if (quote !== null) {
      if (ch === "\\" && quote === '"' && i + 1 < trimmed.length) {
        const next = trimmed[i + 1];
        if (next === '"' || next === "\\") {
          current += next;
          i += 1;
          continue;
        }
      }
      if (ch === quote) {
        quote = null;
      } else {
        current += ch;
      }
      inToken = true;
      continue;
    }

    if (ch === '"' || ch === "'") {
      quote = ch;
      inToken = true;
      continue;
    }

    if (/\s/.test(ch)) {
      if (inToken) {
        tokens.push(current);
        current = "";
        inToken = false;
      }
      continue;
    }

    current += ch;
    inToken = true;
  }

  if (quote !== null) {
    return { args: [], error: "命令参数存在未闭合的引号。" };
  }

  if (inToken) {
    tokens.push(current);
  }

  return { args: tokens };
}

function parseEnvLines(raw: string): { env: ChildEnvVar[]; error?: string } {
  const lines = raw.split(/\r?\n/);
  const env: ChildEnvVar[] = [];

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i].trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    const eqIndex = line.indexOf("=");
    if (eqIndex <= 0) {
      return {
        env: [],
        error: `第 ${i + 1} 行环境变量格式无效，应为 KEY=VALUE。`,
      };
    }
    const key = line.slice(0, eqIndex).trim();
    const value = line.slice(eqIndex + 1);
    if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(key)) {
      return {
        env: [],
        error: `环境变量名 '${key}' 不合法（需匹配 [a-zA-Z_][a-zA-Z0-9_]*）。`,
      };
    }
    env.push({ key, value });
  }

  return { env };
}

function formatCompactCommand(record: ChildServerRecord): string {
  if (record.args.length === 0) {
    return record.command;
  }
  const previewArgs = record.args
    .map((arg) => (arg.includes("\n") ? "<inline-script>" : arg))
    .join(" ");
  return `${record.command} ${previewArgs}`;
}

export function ProcessHostPanel({
  hostSnapshot,
  isLoading,
  isStarting,
  busyServerIds,
  callingServerIds,
  error,
  onStartServer,
  onStopServer,
  onRemoveServer,
  onRestartServer,
  onCallTool,
  onInspectTraceSeq,
  onRefresh,
  onClearError,
}: ProcessHostPanelProps) {
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [inspectedServerId, setInspectedServerId] = useState<string | null>(
    null,
  );
  const [callingToolKey, setCallingToolKey] = useState<string | null>(null);
  const [lastToolCallResult, setLastToolCallResult] =
    useState<ChildToolCallResult | null>(null);

  // Add-dialog form state
  const [formId, setFormId] = useState("");
  const [formName, setFormName] = useState("");
  const [formCommand, setFormCommand] = useState("");
  const [formArgsText, setFormArgsText] = useState("");
  const [formCwd, setFormCwd] = useState("");
  const [formEnvText, setFormEnvText] = useState("");
  const [formHandshake, setFormHandshake] = useState(true);
  const [formError, setFormError] = useState<string | null>(null);

  const inspectedServer = useMemo(
    () =>
      inspectedServerId
        ? (hostSnapshot.servers.find((s) => s.id === inspectedServerId) ??
          null)
        : null,
    [hostSnapshot.servers, inspectedServerId],
  );

  const activeToolCallResult =
    inspectedServer && lastToolCallResult?.server_id === inspectedServer.id
      ? lastToolCallResult
      : null;

  async function handleTestCallTool(
    serverId: string,
    toolName: string,
    argumentsJson: string,
  ) {
    if (!onCallTool) {
      return;
    }
    const key = `${serverId}:${toolName}`;
    setCallingToolKey(key);
    try {
      const result = await onCallTool(serverId, toolName, argumentsJson);
      if (result) {
        setLastToolCallResult(result);
      }
    } finally {
      setCallingToolKey((prev) => (prev === key ? null : prev));
    }
  }

  function resetFormWithPreset(preset: "echo" | "filesystem" | "fetch" | "blank") {
    setFormError(null);
    if (preset === "echo") {
      const spec = buildBuiltinEchoProbeSpec();
      setFormId(spec.id);
      setFormName(spec.name);
      setFormCommand(spec.command);
      setFormArgsText(JSON.stringify(spec.args ?? [], null, 2));
      setFormCwd("");
      setFormEnvText("");
      setFormHandshake(true);
      return;
    }
    if (preset === "filesystem") {
      setFormId("filesystem-mcp");
      setFormName("Filesystem MCP");
      setFormCommand("npx");
      setFormArgsText("-y @modelcontextprotocol/server-filesystem .");
      setFormCwd("");
      setFormEnvText("");
      setFormHandshake(true);
      return;
    }
    if (preset === "fetch") {
      setFormId("fetch-mcp");
      setFormName("Fetch MCP");
      setFormCommand("uvx");
      setFormArgsText("mcp-server-fetch");
      setFormCwd("");
      setFormEnvText("");
      setFormHandshake(true);
      return;
    }
    setFormId("");
    setFormName("");
    setFormCommand("");
    setFormArgsText("");
    setFormCwd("");
    setFormEnvText("");
    setFormHandshake(true);
  }

  function handleOpenAddDialog() {
    if (!formId && !formCommand) {
      resetFormWithPreset("filesystem");
    } else {
      setFormError(null);
    }
    setIsAddOpen(true);
  }

  async function handleSpawnBuiltinEcho() {
    const existingIds = new Set(hostSnapshot.servers.map((s) => s.id));
    let spec = buildBuiltinEchoProbeSpec();
    if (existingIds.has(spec.id)) {
      const existing = hostSnapshot.servers.find((s) => s.id === spec.id);
      if (existing && existing.status === "running") {
        let index = 2;
        while (existingIds.has(`win11-stdio-echo-${index}`)) {
          index += 1;
        }
        spec = buildBuiltinEchoProbeSpec(String(index));
      }
    }
    await onStartServer(spec);
  }

  async function handleSubmitAddForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setFormError(null);

    const trimmedId = formId.trim();
    const trimmedName = formName.trim();
    const trimmedCommand = formCommand.trim();

    if (
      !trimmedId ||
      trimmedId === "." ||
      trimmedId === ".." ||
      !/^[a-zA-Z0-9._-]{1,64}$/.test(trimmedId)
    ) {
      setFormError(
        "服务标识 (ID) 需为 1-64 位字母、数字或 . _ - 字符（不可为 . 或 ..）。",
      );
      return;
    }
    if (!trimmedName) {
      setFormError("请输入显示名称。");
      return;
    }
    if (!trimmedCommand) {
      setFormError("请输入可执行命令（例如 npx、uvx、node 或 powershell.exe）。");
      return;
    }

    const parsedArgs = parseArgsInput(formArgsText);
    if (parsedArgs.error) {
      setFormError(parsedArgs.error);
      return;
    }

    const parsedEnv = parseEnvLines(formEnvText);
    if (parsedEnv.error) {
      setFormError(parsedEnv.error);
      return;
    }

    const record = await onStartServer({
      id: trimmedId,
      name: trimmedName,
      command: trimmedCommand,
      args: parsedArgs.args,
      cwd: formCwd.trim() || null,
      env: parsedEnv.env,
      perform_mcp_handshake: formHandshake,
    });

    if (record) {
      setIsAddOpen(false);
    }
  }

  const servers = hostSnapshot.servers;

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {/* Top Toolbar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border-subtle px-3 py-2">
        <div
          aria-live="polite"
          className="flex flex-wrap items-center gap-1.5"
        >
          <MetricPill tone={hostSnapshot.running_count > 0 ? "ok" : "neutral"}>
            运行中 {hostSnapshot.running_count} / {servers.length}
          </MetricPill>
          <MetricPill tone="subtle" className="text-muted-foreground">
            合并 PATH {hostSnapshot.merged_path_entries} 项
          </MetricPill>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="icon-xs"
                aria-label="刷新子进程托管快照"
                disabled={isLoading}
                onClick={() => void onRefresh()}
              >
                <RefreshCwIcon
                  className={cn(isLoading && "animate-spin")}
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">刷新子进程托管快照</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                size="xs"
                variant="secondary"
                disabled={isStarting}
                onClick={() => void handleSpawnBuiltinEcho()}
              >
                <PlayIcon data-icon="inline-start" />
                {isStarting ? "启动中..." : "启动回显探针"}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              一键拉起内置 PowerShell Stdio MCP 回显探针，验证 Job Object 与协议握手
            </TooltipContent>
          </Tooltip>

          <Button
            type="button"
            size="xs"
            variant="default"
            onClick={handleOpenAddDialog}
          >
            <PlusIcon data-icon="inline-start" />
            添加子进程
          </Button>
        </div>
      </div>

      {/* Host Error Feedback */}
      {error && (
        <div className="shrink-0 px-3 pt-2">
          <Alert variant="destructive">
            <AlertCircleIcon />
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0 flex-1">
                <AlertTitle>子进程托管操作异常</AlertTitle>
                <AlertDescription className="font-mono text-caption">
                  {error}
                </AlertDescription>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="关闭子进程异常提示"
                onClick={onClearError}
              >
                <XIcon />
              </Button>
            </div>
          </Alert>
        </div>
      )}

      {/* Bounded Scrollable Child Servers Area */}
      <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
        {servers.length === 0 ? (
          <div className="flex h-full p-3">
            <EmptyState
              icon={<ServerIcon />}
              title="尚未托管受控 Stdio 子进程"
              description="KeelDesk 通过 CREATE_NO_WINDOW 无黑框派生并通过 Win32 Job Object 绑定进程树，确保退出时级联回收全部子孙进程。"
              action={
                <>
                  <Button
                    type="button"
                    size="sm"
                    variant="default"
                    onClick={handleOpenAddDialog}
                  >
                    <PlusIcon data-icon="inline-start" />
                    添加子进程
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    variant="secondary"
                    disabled={isStarting}
                    onClick={() => void handleSpawnBuiltinEcho()}
                  >
                    <PlayIcon data-icon="inline-start" />
                    启动回显探针
                  </Button>
                </>
              }
            />
          </div>
        ) : (
          <ScrollArea className="h-full">
            <div
              role="region"
              aria-label="Windows 11 受控 Stdio MCP 子进程列表"
              tabIndex={0}
              className="outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
            >
              <Table className="table-fixed">
                <TableHeader className="sticky top-0 z-(--z-sticky) bg-surface-subtle">
                  <TableRow className="hover:bg-transparent">
                    <TableHead scope="col" className="pl-3">
                      服务 / 命令
                    </TableHead>
                    <TableHead scope="col" className="w-32">
                      状态 / PID
                    </TableHead>
                    <TableHead scope="col" className="w-28">
                      MCP 握手
                    </TableHead>
                    <TableHead scope="col" className="w-36 pr-3 text-right">
                      操作
                    </TableHead>
                  </TableRow>
                </TableHeader>

                <TableBody>
                  {servers.map((record) => {
                    const isRowBusy = Boolean(busyServerIds[record.id]);
                    const isActiveState =
                      record.status === "running" ||
                      record.status === "starting";
                    const formattedCommand = formatCompactCommand(record);
                    const commandTooltip = record.resolved_executable
                      ? `${record.resolved_executable}${record.args.length > 0 ? ` (${record.args.length} args)` : ""}`
                      : formattedCommand;

                    return (
                      <TableRow key={record.id}>
                        <TableHead
                          scope="row"
                          className="pl-3 font-normal text-foreground"
                        >
                          <div className="flex min-w-0 flex-col gap-0.5">
                            <div className="flex min-w-0 items-center gap-1.5">
                              <span className="truncate text-caption font-medium text-foreground">
                                {record.name}
                              </span>
                            </div>
                            <TruncatedText
                              text={formattedCommand}
                              tooltip={commandTooltip}
                              mono
                              muted
                              className="text-micro"
                              tooltipClassName="font-mono text-micro"
                            />
                          </div>
                        </TableHead>

                        <TableCell>
                          <div className="flex flex-col items-start gap-1">
                            <StatusBadge tone={childStatusToTone(record.status)}>
                              {childStatusLabel(record.status)}
                            </StatusBadge>
                            {record.pid !== null ? (
                              <span className="font-mono text-micro tabular-nums text-muted-foreground">
                                PID {record.pid}
                              </span>
                            ) : record.last_exit_code !== null ? (
                              <span className="font-mono text-micro tabular-nums text-muted-foreground">
                                Exit {record.last_exit_code}
                              </span>
                            ) : null}
                          </div>
                        </TableCell>

                        <TableCell>
                          {record.perform_mcp_handshake ? (
                            <div className="flex flex-col items-start gap-1">
                              <MetricPill
                                tone={
                                  record.discovered_tools.length > 0
                                    ? "subtle"
                                    : "neutral"
                                }
                              >
                                {record.discovered_tools.length} 工具
                              </MetricPill>
                              {record.handshake_ms !== null && (
                                <span className="font-mono text-micro tabular-nums text-muted-foreground">
                                  {record.handshake_ms} ms
                                </span>
                              )}
                            </div>
                          ) : (
                            <MetricPill tone="subtle">裸进程</MetricPill>
                          )}
                        </TableCell>

                        <TableCell className="pr-3 text-right">
                          <div className="inline-flex items-center justify-end gap-1">
                            <Button
                              type="button"
                              size="xs"
                              variant="ghost"
                              onClick={() => setInspectedServerId(record.id)}
                            >
                              详情
                            </Button>

                            {isActiveState ? (
                              <Button
                                type="button"
                                size="xs"
                                variant="outline"
                                disabled={isRowBusy}
                                onClick={() => void onStopServer(record.id)}
                              >
                                <SquareIcon data-icon="inline-start" />
                                停止
                              </Button>
                            ) : (
                              <>
                                <Button
                                  type="button"
                                  size="xs"
                                  variant="outline"
                                  disabled={isRowBusy || isStarting}
                                  onClick={() =>
                                    void onRestartServer(record.id)
                                  }
                                >
                                  <PlayIcon data-icon="inline-start" />
                                  启动
                                </Button>
                                <Tooltip>
                                  <TooltipTrigger asChild>
                                    <Button
                                      type="button"
                                      size="icon-xs"
                                      variant="ghost"
                                      aria-label={`移除托管记录 ${record.name}`}
                                      disabled={isRowBusy}
                                      onClick={() =>
                                        void onRemoveServer(record.id)
                                      }
                                    >
                                      <Trash2Icon />
                                    </Button>
                                  </TooltipTrigger>
                                  <TooltipContent side="left">
                                    移除托管记录
                                  </TooltipContent>
                                </Tooltip>
                              </>
                            )}
                          </div>
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          </ScrollArea>
        )}
      </div>

      {/* Card Footer */}
      <CardFooter className="shrink-0 justify-between gap-2">
        <span className="inline-flex min-w-0 items-center gap-1.5 truncate">
          <ShieldCheckIcon
            aria-hidden="true"
            className="size-3.5 shrink-0 text-primary"
          />
          <span className="truncate">
            Win32 Job Object 进程树级联回收已启用
          </span>
        </span>
        <span className="shrink-0 font-mono text-micro tabular-nums">
          Job 守护 · 零黑框
        </span>
      </CardFooter>

      {/* Child Server Detail Inspector Sheet */}
      <Sheet
        open={inspectedServer !== null}
        onOpenChange={(open) => {
          if (!open) {
            setInspectedServerId(null);
          }
        }}
      >
        <SheetContent
          side="right"
          className="flex flex-col gap-3 sm:max-w-lg"
        >
          {inspectedServer && (
            <>
              <SheetHeader className="shrink-0">
                <div className="flex flex-wrap items-center gap-2">
                  <SheetTitle>{inspectedServer.name}</SheetTitle>
                  <StatusBadge
                    tone={childStatusToTone(inspectedServer.status)}
                  >
                    {childStatusLabel(inspectedServer.status)}
                  </StatusBadge>
                </div>
                <SheetDescription className="font-mono">
                  服务标识：{inspectedServer.id}
                </SheetDescription>
              </SheetHeader>

              {/* Process Metadata Block */}
              <dl className="grid shrink-0 grid-cols-2 gap-x-4 gap-y-2.5 rounded-md border border-border-subtle bg-surface-sunken p-3 text-caption">
                <div className="flex min-w-0 flex-col gap-0.5">
                  <dt className="text-muted-foreground">进程 PID</dt>
                  <dd className="font-mono tabular-nums text-foreground">
                    {inspectedServer.pid !== null
                      ? inspectedServer.pid
                      : "未运行"}
                  </dd>
                </div>

                <div className="flex min-w-0 flex-col gap-0.5">
                  <dt className="text-muted-foreground">Win32 Job Object</dt>
                  <dd className="flex items-center gap-1.5">
                    <MetricPill
                      tone={
                        inspectedServer.job_object_attached ? "ok" : "neutral"
                      }
                    >
                      {inspectedServer.job_object_attached
                        ? "KILL_ON_JOB_CLOSE"
                        : "未绑定"}
                    </MetricPill>
                  </dd>
                </div>

                <div className="flex min-w-0 flex-col gap-0.5">
                  <dt className="text-muted-foreground">MCP 握手耗时</dt>
                  <dd className="font-mono tabular-nums text-foreground">
                    {inspectedServer.handshake_ms !== null
                      ? `${inspectedServer.handshake_ms} ms`
                      : "—"}
                  </dd>
                </div>

                <div className="flex min-w-0 flex-col gap-0.5">
                  <dt className="text-muted-foreground">服务端标识</dt>
                  <dd className="truncate font-mono text-foreground">
                    {inspectedServer.server_info_name
                      ? `${inspectedServer.server_info_name} ${inspectedServer.server_info_version ?? ""}`.trim()
                      : "—"}
                  </dd>
                </div>

                <div className="col-span-2 flex min-w-0 flex-col gap-0.5">
                  <dt className="text-muted-foreground">解析可执行路径</dt>
                  <dd className="min-w-0">
                    <TruncatedText
                      text={
                        inspectedServer.resolved_executable ??
                        inspectedServer.command
                      }
                      mono
                      className="text-micro text-foreground"
                    />
                  </dd>
                </div>

                {inspectedServer.env_keys.length > 0 && (
                  <div className="col-span-2 flex min-w-0 flex-col gap-1">
                    <dt className="text-muted-foreground">
                      已注入环境变量（密钥已脱敏）
                    </dt>
                    <dd className="flex flex-wrap gap-1">
                      {inspectedServer.env_keys.map((key) => (
                        <MetricPill key={key} tone="subtle">
                          {key} = •••
                        </MetricPill>
                      ))}
                    </dd>
                  </div>
                )}
              </dl>

              {inspectedServer.last_error && (
                <Alert variant="destructive" className="shrink-0">
                  <AlertCircleIcon />
                  <AlertTitle>最近错误摘要</AlertTitle>
                  <AlertDescription className="font-mono text-caption">
                    {inspectedServer.last_error}
                  </AlertDescription>
                </Alert>
              )}

              {/* Discovered Tools List */}
              {inspectedServer.discovered_tools.length > 0 && (
                <div className="flex shrink-0 flex-col gap-1.5">
                  <div className="flex items-center justify-between text-caption font-medium text-ink-secondary">
                    <span className="inline-flex items-center gap-1.5">
                      <WrenchIcon
                        aria-hidden="true"
                        className="size-3.5 text-primary"
                      />
                      <span>已发现 MCP 工具</span>
                    </span>
                    <MetricPill tone="subtle">
                      {inspectedServer.discovered_tools.length} 项
                    </MetricPill>
                  </div>

                  <ScrollArea className="max-h-32 rounded-md border border-border-subtle bg-surface-sunken">
                    <div className="divide-y divide-border-subtle">
                      {inspectedServer.discovered_tools.map((tool) => {
                        const isServerCalling = Boolean(
                          callingServerIds?.[inspectedServer.id],
                        );
                        const isCurrentToolCalling =
                          isServerCalling &&
                          callingToolKey ===
                            `${inspectedServer.id}:${tool.name}`;
                        const canCallTool =
                          inspectedServer.status === "running" &&
                          Boolean(onCallTool);
                        const defaultArgsJson =
                          tool.name === "echo_stdio_probe"
                            ? '{"text":"hello-keeldesk"}'
                            : "{}";

                        return (
                          <div
                            key={tool.name}
                            className="flex items-center justify-between gap-2 px-3 py-2"
                          >
                            <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                              <span className="truncate font-mono text-caption font-medium text-foreground">
                                {tool.name}
                              </span>
                              {tool.description && (
                                <span className="text-micro text-muted-foreground">
                                  {tool.description}
                                </span>
                              )}
                            </div>

                            {canCallTool && (
                              <Button
                                type="button"
                                size="xs"
                                variant="outline"
                                disabled={isServerCalling}
                                onClick={() =>
                                  void handleTestCallTool(
                                    inspectedServer.id,
                                    tool.name,
                                    defaultArgsJson,
                                  )
                                }
                              >
                                <PlayIcon data-icon="inline-start" />
                                {isCurrentToolCalling ? "调用中..." : "试调"}
                              </Button>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </ScrollArea>

                  <div role="status" aria-live="polite" className="min-w-0">
                    {activeToolCallResult && (
                      <div
                        className={cn(
                          "flex flex-wrap items-center justify-between gap-2 rounded-md border px-2.5 py-1.5 text-caption",
                          activeToolCallResult.is_error
                            ? "border-status-danger-border bg-status-danger-subtle text-foreground"
                            : "border-status-ok-border bg-status-ok-subtle text-foreground",
                        )}
                      >
                        <div className="flex min-w-0 flex-1 items-center gap-1.5">
                          <StatusDot
                            tone={
                              activeToolCallResult.is_error ? "danger" : "ok"
                            }
                          />
                          <span className="truncate font-mono text-micro text-foreground">
                            {activeToolCallResult.tool_name}
                          </span>
                          <MetricPill
                            tone={
                              activeToolCallResult.is_error ? "danger" : "ok"
                            }
                          >
                            {activeToolCallResult.is_error ? "Error" : "OK"} ·{" "}
                            {activeToolCallResult.duration_ms} ms
                          </MetricPill>
                        </div>

                        {activeToolCallResult.trace_seq !== null &&
                          onInspectTraceSeq && (
                            <Button
                              type="button"
                              size="xs"
                              variant="ghost"
                              onClick={() => {
                                const seq = activeToolCallResult.trace_seq;
                                if (seq !== null) {
                                  setInspectedServerId(null);
                                  onInspectTraceSeq(seq);
                                }
                              }}
                              className="font-mono tabular-nums"
                            >
                              查看报文 #{activeToolCallResult.trace_seq}
                            </Button>
                          )}
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* Live stderr tail */}
              <div className="flex min-h-0 flex-1 flex-col gap-1.5">
                <div className="flex shrink-0 items-center justify-between text-caption">
                  <span className="font-medium text-ink-secondary">
                    实时 stderr 日志尾流
                  </span>
                  <span className="font-mono text-micro tabular-nums text-muted-foreground">
                    {inspectedServer.stderr_tail.length} 行
                  </span>
                </div>

                <CodeWell
                  code={
                    inspectedServer.stderr_tail.length > 0
                      ? inspectedServer.stderr_tail.join("\n")
                      : "(暂无 stderr 输出)"
                  }
                  copyValue={
                    inspectedServer.stderr_tail.length > 0
                      ? inspectedServer.stderr_tail.join("\n")
                      : ""
                  }
                  copyLabel="复制 stderr 日志"
                  copiedLabel="已复制日志"
                  wrapLines
                  className="flex min-h-0 flex-1 flex-col"
                  scrollAreaClassName="min-h-0 flex-1"
                  viewportClassName="h-full"
                />
              </div>
            </>
          )}
        </SheetContent>
      </Sheet>

      {/* Add Child Server Dialog */}
      <Dialog open={isAddOpen} onOpenChange={setIsAddOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>添加受控 Stdio MCP 子进程</DialogTitle>
            <DialogDescription>
              通过隐藏控制台窗口派生子进程并绑定 Win32 Job Object，支持自动完成
              initialize 与 tools/list 握手。
            </DialogDescription>
          </DialogHeader>

          {/* Preset Quick-Fill Chips */}
          <div className="flex flex-col gap-1.5">
            <span className="text-caption font-medium text-ink-secondary">
              快速填充预设
            </span>
            <div className="flex flex-wrap items-center gap-1.5">
              <Button
                type="button"
                size="xs"
                variant="outline"
                onClick={() => resetFormWithPreset("echo")}
              >
                内置回显探针 (PowerShell)
              </Button>
              <Button
                type="button"
                size="xs"
                variant="outline"
                onClick={() => resetFormWithPreset("filesystem")}
              >
                Filesystem (npx)
              </Button>
              <Button
                type="button"
                size="xs"
                variant="outline"
                onClick={() => resetFormWithPreset("fetch")}
              >
                Fetch (uvx)
              </Button>
            </div>
          </div>

          <form
            id="add-child-server-form"
            className="flex flex-col gap-3"
            onSubmit={(event) => void handleSubmitAddForm(event)}
          >
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="child-server-id">服务标识 (ID)</Label>
                <Input
                  id="child-server-id"
                  inputSize="sm"
                  value={formId}
                  onChange={(e) => setFormId(e.currentTarget.value)}
                  placeholder="filesystem-mcp"
                  className="font-mono"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <Label htmlFor="child-server-name">显示名称</Label>
                <Input
                  id="child-server-name"
                  inputSize="sm"
                  value={formName}
                  onChange={(e) => setFormName(e.currentTarget.value)}
                  placeholder="Filesystem MCP"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="child-server-command">可执行命令</Label>
                <Input
                  id="child-server-command"
                  inputSize="sm"
                  value={formCommand}
                  onChange={(e) => setFormCommand(e.currentTarget.value)}
                  placeholder="npx / uvx / node / powershell.exe"
                  className="font-mono"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <Label htmlFor="child-server-cwd">工作目录 (可选)</Label>
                <Input
                  id="child-server-cwd"
                  inputSize="sm"
                  value={formCwd}
                  onChange={(e) => setFormCwd(e.currentTarget.value)}
                  placeholder="C:\\Users\\Developer\\workspace"
                  className="font-mono"
                />
              </div>
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between gap-2">
                <Label htmlFor="child-server-args">命令参数</Label>
                <span className="text-micro text-muted-foreground">
                  支持空格分词（带引号）或 JSON 字符串数组
                </span>
              </div>
              <textarea
                id="child-server-args"
                rows={2}
                value={formArgsText}
                onChange={(e) => setFormArgsText(e.currentTarget.value)}
                placeholder="-y @modelcontextprotocol/server-filesystem ."
                className={SUNKEN_TEXTAREA_CLASS}
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <Label htmlFor="child-server-env">
                环境变量 (可选，每行 KEY=VALUE)
              </Label>
              <textarea
                id="child-server-env"
                rows={2}
                value={formEnvText}
                onChange={(e) => setFormEnvText(e.currentTarget.value)}
                placeholder="API_TOKEN=..."
                className={SUNKEN_TEXTAREA_CLASS}
              />
            </div>

            <div className="flex items-center justify-between gap-3 rounded-md border border-border-subtle bg-surface-sunken px-3 py-2">
              <div className="flex min-w-0 items-center gap-2">
                <InfoIcon
                  aria-hidden="true"
                  className="size-3.5 shrink-0 text-primary"
                />
                <Label
                  htmlFor="child-server-handshake"
                  className="cursor-pointer text-caption text-foreground"
                >
                  启用 MCP Stdio 协议握手 (initialize + tools/list)
                </Label>
              </div>
              <Switch
                id="child-server-handshake"
                size="sm"
                checked={formHandshake}
                onCheckedChange={setFormHandshake}
              />
            </div>

            {formError && (
              <Alert variant="destructive">
                <AlertCircleIcon />
                <AlertDescription className="text-caption">
                  {formError}
                </AlertDescription>
              </Alert>
            )}

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => setIsAddOpen(false)}
              >
                取消
              </Button>
              <Button type="submit" disabled={isStarting}>
                <PlayIcon data-icon="inline-start" />
                {isStarting ? "启动并握手中..." : "启动子进程"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
