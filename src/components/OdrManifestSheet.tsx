import {
  AlertCircleIcon,
  CheckCircle2Icon,
  DownloadIcon,
  FileJson2Icon,
  InfoIcon,
  RefreshCwIcon,
  TerminalIcon,
  Trash2Icon,
  UploadCloudIcon,
  XIcon,
} from "lucide-react";
import type {
  OdrActionOutcome,
  OdrRegisterMode,
  OdrStatusReport,
} from "@/bindings";
import {
  CodeWell,
  CopyButton,
  MetricPill,
  StatusBadge,
  TruncatedText,
  odrReadinessLabel,
  odrReadinessToTone,
} from "@/components/ds";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { cn } from "@/lib/utils";

export interface OdrManifestSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  odrStatus: OdrStatusReport | null;
  isLoading: boolean;
  isActing: boolean;
  lastOutcome: OdrActionOutcome | null;
  error: string | null;
  onRefresh: () => void;
  onRegister: (mode: OdrRegisterMode) => void;
  onUnregister: (serverId?: string | null) => void;
  onExportManifest: () => void;
  onClearOutcome: () => void;
}

export function OdrManifestSheet({
  open,
  onOpenChange,
  odrStatus,
  isLoading,
  isActing,
  lastOutcome,
  error,
  onRefresh,
  onRegister,
  onUnregister,
  onExportManifest,
  onClearOutcome,
}: OdrManifestSheetProps) {
  const buildLabel = odrStatus
    ? `Build ${odrStatus.os_build}.${odrStatus.os_ubr}`
    : "探测中...";

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="flex flex-col gap-3 sm:max-w-lg"
      >
        <SheetHeader className="shrink-0">
          <div className="flex flex-wrap items-center gap-2">
            <SheetTitle className="flex items-center gap-2">
              <FileJson2Icon
                aria-hidden="true"
                className="size-4 text-primary"
              />
              <span>Windows 11 ODR 注册与清单</span>
            </SheetTitle>
            <StatusBadge
              tone={odrReadinessToTone(odrStatus?.readiness)}
            >
              {odrReadinessLabel(odrStatus?.readiness)}
            </StatusBadge>
          </div>
          <SheetDescription>
            探测系统 odr.exe (On-Device Agent Registry) 就绪状态，向 Windows 11
            注册 KeelDesk 或导出 MCPB &quot;0.3&quot; 清单。
          </SheetDescription>
        </SheetHeader>

        {/* Readiness Checklist (sunken block, never nested Card) */}
        <div className="flex shrink-0 flex-col gap-2.5 rounded-md border border-border-subtle bg-surface-sunken p-3">
          <dl className="grid grid-cols-2 gap-x-4 gap-y-2.5 text-caption">
            <div className="flex min-w-0 flex-col gap-1">
              <dt className="text-muted-foreground">Windows 11 构建号</dt>
              <dd className="flex flex-wrap items-center gap-1.5">
                <span className="font-mono text-caption font-medium tabular-nums text-foreground">
                  {buildLabel}
                </span>
                {odrStatus && (
                  <MetricPill
                    tone={odrStatus.meets_odr_build_hint ? "ok" : "neutral"}
                  >
                    {odrStatus.meets_odr_build_hint
                      ? ">= 26220.7262"
                      : "建议 26220.7262+"}
                  </MetricPill>
                )}
              </dd>
            </div>

            <div className="flex min-w-0 flex-col gap-1">
              <dt className="text-muted-foreground">开发者模式</dt>
              <dd className="flex items-center">
                <StatusBadge
                  tone={odrStatus?.developer_mode_enabled ? "ok" : "warn"}
                >
                  {odrStatus?.developer_mode_enabled ? "已开启" : "未开启"}
                </StatusBadge>
              </dd>
            </div>

            <div className="flex min-w-0 flex-col gap-1">
              <dt className="text-muted-foreground">odr.exe 组件</dt>
              <dd className="flex min-w-0 items-center gap-1.5">
                <StatusBadge
                  tone={odrStatus?.odr_installed ? "ok" : "neutral"}
                >
                  {odrStatus?.odr_installed ? "已检出" : "未安装 · 回退模式"}
                </StatusBadge>
                {odrStatus?.odr_version && (
                  <TruncatedText
                    text={odrStatus.odr_version}
                    mono
                    muted
                    className="text-micro"
                  />
                )}
              </dd>
            </div>

            <div className="flex min-w-0 flex-col gap-1">
              <dt className="text-muted-foreground">注册状态</dt>
              <dd className="flex items-center gap-1.5">
                <StatusBadge
                  tone={odrStatus?.keel_enrolled ? "ok" : "neutral"}
                >
                  {odrStatus?.keel_enrolled ? "已注册" : "未注册"}
                </StatusBadge>
                {odrStatus && odrStatus.enrolled_servers.length > 0 && (
                  <MetricPill tone="subtle">
                    {odrStatus.enrolled_servers.length} 项记录
                  </MetricPill>
                )}
              </dd>
            </div>
          </dl>

          {odrStatus?.guidance_message && (
            <p className="border-t border-border-subtle pt-2 text-caption text-muted-foreground">
              {odrStatus.guidance_message}
            </p>
          )}
        </div>

        {/* Action Toolbar */}
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          <Button
            type="button"
            size="sm"
            variant="default"
            disabled={isActing || !odrStatus}
            onClick={() => onRegister("uri")}
          >
            <UploadCloudIcon data-icon="inline-start" />
            URI 注册
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={isActing || !odrStatus}
            onClick={() => onRegister("manifest")}
          >
            <FileJson2Icon data-icon="inline-start" />
            Manifest 注册
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={isActing || !odrStatus}
            onClick={onExportManifest}
          >
            <DownloadIcon data-icon="inline-start" />
            导出 MCPB 清单
          </Button>
          {odrStatus?.keel_enrolled && (
            <Button
              type="button"
              size="sm"
              variant="destructive"
              disabled={isActing}
              onClick={() => onUnregister("keeldesk")}
            >
              <Trash2Icon data-icon="inline-start" />
              移除注册
            </Button>
          )}
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={isLoading || isActing}
            onClick={onRefresh}
            className="ml-auto"
          >
            <RefreshCwIcon
              data-icon="inline-start"
              className={cn(isLoading && "animate-spin")}
            />
            刷新探测
          </Button>
        </div>

        {/* Polite Live Feedback Region */}
        <div role="status" aria-live="polite" className="shrink-0">
          {error && (
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>ODR 操作异常</AlertTitle>
              <AlertDescription className="font-mono text-caption">
                {error}
              </AlertDescription>
            </Alert>
          )}

          {!error && lastOutcome && (
            <div
              className={cn(
                "flex items-start justify-between gap-2 rounded-md border px-3 py-2 text-caption",
                lastOutcome.succeeded &&
                  "border-status-ok-border bg-status-ok-subtle text-foreground",
                !lastOutcome.succeeded &&
                  !lastOutcome.executed_cli &&
                  "border-status-warn-border bg-status-warn-subtle text-foreground",
                !lastOutcome.succeeded &&
                  lastOutcome.executed_cli &&
                  "border-status-danger-border bg-status-danger-subtle text-foreground",
              )}
            >
              <div className="flex min-w-0 flex-1 items-start gap-2">
                {lastOutcome.succeeded ? (
                  <CheckCircle2Icon
                    aria-hidden="true"
                    className="mt-0.5 size-3.5 shrink-0 text-status-ok"
                  />
                ) : !lastOutcome.executed_cli ? (
                  <InfoIcon
                    aria-hidden="true"
                    className="mt-0.5 size-3.5 shrink-0 text-status-warn"
                  />
                ) : (
                  <AlertCircleIcon
                    aria-hidden="true"
                    className="mt-0.5 size-3.5 shrink-0 text-status-danger"
                  />
                )}
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <span className="break-words text-caption text-foreground">
                    {lastOutcome.message}
                  </span>
                  {lastOutcome.exported_manifest_path && (
                    <TruncatedText
                      text={lastOutcome.exported_manifest_path}
                      mono
                      muted
                      className="text-micro"
                    />
                  )}
                </div>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="关闭 ODR 操作提示"
                onClick={onClearOutcome}
              >
                <XIcon />
              </Button>
            </div>
          )}
        </div>

        {/* CLI Command Reference */}
        {odrStatus && (
          <div className="flex shrink-0 flex-col gap-1.5">
            <div className="flex items-center gap-1.5 text-caption font-medium text-ink-secondary">
              <TerminalIcon
                aria-hidden="true"
                className="size-3.5 text-primary"
              />
              <span>ODR CLI 参考命令</span>
            </div>

            <div className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between gap-2 rounded-md border border-border-subtle bg-surface-sunken px-2.5 py-1.5">
                <span className="shrink-0 font-mono text-micro text-muted-foreground">
                  URI
                </span>
                <TruncatedText
                  text={odrStatus.uri_register_command}
                  mono
                  className="flex-1 text-micro text-foreground"
                />
                <CopyButton
                  value={odrStatus.uri_register_command}
                  label="复制 URI 注册命令"
                  copiedLabel="已复制 URI 命令"
                  side="left"
                />
              </div>

              <div className="flex items-center justify-between gap-2 rounded-md border border-border-subtle bg-surface-sunken px-2.5 py-1.5">
                <span className="shrink-0 font-mono text-micro text-muted-foreground">
                  Manifest
                </span>
                <TruncatedText
                  text={odrStatus.manifest_register_command}
                  mono
                  className="flex-1 text-micro text-foreground"
                />
                <CopyButton
                  value={odrStatus.manifest_register_command}
                  label="复制 Manifest 注册命令"
                  copiedLabel="已复制 Manifest 命令"
                  side="left"
                />
              </div>
            </div>
          </div>
        )}

        {/* Bounded Scrollable MCPB 0.3 Manifest JSON Viewer */}
        <div className="flex min-h-0 flex-1 flex-col gap-1.5">
          <div className="flex shrink-0 items-center justify-between gap-2 text-caption">
            <span className="font-medium text-ink-secondary">
              MCPB &quot;0.3&quot; 清单预览
            </span>
            {odrStatus && (
              <TruncatedText
                text={odrStatus.default_manifest_path}
                mono
                muted
                className="max-w-56 text-micro"
              />
            )}
          </div>

          <CodeWell
            code={odrStatus?.manifest_json ?? "{}"}
            copyValue={odrStatus?.manifest_json}
            copyLabel="复制 MCPB 清单 JSON"
            copiedLabel="已复制清单 JSON"
            className="flex min-h-0 flex-1 flex-col"
            scrollAreaClassName="min-h-0 flex-1"
            viewportClassName="h-full"
          />
        </div>
      </SheetContent>
    </Sheet>
  );
}
