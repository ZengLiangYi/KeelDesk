import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  events,
  type KillPortProcessResult,
  type KeelRuntimeSnapshot,
  type PendingToolApproval,
  type SessionAllowlistEntry,
  type ToolApprovalDecision,
} from "@/bindings";
import { isTauriRuntime } from "./useKeelRuntime";

export interface PreviewApprovalDrillOptions {
  port?: number | null;
  processName?: string;
  reason?: string | null;
}

interface PreviewPendingResolver {
  target: PendingToolApproval;
  port: number;
  processName: string;
  timeoutHandle: number;
  resolve: (result: KillPortProcessResult) => void;
  reject: (error: Error) => void;
}

const PREVIEW_APPROVAL_TIMEOUT_SECS = 60;

function mergePendingApproval(
  list: PendingToolApproval[],
  incoming: PendingToolApproval,
): PendingToolApproval[] {
  if (list.some((entry) => entry.request_id === incoming.request_id)) {
    return list;
  }
  return [...list, incoming];
}

export function useToolApproval(snapshot?: KeelRuntimeSnapshot | null) {
  const [pendingApprovals, setPendingApprovals] = useState<
    PendingToolApproval[]
  >(() => snapshot?.pending_approvals ?? []);
  const [previewAllowlist, setPreviewAllowlist] = useState<
    SessionAllowlistEntry[]
  >([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const previewSeqRef = useRef(1);
  const previewResolversRef = useRef<Map<string, PreviewPendingResolver>>(
    new Map(),
  );

  useEffect(() => {
    if (!isTauriRuntime() || !snapshot) {
      return;
    }
    setPendingApprovals(snapshot.pending_approvals);
  }, [snapshot]);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void events.toolApprovalRequestedEvent
      .listen((event) => {
        if (!cancelled) {
          setError(null);
          setPendingApprovals((prev) =>
            mergePendingApproval(prev, event.payload),
          );
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
  }, []);

  useEffect(() => {
    const resolvers = previewResolversRef.current;
    return () => {
      for (const entry of resolvers.values()) {
        window.clearTimeout(entry.timeoutHandle);
      }
      resolvers.clear();
    };
  }, []);

  const resolveDecision = useCallback(
    async (
      requestId: string,
      decision: ToolApprovalDecision,
    ): Promise<boolean> => {
      setIsSubmitting(true);
      setError(null);

      try {
        if (!isTauriRuntime()) {
          const resolverEntry = previewResolversRef.current.get(requestId);
          if (resolverEntry) {
            window.clearTimeout(resolverEntry.timeoutHandle);
            previewResolversRef.current.delete(requestId);
          }

          setPendingApprovals((prev) =>
            prev.filter((item) => item.request_id !== requestId),
          );

          if (decision === "approve_for_session" && resolverEntry) {
            const { session_scope, tool_name } = resolverEntry.target;
            setPreviewAllowlist((prev) => {
              if (
                prev.some(
                  (entry) =>
                    entry.session_scope === session_scope &&
                    entry.tool_name === tool_name,
                )
              ) {
                return prev;
              }
              return [
                ...prev,
                {
                  session_scope,
                  tool_name,
                  granted_at_iso: new Date().toISOString(),
                },
              ];
            });
          }

          if (resolverEntry) {
            if (decision === "deny") {
              resolverEntry.reject(
                new Error(
                  "User denied execution of 'kill_port_process' in KeelDesk approval gate.",
                ),
              );
            } else {
              resolverEntry.resolve({
                port: resolverEntry.port,
                dry_run: true,
                target_pids: [4820],
                terminated_pids: [],
                process_names: [resolverEntry.processName],
                message: `Dry-run check passed on TCP port ${resolverEntry.port} (${resolverEntry.processName}); no process was terminated.`,
              });
            }
          }

          return true;
        }

        const res = await commands.resolveKeelToolApproval(requestId, decision);
        if (res.status === "ok") {
          setPendingApprovals((prev) =>
            prev.filter((item) => item.request_id !== requestId),
          );
          return true;
        }

        setError(res.error);
        if (
          res.error.includes("not found") ||
          res.error.includes("already expired")
        ) {
          setPendingApprovals((prev) =>
            prev.filter((item) => item.request_id !== requestId),
          );
        }
        return false;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setIsSubmitting(false);
      }
    },
    [],
  );

  const approveOnce = useCallback(
    (requestId: string) => resolveDecision(requestId, "approve_once"),
    [resolveDecision],
  );

  const approveForSession = useCallback(
    (requestId: string) => resolveDecision(requestId, "approve_for_session"),
    [resolveDecision],
  );

  const deny = useCallback(
    (requestId: string) => resolveDecision(requestId, "deny"),
    [resolveDecision],
  );

  const clearSessionAllowlist = useCallback(async (): Promise<boolean> => {
    setError(null);
    if (!isTauriRuntime()) {
      setPreviewAllowlist([]);
      return true;
    }

    try {
      const res = await commands.clearKeelSessionAllowlist();
      if (res.status === "ok") {
        return true;
      }
      setError(res.error);
      return false;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return false;
    }
  }, []);

  const simulatePreviewApproval = useCallback(
    (
      options?: PreviewApprovalDrillOptions,
    ): Promise<KillPortProcessResult> => {
      const targetPort =
        options?.port && options.port > 0 ? options.port : 19527;
      const processName = options?.processName ?? "keeldesk.exe";
      const normalizedReason =
        options?.reason?.trim() ||
        "桌面端人工确认网关演练 (dry-run)";
      const sessionScope = "browser-preview";
      const toolName = "kill_port_process";

      const isAllowlisted = previewAllowlist.some(
        (entry) =>
          entry.session_scope === sessionScope && entry.tool_name === toolName,
      );
      if (isAllowlisted) {
        return Promise.resolve({
          port: targetPort,
          dry_run: true,
          target_pids: [4820],
          terminated_pids: [],
          process_names: [processName],
          message: `Session allowlisted dry-run passed on TCP port ${targetPort} (${processName}).`,
        });
      }

      const seq = previewSeqRef.current++;
      const requestId = `preview-apr-${seq}`;
      const requestedAtMs = Date.now();
      const pending: PendingToolApproval = {
        request_id: requestId,
        tool_name: toolName,
        tool_source: "workbench",
        description:
          "Terminate the process owning a listening TCP port on Windows 11 (protected by KeelDesk interactive desktop approval).",
        arguments_json: JSON.stringify(
          {
            port: targetPort,
            dry_run: true,
            reason: normalizedReason,
          },
          null,
          2,
        ),
        caller_hint: "loopback-drill",
        session_scope: sessionScope,
        requested_at_iso: new Date(requestedAtMs).toISOString(),
        expires_at_iso: new Date(
          requestedAtMs + PREVIEW_APPROVAL_TIMEOUT_SECS * 1000,
        ).toISOString(),
        timeout_secs: PREVIEW_APPROVAL_TIMEOUT_SECS,
      };

      setError(null);
      setPendingApprovals((prev) => mergePendingApproval(prev, pending));

      return new Promise<KillPortProcessResult>((resolve, reject) => {
        const timeoutHandle = window.setTimeout(() => {
          previewResolversRef.current.delete(requestId);
          setPendingApprovals((prev) =>
            prev.filter((item) => item.request_id !== requestId),
          );
          reject(
            new Error(
              "Approval request timed out after 60s; tool execution blocked.",
            ),
          );
        }, PREVIEW_APPROVAL_TIMEOUT_SECS * 1000);

        previewResolversRef.current.set(requestId, {
          target: pending,
          port: targetPort,
          processName,
          timeoutHandle,
          resolve,
          reject,
        });
      });
    },
    [previewAllowlist],
  );

  const activeApproval: PendingToolApproval | null =
    pendingApprovals[0] ?? null;
  const sessionAllowlist: SessionAllowlistEntry[] = isTauriRuntime()
    ? (snapshot?.session_allowlist ?? [])
    : previewAllowlist;

  return {
    pendingApprovals,
    activeApproval,
    sessionAllowlist,
    isSubmitting,
    error,
    approveOnce,
    approveForSession,
    deny,
    clearSessionAllowlist,
    simulatePreviewApproval,
  };
}
