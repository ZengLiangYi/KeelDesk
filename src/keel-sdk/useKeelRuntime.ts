import { useCallback, useEffect, useState } from "react";
import {
  commands,
  events,
  type KeelPingSelfTestResult,
  type KeelRuntimeSnapshot,
} from "@/bindings";

const BROWSER_PREVIEW_SNAPSHOT: KeelRuntimeSnapshot = {
  status: "running",
  endpoint_url: "http://127.0.0.1:19527/mcp",
  bind_addr: "127.0.0.1:19527",
  bound_port: 19527,
  server_name: "keeldesk",
  server_version: "0.1.0",
  started_at_iso: new Date().toISOString(),
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
  active_sessions: [],
};

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function useKeelRuntime() {
  const [snapshot, setSnapshot] = useState<KeelRuntimeSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isPinging, setIsPinging] = useState(false);
  const [lastPingResult, setLastPingResult] =
    useState<KeelPingSelfTestResult | null>(null);
  const [pingError, setPingError] = useState<string | null>(null);

  const refreshSnapshot = useCallback(async () => {
    if (!isTauriRuntime()) {
      setSnapshot(BROWSER_PREVIEW_SNAPSHOT);
      setIsLoading(false);
      return;
    }

    try {
      const res = await commands.getKeelRuntimeSnapshot();
      if (res.status === "ok") {
        setSnapshot(res.data);
      } else {
        setPingError(res.error);
      }
    } catch (err) {
      setPingError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const runSelfTestPing = useCallback(async (echoMessage?: string) => {
    setIsPinging(true);
    setPingError(null);

    try {
      if (!isTauriRuntime()) {
        const now = new Date().toISOString();
        const trimmed = echoMessage?.trim() || null;
        setLastPingResult({
          payload: {
            ok: true,
            server: "keeldesk",
            version: "0.1.0",
            transport: "streamable-http",
            endpoint: BROWSER_PREVIEW_SNAPSHOT.endpoint_url,
            echo: trimmed,
            timestamp_iso: now,
          },
          roundtrip_ms: 2,
          advertised_tools: BROWSER_PREVIEW_SNAPSHOT.tools.map((t) => t.name),
        });
        setSnapshot((prev) => {
          const base = prev ?? BROWSER_PREVIEW_SNAPSHOT;
          const nextId = base.total_calls + 1;
          return {
            ...base,
            total_calls: nextId,
            recent_calls: [
              {
                id: nextId,
                tool_name: "keel_ping",
                timestamp_iso: now,
                duration_ms: 1,
                succeeded: true,
                summary: trimmed
                  ? `OK · echo="${trimmed}"`
                  : "OK · loopback pong",
                caller_hint: "browser-preview",
              },
              ...base.recent_calls,
            ].slice(0, 32),
          };
        });
        return;
      }

      const normalized =
        echoMessage && echoMessage.trim().length > 0
          ? echoMessage.trim()
          : null;
      const res = await commands.selfTestKeelPing(normalized);
      if (res.status === "ok") {
        setLastPingResult(res.data);
      } else {
        setPingError(res.error);
      }
    } catch (err) {
      setPingError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsPinging(false);
    }
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

  return {
    snapshot,
    isLoading,
    isPinging,
    lastPingResult,
    pingError,
    refreshSnapshot,
    runSelfTestPing,
  };
}
