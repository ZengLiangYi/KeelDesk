import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  commands,
  events,
  type KeelHealthCheckResult,
  type KeelPingSelfTestResult,
  type KeelRuntimeSnapshot,
  type KeelServerStatus,
  type KeelToolDescriptor,
  type McpSessionSummary,
  type ToolCallLogEntry,
} from "./bindings";
import { pushPreviewProtocolTrace } from "./previewTraceBus";

const DEFAULT_ENDPOINT_URL = "http://127.0.0.1:19527/mcp";
const DEFAULT_BIND_ADDR = "127.0.0.1:19527";
const DEFAULT_PORT = 19527;

function createBrowserPreviewSnapshot(): KeelRuntimeSnapshot {
  const now = Date.now();
  return {
    status: "running",
    endpoint_url: DEFAULT_ENDPOINT_URL,
    bind_addr: DEFAULT_BIND_ADDR,
    bound_port: DEFAULT_PORT,
    server_name: "keeldesk",
    server_version: "0.1.0",
    started_at_iso: new Date(now - 240_000).toISOString(),
    error_message: null,
    total_calls: 0,
    tools: [
      {
        name: "keel_ping",
        description:
          "Verify connectivity with the local KeelDesk Windows 11 MCP spine and return runtime status.",
        source: "keel_core",
        safety: "safe",
      },
      {
        name: "scan_dev_env",
        description:
          "Probe installed Windows 11 developer toolchains (versions, executable paths, and latency) without flashing console windows.",
        source: "workbench",
        safety: "safe",
      },
      {
        name: "list_active_ports",
        description:
          "Enumerate active Windows 11 listening TCPv4/TCPv6 ports and their owning processes via native Win32 IP Helper APIs.",
        source: "workbench",
        safety: "safe",
      },
      {
        name: "kill_port_process",
        description:
          "Terminate the process owning a listening TCP port on Windows 11 (protected by KeelDesk interactive desktop approval).",
        source: "workbench",
        safety: "confirm_required",
      },
    ],
    recent_calls: [],
    pending_approvals: [],
    session_allowlist: [],
    active_sessions: [
      {
        session_id: "preview-sess-claude-01",
        client_name: "claude-code",
        client_version: "2.1.0",
        protocol_version: "2025-03-26",
        user_agent: "claude-code/2.1.0",
        connected_at_iso: new Date(now - 180_000).toISOString(),
        last_active_at_iso: new Date(now - 24_000).toISOString(),
        request_count: 4,
      },
    ],
  };
}

const BROWSER_PREVIEW_SNAPSHOT: KeelRuntimeSnapshot =
  createBrowserPreviewSnapshot();

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export interface UseMcpServerStateResult {
  snapshot: KeelRuntimeSnapshot | null;
  status: KeelServerStatus;
  isRunning: boolean;
  boundPort: number;
  bindAddr: string;
  endpointUrl: string;
  healthzUrl: string;
  activeSessions: McpSessionSummary[];
  tools: KeelToolDescriptor[];
  recentCalls: ToolCallLogEntry[];
  totalCalls: number;
  isLoading: boolean;
  isPinging: boolean;
  isCheckingHealth: boolean;
  lastPingResult: KeelPingSelfTestResult | null;
  lastHealthCheck: KeelHealthCheckResult | null;
  error: string | null;
  pingError: string | null;
  refreshSnapshot: () => Promise<KeelRuntimeSnapshot | null>;
  checkHealth: () => Promise<KeelHealthCheckResult | null>;
  runSelfTestPing: (
    echoMessage?: string,
  ) => Promise<KeelPingSelfTestResult | null>;
  clearError: () => void;
}

export function useMcpServerState(): UseMcpServerStateResult {
  const [snapshot, setSnapshot] = useState<KeelRuntimeSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isPinging, setIsPinging] = useState(false);
  const [isCheckingHealth, setIsCheckingHealth] = useState(false);
  const [lastPingResult, setLastPingResult] =
    useState<KeelPingSelfTestResult | null>(null);
  const [lastHealthCheck, setLastHealthCheck] =
    useState<KeelHealthCheckResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pingError, setPingError] = useState<string | null>(null);

  const snapshotRef = useRef<KeelRuntimeSnapshot | null>(snapshot);
  useEffect(() => {
    snapshotRef.current = snapshot;
  }, [snapshot]);

  const refreshSnapshot =
    useCallback(async (): Promise<KeelRuntimeSnapshot | null> => {
      if (!isTauriRuntime()) {
        const next = snapshotRef.current ?? BROWSER_PREVIEW_SNAPSHOT;
        setSnapshot(next);
        setIsLoading(false);
        return next;
      }

      try {
        const res = await commands.getKeelRuntimeSnapshot();
        if (res.status === "ok") {
          setSnapshot(res.data);
          return res.data;
        }
        setError(res.error);
        setPingError(res.error);
        return null;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        setPingError(message);
        return null;
      } finally {
        setIsLoading(false);
      }
    }, []);

  const checkHealth =
    useCallback(async (): Promise<KeelHealthCheckResult | null> => {
      setIsCheckingHealth(true);
      setError(null);

      try {
        if (!isTauriRuntime()) {
          const base = snapshotRef.current ?? BROWSER_PREVIEW_SNAPSHOT;
          const result: KeelHealthCheckResult = {
            ok: base.status === "running",
            status_code: base.status === "running" ? 200 : 503,
            latency_ms: 1,
            endpoint_url: base.endpoint_url,
            healthz_url: base.endpoint_url.replace(/\/mcp$/, "/healthz"),
            bound_port: base.bound_port,
            active_session_count: base.active_sessions.length,
            total_calls: base.total_calls,
            checked_at_iso: new Date().toISOString(),
          };
          setLastHealthCheck(result);
          return result;
        }

        const res = await commands.checkKeelHealthz();
        if (res.status === "ok") {
          setLastHealthCheck(res.data);
          return res.data;
        }
        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setIsCheckingHealth(false);
      }
    }, []);

  const runSelfTestPing = useCallback(
    async (echoMessage?: string): Promise<KeelPingSelfTestResult | null> => {
      setIsPinging(true);
      setPingError(null);
      setError(null);

      try {
        if (!isTauriRuntime()) {
          const now = new Date().toISOString();
          const trimmed = echoMessage?.trim() || null;
          const base = snapshotRef.current ?? BROWSER_PREVIEW_SNAPSHOT;
          const nextId = base.total_calls + 1;

          const pingPayload = {
            ok: true,
            server: "keeldesk",
            version: base.server_version,
            transport: "streamable-http",
            endpoint: base.endpoint_url,
            echo: trimmed,
            timestamp_iso: now,
          };

          const result: KeelPingSelfTestResult = {
            payload: pingPayload,
            roundtrip_ms: 2,
            advertised_tools: base.tools.map((tool) => tool.name),
          };

          setLastPingResult(result);
          setSnapshot((prev) => {
            const current = prev ?? BROWSER_PREVIEW_SNAPSHOT;
            const callId = current.total_calls + 1;
            return {
              ...current,
              total_calls: callId,
              recent_calls: [
                {
                  id: callId,
                  tool_name: "keel_ping",
                  timestamp_iso: now,
                  duration_ms: 1,
                  succeeded: true,
                  summary: trimmed
                    ? `OK · echo="${trimmed}"`
                    : "OK · loopback pong",
                  caller_hint: "browser-preview",
                },
                ...current.recent_calls,
              ].slice(0, 32),
            };
          });

          pushPreviewProtocolTrace({
            transport: "streamable_http",
            direction: "inbound",
            target: "keeldesk",
            session_id: "preview-sess-selftest",
            caller_hint: `keeldesk-selftest/${base.server_version}`,
            jsonrpc_id: String(nextId),
            method: "tools/call",
            tool_name: "keel_ping",
            safety_tier: "safe",
            approval_outcome: "not_required",
            status: "succeeded",
            status_code: 200,
            error_message: null,
            latency: {
              total_ms: 2,
              approval_wait_ms: 0,
              execution_ms: 2,
            },
            request_value: {
              jsonrpc: "2.0",
              id: nextId,
              method: "tools/call",
              params: {
                name: "keel_ping",
                arguments: trimmed ? { message: trimmed } : {},
              },
            },
            response_value: {
              jsonrpc: "2.0",
              id: nextId,
              result: {
                isError: false,
                structuredContent: pingPayload,
              },
            },
            linked_call_id: nextId,
            timestamp_iso: now,
          });

          return result;
        }

        const normalized =
          echoMessage && echoMessage.trim().length > 0
            ? echoMessage.trim()
            : null;
        const res = await commands.selfTestKeelPing(normalized);
        if (res.status === "ok") {
          setLastPingResult(res.data);
          return res.data;
        }

        setPingError(res.error);
        setError(res.error);
        return null;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setPingError(message);
        setError(message);
        return null;
      } finally {
        setIsPinging(false);
      }
    },
    [],
  );

  const clearError = useCallback(() => {
    setError(null);
    setPingError(null);
  }, []);

  useEffect(() => {
    void refreshSnapshot();

    if (!isTauriRuntime()) {
      return;
    }

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void events.keelRuntimeUpdatedEvent
      .listen((event) => {
        if (!cancelled) {
          setSnapshot(event.payload);
          setIsLoading(false);
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      });

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [refreshSnapshot]);

  const status: KeelServerStatus = snapshot?.status ?? "starting";
  const isRunning = status === "running";
  const boundPort = snapshot?.bound_port ?? DEFAULT_PORT;
  const bindAddr = snapshot?.bind_addr ?? DEFAULT_BIND_ADDR;
  const endpointUrl = snapshot?.endpoint_url ?? DEFAULT_ENDPOINT_URL;
  const healthzUrl = useMemo(
    () => endpointUrl.replace(/\/mcp$/, "/healthz"),
    [endpointUrl],
  );
  const activeSessions = snapshot?.active_sessions ?? [];
  const tools = snapshot?.tools ?? [];
  const recentCalls = snapshot?.recent_calls ?? [];
  const totalCalls = snapshot?.total_calls ?? 0;

  return {
    snapshot,
    status,
    isRunning,
    boundPort,
    bindAddr,
    endpointUrl,
    healthzUrl,
    activeSessions,
    tools,
    recentCalls,
    totalCalls,
    isLoading,
    isPinging,
    isCheckingHealth,
    lastPingResult,
    lastHealthCheck,
    error,
    pingError,
    refreshSnapshot,
    checkHealth,
    runSelfTestPing,
    clearError,
  };
}
