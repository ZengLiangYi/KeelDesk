import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertCircleIcon,
  NetworkIcon,
  RefreshCwIcon,
  SearchIcon,
  ShieldAlertIcon,
} from "lucide-react";
import { isTauriRuntime } from "@keeldesk/react";
import { commands, type ActivePortReport } from "@/bindings";
import {
  EmptyState,
  MetricPill,
  TruncatedText,
  formatTimeHms,
} from "@/components/ds";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { CardFooter } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
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

export interface PortsGovernancePanelProps {
  isActive?: boolean;
  isServerRunning: boolean;
  isDrilling: boolean;
  onDrillPort: (port: number, processName: string) => void;
}

function buildBrowserPreviewPortReport(): ActivePortReport {
  return {
    captured_at_iso: new Date().toISOString(),
    total_listening: 5,
    entries: [
      {
        port: 19527,
        protocol: "tcp4",
        local_addr: "127.0.0.1",
        pid: 4820,
        process_name: "keeldesk.exe",
        exe_path: "C:\\Users\\Developer\\AppData\\Local\\KeelDesk\\keeldesk.exe",
        is_loopback: true,
        is_keeldesk_self: true,
      },
      {
        port: 1420,
        protocol: "tcp4",
        local_addr: "127.0.0.1",
        pid: 9104,
        process_name: "node.exe",
        exe_path: "C:\\Program Files\\nodejs\\node.exe",
        is_loopback: true,
        is_keeldesk_self: false,
      },
      {
        port: 5432,
        protocol: "tcp4",
        local_addr: "127.0.0.1",
        pid: 3312,
        process_name: "postgres.exe",
        exe_path: "C:\\Program Files\\PostgreSQL\\17\\bin\\postgres.exe",
        is_loopback: true,
        is_keeldesk_self: false,
      },
      {
        port: 5173,
        protocol: "tcp6",
        local_addr: "::1",
        pid: 11280,
        process_name: "node.exe",
        exe_path: "C:\\Program Files\\nodejs\\node.exe",
        is_loopback: true,
        is_keeldesk_self: false,
      },
      {
        port: 445,
        protocol: "tcp4",
        local_addr: "0.0.0.0",
        pid: 4,
        process_name: "System",
        exe_path: null,
        is_loopback: false,
        is_keeldesk_self: false,
      },
    ],
  };
}

export function PortsGovernancePanel({
  isActive = true,
  isServerRunning,
  isDrilling,
  onDrillPort,
}: PortsGovernancePanelProps) {
  const [report, setReport] = useState<ActivePortReport | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filterQuery, setFilterQuery] = useState("");
  const hasFetchedRef = useRef(false);

  const fetchActivePorts = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (!isTauriRuntime()) {
        setReport(buildBrowserPreviewPortReport());
        return;
      }

      const res = await commands.listWin11ActivePorts(null, null);
      if (res.status === "ok") {
        setReport(res.data);
      } else {
        setError(res.error);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!isActive || hasFetchedRef.current) {
      return;
    }
    hasFetchedRef.current = true;
    void fetchActivePorts();
  }, [isActive, fetchActivePorts]);

  const entries = report?.entries ?? [];
  const normalizedQuery = filterQuery.trim().toLowerCase();
  const numericPortQuery = normalizedQuery.startsWith(":")
    ? normalizedQuery.slice(1)
    : normalizedQuery;

  const filteredEntries = useMemo(() => {
    if (!normalizedQuery) {
      return entries;
    }
    return entries.filter((entry) => {
      if (String(entry.port).includes(numericPortQuery)) {
        return true;
      }
      if (String(entry.pid).includes(normalizedQuery)) {
        return true;
      }
      if (entry.process_name.toLowerCase().includes(normalizedQuery)) {
        return true;
      }
      if (entry.local_addr.toLowerCase().includes(normalizedQuery)) {
        return true;
      }
      if (entry.protocol.toLowerCase().includes(normalizedQuery)) {
        return true;
      }
      return false;
    });
  }, [entries, normalizedQuery, numericPortQuery]);

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {/* Filter & Refresh Toolbar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border-subtle px-3 py-2">
        <div className="relative flex min-w-0 flex-1 items-center sm:max-w-xs">
          <SearchIcon
            aria-hidden="true"
            className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground"
          />
          <Input
            inputSize="sm"
            value={filterQuery}
            onChange={(event) => setFilterQuery(event.currentTarget.value)}
            placeholder="筛选端口、进程名、PID 或地址..."
            aria-label="筛选活跃监听端口"
            className="pl-8 font-mono"
          />
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <MetricPill tone="neutral" aria-live="polite">
            {normalizedQuery
              ? `${filteredEntries.length} / ${entries.length} 项`
              : `${entries.length} 个监听`}
          </MetricPill>
          <Button
            type="button"
            variant="outline"
            size="xs"
            disabled={isLoading}
            onClick={() => void fetchActivePorts()}
          >
            <RefreshCwIcon
              data-icon="inline-start"
              className={cn(isLoading && "animate-spin")}
            />
            {isLoading ? "扫描中..." : "刷新"}
          </Button>
        </div>
      </div>

      {error && (
        <div className="shrink-0 px-3 pt-2">
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>端口快照枚举失败</AlertTitle>
            <AlertDescription className="font-mono text-caption">
              {error}
            </AlertDescription>
          </Alert>
        </div>
      )}

      {/* Bounded Scrollable Ports Table */}
      <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
        {isLoading && report === null ? (
          <div className="flex flex-col gap-2 p-3">
            <Skeleton className="h-7 w-full rounded-sm" />
            <Skeleton className="h-8 w-full rounded-sm" />
            <Skeleton className="h-8 w-full rounded-sm" />
            <Skeleton className="h-8 w-11/12 rounded-sm" />
          </div>
        ) : (
          <ScrollArea className="h-full">
            <div
              role="region"
              aria-label="Windows 11 活跃监听端口列表"
              tabIndex={0}
              className="outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
            >
              <Table className="table-fixed">
                <TableHeader className="sticky top-0 z-(--z-sticky) bg-surface-subtle">
                  <TableRow className="hover:bg-transparent">
                    <TableHead scope="col" className="w-40 pl-3">
                      端口 / 协议
                    </TableHead>
                    <TableHead scope="col" className="w-28">
                      监听地址
                    </TableHead>
                    <TableHead scope="col">PID / 进程</TableHead>
                    <TableHead scope="col" className="w-24 pr-3 text-right">
                      演练操作
                    </TableHead>
                  </TableRow>
                </TableHeader>

                <TableBody>
                  {filteredEntries.length === 0 ? (
                    <TableRow className="hover:bg-transparent">
                      <TableCell colSpan={4} className="p-3 whitespace-normal">
                        <EmptyState
                          icon={<NetworkIcon />}
                          title={
                            entries.length === 0
                              ? "未检出活跃监听端口"
                              : "未匹配到符合筛选条件的监听记录"
                          }
                          description={
                            entries.length === 0
                              ? "当前主机暂未检测到活跃 TCPv4 或 TCPv6 监听套接字。"
                              : "请尝试调整端口号、进程名、PID 或监听地址筛选关键词。"
                          }
                        />
                      </TableCell>
                    </TableRow>
                  ) : (
                    filteredEntries.map((entry) => {
                      const rowKey = `${entry.protocol}-${entry.local_addr}-${entry.port}-${entry.pid}`;
                      return (
                        <TableRow key={rowKey}>
                          <TableHead
                            scope="row"
                            className="pl-3 font-mono text-caption font-medium tabular-nums text-foreground"
                          >
                            <div className="flex flex-wrap items-center gap-1.5">
                              <span>:{entry.port}</span>
                              <MetricPill tone="subtle">
                                {entry.protocol}
                              </MetricPill>
                              {entry.is_keeldesk_self && (
                                <MetricPill tone="primary">KeelDesk</MetricPill>
                              )}
                            </div>
                          </TableHead>

                          <TableCell className="truncate font-mono text-caption tabular-nums text-muted-foreground">
                            {entry.local_addr}
                          </TableCell>

                          <TableCell className="truncate text-caption">
                            {entry.exe_path ? (
                              <TruncatedText
                                text={entry.process_name}
                                tooltip={entry.exe_path}
                                tooltipClassName="font-mono text-micro"
                              >
                                <span className="shrink-0 font-mono text-micro tabular-nums text-muted-foreground">
                                  {entry.pid}
                                </span>
                                <span className="truncate font-mono text-caption text-foreground">
                                  {entry.process_name}
                                </span>
                              </TruncatedText>
                            ) : (
                              <span className="inline-flex max-w-full items-center gap-1.5 truncate">
                                <span className="shrink-0 font-mono text-micro tabular-nums text-muted-foreground">
                                  {entry.pid}
                                </span>
                                <span className="truncate font-mono text-caption text-foreground">
                                  {entry.process_name}
                                </span>
                              </span>
                            )}
                          </TableCell>

                          <TableCell className="pr-3 text-right">
                            <Tooltip>
                              <TooltipTrigger asChild>
                                <Button
                                  type="button"
                                  size="xs"
                                  variant="outline"
                                  disabled={isDrilling || !isServerRunning}
                                  onClick={() =>
                                    onDrillPort(entry.port, entry.process_name)
                                  }
                                >
                                  <ShieldAlertIcon data-icon="inline-start" />
                                  演练拦截
                                </Button>
                              </TooltipTrigger>
                              <TooltipContent side="left">
                                对端口 :{entry.port} 发起 dry_run 审批演练，验证拦截弹窗且绝不终止真实进程
                              </TooltipContent>
                            </Tooltip>
                          </TableCell>
                        </TableRow>
                      );
                    })
                  )}
                </TableBody>
              </Table>
            </div>
          </ScrollArea>
        )}
      </div>

      {/* Footer Strip */}
      <CardFooter className="shrink-0 justify-between gap-2">
        <span className="inline-flex min-w-0 items-center gap-1.5 truncate">
          <NetworkIcon
            aria-hidden="true"
            className="size-3.5 shrink-0 text-primary"
          />
          <span className="truncate">
            Win32 GetExtendedTcpTable 原生监听快照
          </span>
        </span>
        <span className="shrink-0 font-mono text-micro tabular-nums">
          采样 {formatTimeHms(report?.captured_at_iso)}
        </span>
      </CardFooter>
    </div>
  );
}
