import { useCallback, useEffect, useRef, useState } from "react";
import {
  AlertCircleIcon,
  CheckCircle2Icon,
  CpuIcon,
  MinusCircleIcon,
  RefreshCwIcon,
} from "lucide-react";
import { commands, type DevEnvScanReport } from "@/bindings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CardFooter } from "@/components/ui/card";
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
import { isTauriRuntime } from "@/keel-sdk";
import { cn } from "@/lib/utils";

export interface DevEnvPanelProps {
  isActive?: boolean;
}

function buildBrowserPreviewDevEnvReport(): DevEnvScanReport {
  return {
    scanned_at_iso: new Date().toISOString(),
    host: {
      edition: "Windows 11 Pro",
      display_version: "24H2",
      build_label: "26300.0",
      hostname: "LOCALHOST",
      arch: "x86_64",
      is_windows_11: true,
    },
    path_entry_count: 34,
    installed_count: 7,
    probes: [
      {
        id: "git",
        display_name: "Git",
        installed: true,
        version: "git version 2.48.1.windows.1",
        executable_path: "C:\\Program Files\\Git\\cmd\\git.exe",
        probe_ms: 18,
      },
      {
        id: "node",
        display_name: "Node.js",
        installed: true,
        version: "v22.14.0",
        executable_path: "C:\\Program Files\\nodejs\\node.exe",
        probe_ms: 14,
      },
      {
        id: "pnpm",
        display_name: "pnpm",
        installed: true,
        version: "10.6.1",
        executable_path: "C:\\Users\\Developer\\AppData\\Local\\pnpm\\pnpm.EXE",
        probe_ms: 42,
      },
      {
        id: "rustc",
        display_name: "Rust (rustc)",
        installed: true,
        version: "rustc 1.85.0 (4d91de4e4 2025-02-17)",
        executable_path: "C:\\Users\\Developer\\.cargo\\bin\\rustc.exe",
        probe_ms: 21,
      },
      {
        id: "cargo",
        display_name: "Cargo",
        installed: true,
        version: "cargo 1.85.0 (d73d2caf9 2024-12-31)",
        executable_path: "C:\\Users\\Developer\\.cargo\\bin\\cargo.exe",
        probe_ms: 19,
      },
      {
        id: "pwsh",
        display_name: "PowerShell 7",
        installed: true,
        version: "PowerShell 7.5.0",
        executable_path: "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
        probe_ms: 36,
      },
      {
        id: "uv",
        display_name: "uv",
        installed: true,
        version: "uv 0.6.4 (x86_64-pc-windows-msvc)",
        executable_path: "C:\\Users\\Developer\\.local\\bin\\uv.exe",
        probe_ms: 11,
      },
      {
        id: "python",
        display_name: "Python",
        installed: false,
        version: null,
        executable_path: null,
        probe_ms: 4,
      },
    ],
  };
}

function formatScanHms(iso: string | undefined): string {
  if (!iso) {
    return "--:--:--";
  }
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

export function DevEnvPanel({ isActive = true }: DevEnvPanelProps) {
  const [report, setReport] = useState<DevEnvScanReport | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasFetchedRef = useRef(false);

  const scanDevEnv = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (!isTauriRuntime()) {
        setReport(buildBrowserPreviewDevEnvReport());
        return;
      }

      const res = await commands.scanWin11DevEnv();
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
    void scanDevEnv();
  }, [isActive, scanDevEnv]);

  const probes = report?.probes ?? [];
  const installedCount = report?.installed_count ?? 0;

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {/* Summary & Rescan Header Strip */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-border px-3 py-2">
        <div
          aria-live="polite"
          className="flex flex-wrap items-center gap-1.5 text-xs"
        >
          <Badge
            variant="secondary"
            className="font-mono tabular-nums select-none"
          >
            已就绪 {installedCount} / {probes.length} 项工具链
          </Badge>
          <Badge
            variant="outline"
            className="font-mono text-[11px] tabular-nums text-muted-foreground select-none"
          >
            PATH {report?.path_entry_count ?? 0} 项
          </Badge>
          {report && (
            <span className="font-mono text-[11px] tabular-nums text-muted-foreground">
              扫描于 {formatScanHms(report.scanned_at_iso)}
            </span>
          )}
        </div>

        <Button
          type="button"
          variant="outline"
          size="xs"
          disabled={isLoading}
          onClick={() => void scanDevEnv()}
        >
          <RefreshCwIcon
            data-icon="inline-start"
            className={cn(isLoading && "animate-spin")}
          />
          {isLoading ? "探测中..." : "重新探测"}
        </Button>
      </div>

      {error && (
        <div className="shrink-0 px-3 pt-2">
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>本机开发环境扫描异常</AlertTitle>
            <AlertDescription className="font-mono text-xs">
              {error}
            </AlertDescription>
          </Alert>
        </div>
      )}

      {/* Bounded Scrollable Toolchain Table */}
      <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
        {isLoading && report === null ? (
          <div className="flex flex-col gap-2 p-3">
            <Skeleton className="h-7 w-full" />
            <Skeleton className="h-8 w-full" />
            <Skeleton className="h-8 w-full" />
            <Skeleton className="h-8 w-10/12" />
          </div>
        ) : (
          <ScrollArea className="h-full">
            <div
              role="region"
              aria-label="Windows 11 本机开发工具链探测列表"
              tabIndex={0}
              className="outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
            >
              <Table className="table-fixed">
                <TableHeader className="sticky top-0 z-10 bg-card">
                  <TableRow className="hover:bg-transparent">
                    <TableHead scope="col" className="w-36 pl-3 text-xs">
                      工具链
                    </TableHead>
                    <TableHead scope="col" className="w-24 text-xs">
                      状态
                    </TableHead>
                    <TableHead scope="col" className="text-xs">
                      版本摘要
                    </TableHead>
                    <TableHead
                      scope="col"
                      className="w-20 pr-3 text-right text-xs"
                    >
                      耗时
                    </TableHead>
                  </TableRow>
                </TableHeader>

                <TableBody>
                  {probes.map((probe) => (
                    <TableRow key={probe.id}>
                      <TableHead
                        scope="row"
                        className="truncate pl-3 text-xs font-medium text-foreground"
                      >
                        {probe.executable_path ? (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <span
                                tabIndex={0}
                                className="block truncate rounded-xs outline-none focus-visible:ring-1 focus-visible:ring-ring"
                              >
                                {probe.display_name}
                              </span>
                            </TooltipTrigger>
                            <TooltipContent
                              side="top"
                              className="max-w-md font-mono text-[11px] break-all"
                            >
                              {probe.executable_path}
                            </TooltipContent>
                          </Tooltip>
                        ) : (
                          <span className="block truncate text-muted-foreground">
                            {probe.display_name}
                          </span>
                        )}
                      </TableHead>

                      <TableCell>
                        {probe.installed ? (
                          <Badge
                            variant="secondary"
                            className="gap-1 text-[11px] select-none"
                          >
                            <CheckCircle2Icon
                              aria-hidden="true"
                              className="text-(--keel-status-ok)"
                            />
                            已就绪
                          </Badge>
                        ) : (
                          <Badge
                            variant="outline"
                            className="gap-1 text-[11px] text-muted-foreground select-none"
                          >
                            <MinusCircleIcon aria-hidden="true" />
                            未检出
                          </Badge>
                        )}
                      </TableCell>

                      <TableCell className="truncate font-mono text-xs text-muted-foreground">
                        {probe.version ? (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <span
                                tabIndex={0}
                                className="block truncate rounded-xs text-foreground/90 outline-none focus-visible:ring-1 focus-visible:ring-ring"
                              >
                                {probe.version}
                              </span>
                            </TooltipTrigger>
                            <TooltipContent
                              side="top"
                              className="max-w-md font-mono text-[11px] break-all"
                            >
                              <div className="flex flex-col gap-0.5">
                                <span>{probe.version}</span>
                                {probe.executable_path && (
                                  <span className="opacity-75">
                                    {probe.executable_path}
                                  </span>
                                )}
                              </div>
                            </TooltipContent>
                          </Tooltip>
                        ) : (
                          <span className="text-muted-foreground/70">
                            未在系统 PATH 检出可执行文件
                          </span>
                        )}
                      </TableCell>

                      <TableCell className="pr-3 text-right font-mono text-xs tabular-nums text-muted-foreground">
                        {probe.probe_ms} ms
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          </ScrollArea>
        )}
      </div>

      {/* Footer Strip */}
      <CardFooter className="shrink-0 justify-between gap-2 py-2 text-xs text-muted-foreground">
        <span className="inline-flex min-w-0 items-center gap-1.5 truncate">
          <CpuIcon
            aria-hidden="true"
            className="size-3.5 shrink-0 text-primary"
          />
          <span className="truncate">
            CREATE_NO_WINDOW 静默无弹窗并发探针
          </span>
        </span>
        <span className="shrink-0 font-mono text-[11px] tabular-nums">
          {installedCount}/{probes.length} 可用
        </span>
      </CardFooter>
    </div>
  );
}
