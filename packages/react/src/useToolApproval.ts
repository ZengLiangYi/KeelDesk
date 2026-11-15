import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  events,
  type KeelRuntimeSnapshot,
  type PendingToolApproval,
  type SessionAllowlistEntry,
  type ToolApprovalDecision,
} from "./bindings";
import { pushPreviewProtocolTrace } from "./previewTraceBus";
import { isTauriRuntime } from "./useMcpServerState";

export interface PreviewApprovalDrillOptions {
  port?: number | null;
  processName?: string;
  reason?: string | null;
}

export interface PreviewApprovalDrillResult {
  port: number;
  dry_run: boolean;
  target_pids: number[];
  terminated_pids: number[];
  process_names: string[];
  message: string;
}

export interface UseToolApprovalResult {
  pendingApprovals: PendingToolApproval[];
  activeApproval: PendingToolApproval | null;
  sessionAllowlist: SessionAllowlistEntry[];
  isSubmitting: boolean;
  error: string | null;
  approveOnce: (requestId: string) => Promise<boolean>;
  approveForSession: (requestId: string) => Promise<boolean>;
  deny: (requestId: string) => Promise<boolean>;
  clearSessionAllowlist: () => Promise<boolean>;
  simulatePreviewApproval: (
    options?: PreviewApprovalDrillOptions,
  ) => Promise<PreviewApprovalDrillResult>;
}

interface PreviewPendingResolver {
  target: PendingToolApproval;
  port: number;
  processName: string;
  startedMs: number;
  reason: string;
  timeoutHandle: number;
  resolve: (result: PreviewApprovalDrillResult) => void;
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

function buildKillPortRequestValue(
  jsonrpcId: string,
  port: number,
  reason: string,
) {
  return {
    jsonrpc: "2.0",
    id: jsonrpcId,
    method: "tools/call",
    params: {
      name: "kill_port_process",
      arguments: {
        port,
        dry_run: true,
        reason,
      },
    },
  };
}

export function useToolApproval(
  externalSnapshot?: KeelRuntimeSnapshot | null,
): UseToolApprovalResult {
  const isExternallyControlled = externalSnapshot !== undefined;

  const [internalSnapshot, setInternalSnapshot] =
    useState<KeelRuntimeSnapshot | null>(null);
  const [pendingApprovals, setPendingApprovals] = useState<
    PendingToolApproval[]
  >(() => externalSnapshot?.pending_approvals ?? []);
  const [previewAllowlist, setPreviewAllowlist] = useState<
    SessionAllowlistEntry[]
  >([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const previewSeqRef = useRef(1);
  const previewResolversRef = useRef<Map<string, PreviewPendingResolver>>(
    new Map(),
  );

  // When called standalone without an external snapshot, subscribe to runtime snapshot updates.
  useEffect(() => {
    if (isExternallyControlled || !isTauriRuntime()) {
      return;
    }

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void commands
      .getKeelRuntimeSnapshot()
      .then((res) => {
        if (!cancelled && res.status === "ok") {
          setInternalSnapshot(res.data);
          setPendingApprovals(res.data.pending_approvals);
        }
      })
      .catch(() => {
        // Standalone fallback error surfaced via subsequent action calls
      });

    void events.keelRuntimeUpdatedEvent
      .listen((event) => {
        if (!cancelled) {
          setInternalSnapshot(event.payload);
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
  }, [isExternallyControlled]);

  const effectiveSnapshot = isExternallyControlled
    ? externalSnapshot
    : internalSnapshot;

  useEffect(() => {
    if (!isTauriRuntime() || !effectiveSnapshot) {
      return;
    }
    setPendingApprovals(effectiveSnapshot.pending_approvals);
  }, [effectiveSnapshot]);

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
            const approvalWaitMs = Math.max(
              1,
              Date.now() - resolverEntry.startedMs,
            );

            if (decision === "deny") {
              const denialReason =
                "User denied execution of 'kill_port_process' in KeelDesk approval gate.";

              pushPreviewProtocolTrace({
                transport: "streamable_http",
                direction: "inbound",
                target: "keeldesk",
                session_id: "preview-sess-drill",
                caller_hint: resolverEntry.target.caller_hint ?? "loopback-drill",
                jsonrpc_id: resolverEntry.target.request_id,
                method: "tools/call",
                tool_name: "kill_port_process",
                safety_tier: "confirm_required",
                approval_outcome: "denied",
                status: "blocked",
                status_code: 200,
                error_message: denialReason,
                latency: {
                  total_ms: approvalWaitMs,
                  approval_wait_ms: approvalWaitMs,
                  execution_ms: 0,
                },
                request_value: buildKillPortRequestValue(
                  resolverEntry.target.request_id,
                  resolverEntry.port,
                  resolverEntry.reason,
                ),
                response_value: {
                  jsonrpc: "2.0",
                  id: resolverEntry.target.request_id,
                  error: {
                    code: -32001,
                    message: denialReason,
                  },
                },
                linked_call_id: null,
              });

              resolverEntry.reject(new Error(denialReason));
            } else {
              const executionMs = 3;
              const resultPayload: PreviewApprovalDrillResult = {
                port: resolverEntry.port,
                dry_run: true,
                target_pids: [4820],
                terminated_pids: [],
                process_names: [resolverEntry.processName],
                message: `Dry-run check passed on TCP port ${resolverEntry.port} (${resolverEntry.processName}); no process was terminated.`,
              };

              pushPreviewProtocolTrace({
                transport: "streamable_http",
                direction: "inbound",
                target: "keeldesk",
                session_id: "preview-sess-drill",
                caller_hint: resolverEntry.target.caller_hint ?? "loopback-drill",
                jsonrpc_id: resolverEntry.target.request_id,
                method: "tools/call",
                tool_name: "kill_port_process",
                safety_tier: "confirm_required",
                approval_outcome:
                  decision === "approve_for_session"
                    ? "approved_for_session"
                    : "approved_once",
                status: "succeeded",
                status_code: 200,
                error_message: null,
                latency: {
                  total_ms: approvalWaitMs + executionMs,
                  approval_wait_ms: approvalWaitMs,
                  execution_ms: executionMs,
                },
                request_value: buildKillPortRequestValue(
                  resolverEntry.target.request_id,
                  resolverEntry.port,
                  resolverEntry.reason,
                ),
                response_value: {
                  jsonrpc: "2.0",
                  id: resolverEntry.target.request_id,
                  result: {
                    isError: false,
                    structuredContent: resultPayload,
                  },
                },
                linked_call_id: null,
              });

              resolverEntry.resolve(resultPayload);
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
        if (!isExternallyControlled) {
          setInternalSnapshot(res.data);
        }
        return true;
      }
      setError(res.error);
      return false;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return false;
    }
  }, [isExternallyControlled]);

  const simulatePreviewApproval = useCallback(
    (
      options?: PreviewApprovalDrillOptions,
    ): Promise<PreviewApprovalDrillResult> => {
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
        const allowlistedResult: PreviewApprovalDrillResult = {
          port: targetPort,
          dry_run: true,
          target_pids: [4820],
          terminated_pids: [],
          process_names: [processName],
          message: `Session allowlisted dry-run passed on TCP port ${targetPort} (${processName}).`,
        };

        const allowlistedRpcId = `preview-allowlisted-${previewSeqRef.current++}`;
        pushPreviewProtocolTrace({
          transport: "streamable_http",
          direction: "inbound",
          target: "keeldesk",
          session_id: "preview-sess-drill",
          caller_hint: "loopback-drill",
          jsonrpc_id: allowlistedRpcId,
          method: "tools/call",
          tool_name: toolName,
          safety_tier: "confirm_required",
          approval_outcome: "session_allowlisted",
          status: "succeeded",
          status_code: 200,
          error_message: null,
          latency: {
            total_ms: 3,
            approval_wait_ms: 0,
            execution_ms: 3,
          },
          request_value: buildKillPortRequestValue(
            allowlistedRpcId,
            targetPort,
            normalizedReason,
          ),
          response_value: {
            jsonrpc: "2.0",
            id: allowlistedRpcId,
            result: {
              isError: false,
              structuredContent: allowlistedResult,
            },
          },
          linked_call_id: null,
        });

        return Promise.resolve(allowlistedResult);
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

      return new Promise<PreviewApprovalDrillResult>((resolve, reject) => {
        const timeoutHandle = window.setTimeout(() => {
          previewResolversRef.current.delete(requestId);
          setPendingApprovals((prev) =>
            prev.filter((item) => item.request_id !== requestId),
          );
          const timeoutMsg =
            "Approval request timed out after 60s; tool execution blocked.";
          const waitMs = Math.max(
            PREVIEW_APPROVAL_TIMEOUT_SECS * 1000,
            Date.now() - requestedAtMs,
          );

          pushPreviewProtocolTrace({
            transport: "streamable_http",
            direction: "inbound",
            target: "keeldesk",
            session_id: "preview-sess-drill",
            caller_hint: "loopback-drill",
            jsonrpc_id: requestId,
            method: "tools/call",
            tool_name: toolName,
            safety_tier: "confirm_required",
            approval_outcome: "timed_out",
            status: "blocked",
            status_code: 200,
            error_message: timeoutMsg,
            latency: {
              total_ms: waitMs,
              approval_wait_ms: waitMs,
              execution_ms: 0,
            },
            request_value: buildKillPortRequestValue(
              requestId,
              targetPort,
              normalizedReason,
            ),
            response_value: {
              jsonrpc: "2.0",
              id: requestId,
              error: {
                code: -32002,
                message: timeoutMsg,
              },
            },
            linked_call_id: null,
          });

          reject(new Error(timeoutMsg));
        }, PREVIEW_APPROVAL_TIMEOUT_SECS * 1000);

        previewResolversRef.current.set(requestId, {
          target: pending,
          port: targetPort,
          processName,
          startedMs: requestedAtMs,
          reason: normalizedReason,
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
    ? (effectiveSnapshot?.session_allowlist ?? [])
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
