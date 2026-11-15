import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  commands,
  events,
  type McpProtocolTraceEntry,
} from "./bindings";
import {
  MAX_PREVIEW_PROTOCOL_TRACES,
  clearPreviewProtocolTraces,
  getPreviewProtocolTraces,
  subscribePreviewProtocolTraces,
} from "./previewTraceBus";
import { isTauriRuntime } from "./useMcpServerState";

export type DevToolsTransportFilter =
  | "all"
  | "streamable_http"
  | "stdio_child";

export type DevToolsStatusFilter = "all" | "attention" | "confirm_required";

export interface DevToolsStats {
  totalCount: number;
  errorOrBlockedCount: number;
  confirmRequiredCount: number;
  httpCount: number;
  stdioCount: number;
}

export interface UseKeelDevToolsOptions {
  enabled?: boolean;
}

export interface UseKeelDevToolsResult {
  traces: McpProtocolTraceEntry[];
  filteredTraces: McpProtocolTraceEntry[];
  isLoading: boolean;
  error: string | null;
  isPaused: boolean;
  pausedCount: number;
  togglePause: () => void;
  resumeCapture: () => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  transportFilter: DevToolsTransportFilter;
  setTransportFilter: (filter: DevToolsTransportFilter) => void;
  statusFilter: DevToolsStatusFilter;
  setStatusFilter: (filter: DevToolsStatusFilter) => void;
  selectedSeq: number | null;
  setSelectedSeq: (seq: number | null) => void;
  selectedTrace: McpProtocolTraceEntry | null;
  selectByLinkedCallId: (callId: number) => McpProtocolTraceEntry | null;
  stats: DevToolsStats;
  refreshTraces: () => Promise<McpProtocolTraceEntry[]>;
  clearTraces: () => Promise<number>;
  clearError: () => void;
}

const MAX_TRACES = MAX_PREVIEW_PROTOCOL_TRACES;

function isAttentionTrace(entry: McpProtocolTraceEntry): boolean {
  return (
    entry.status !== "succeeded" ||
    entry.approval_outcome === "denied" ||
    entry.approval_outcome === "timed_out" ||
    entry.approval_outcome === "cancelled"
  );
}

function mergeNewestTraces(
  existing: McpProtocolTraceEntry[],
  incoming: McpProtocolTraceEntry[],
): McpProtocolTraceEntry[] {
  if (incoming.length === 0) {
    return existing;
  }
  const seen = new Set<number>();
  const merged: McpProtocolTraceEntry[] = [];

  for (const item of incoming) {
    if (!seen.has(item.seq)) {
      seen.add(item.seq);
      merged.push(item);
    }
  }
  for (const item of existing) {
    if (!seen.has(item.seq)) {
      seen.add(item.seq);
      merged.push(item);
    }
  }

  return merged.slice(0, MAX_TRACES);
}

function matchesFilters(
  entry: McpProtocolTraceEntry,
  transportFilter: DevToolsTransportFilter,
  statusFilter: DevToolsStatusFilter,
  normalizedQuery: string,
): boolean {
  if (transportFilter !== "all" && entry.transport !== transportFilter) {
    return false;
  }
  if (statusFilter === "attention" && !isAttentionTrace(entry)) {
    return false;
  }
  if (
    statusFilter === "confirm_required" &&
    entry.safety_tier !== "confirm_required"
  ) {
    return false;
  }
  if (!normalizedQuery) {
    return true;
  }

  const haystacks = [
    `#${entry.seq}`,
    entry.method,
    entry.tool_name,
    entry.target,
    entry.caller_hint,
    entry.session_id,
    entry.jsonrpc_id,
    entry.error_message,
    entry.approval_outcome,
    entry.request_json,
    entry.response_json,
  ];

  return haystacks.some(
    (value) =>
      typeof value === "string" &&
      value.toLowerCase().includes(normalizedQuery),
  );
}

export function useKeelDevTools(
  options?: UseKeelDevToolsOptions,
): UseKeelDevToolsResult {
  const enabled = options?.enabled ?? true;

  const [traces, setTraces] = useState<McpProtocolTraceEntry[]>([]);
  const [isLoading, setIsLoading] = useState(enabled);
  const [error, setError] = useState<string | null>(null);
  const [isPaused, setIsPaused] = useState(false);
  const [pausedCount, setPausedCount] = useState(0);
  const [searchQuery, setSearchQuery] = useState("");
  const [transportFilter, setTransportFilter] =
    useState<DevToolsTransportFilter>("all");
  const [statusFilter, setStatusFilter] =
    useState<DevToolsStatusFilter>("all");
  const [selectedSeq, setSelectedSeq] = useState<number | null>(null);

  const isPausedRef = useRef(false);
  const pausedBufferRef = useRef<McpProtocolTraceEntry[]>([]);
  const tracesRef = useRef<McpProtocolTraceEntry[]>(traces);

  useEffect(() => {
    tracesRef.current = traces;
  }, [traces]);

  const handleIncomingTrace = useCallback((entry: McpProtocolTraceEntry) => {
    if (isPausedRef.current) {
      const deduped = mergeNewestTraces(pausedBufferRef.current, [entry]);
      pausedBufferRef.current = deduped;
      setPausedCount(deduped.length);
      return;
    }
    setTraces((prev) => mergeNewestTraces(prev, [entry]));
  }, []);

  const handleRemoteCleared = useCallback(() => {
    pausedBufferRef.current = [];
    setPausedCount(0);
    setTraces([]);
    setSelectedSeq(null);
  }, []);

  const refreshTraces = useCallback(async (): Promise<
    McpProtocolTraceEntry[]
  > => {
    if (!enabled) {
      setIsLoading(false);
      return tracesRef.current;
    }

    setError(null);
    if (!isTauriRuntime()) {
      const preview = getPreviewProtocolTraces();
      setTraces(preview);
      setIsLoading(false);
      return preview;
    }

    setIsLoading(true);
    try {
      const res = await commands.getKeelProtocolTraces();
      if (res.status === "ok") {
        const capped = res.data.slice(0, MAX_TRACES);
        setTraces(capped);
        return capped;
      }
      setError(res.error);
      return tracesRef.current;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return tracesRef.current;
    } finally {
      setIsLoading(false);
    }
  }, [enabled]);

  const clearTraces = useCallback(async (): Promise<number> => {
    setError(null);
    pausedBufferRef.current = [];
    setPausedCount(0);

    if (!isTauriRuntime()) {
      const cleared = clearPreviewProtocolTraces();
      setTraces([]);
      setSelectedSeq(null);
      return cleared;
    }

    try {
      const res = await commands.clearKeelProtocolTraces();
      if (res.status === "ok") {
        setTraces([]);
        setSelectedSeq(null);
        return res.data;
      }
      setError(res.error);
      return 0;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return 0;
    }
  }, []);

  const resumeCapture = useCallback(() => {
    isPausedRef.current = false;
    setIsPaused(false);
    const buffered = pausedBufferRef.current;
    pausedBufferRef.current = [];
    setPausedCount(0);
    if (buffered.length > 0) {
      setTraces((prev) => mergeNewestTraces(prev, buffered));
    }
  }, []);

  const togglePause = useCallback(() => {
    if (isPausedRef.current) {
      resumeCapture();
      return;
    }
    isPausedRef.current = true;
    setIsPaused(true);
  }, [resumeCapture]);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  useEffect(() => {
    if (!enabled) {
      setIsLoading(false);
      return;
    }

    void refreshTraces();

    if (!isTauriRuntime()) {
      return subscribePreviewProtocolTraces({
        onTrace: handleIncomingTrace,
        onCleared: handleRemoteCleared,
      });
    }

    let unlistenTrace: (() => void) | null = null;
    let unlistenCleared: (() => void) | null = null;
    let cancelled = false;

    void events.keelProtocolTraceEvent
      .listen((event) => {
        if (!cancelled) {
          handleIncomingTrace(event.payload);
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenTrace = fn;
        }
      });

    void events.keelProtocolTraceClearedEvent
      .listen(() => {
        if (!cancelled) {
          handleRemoteCleared();
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenCleared = fn;
        }
      });

    return () => {
      cancelled = true;
      if (unlistenTrace) {
        unlistenTrace();
      }
      if (unlistenCleared) {
        unlistenCleared();
      }
    };
  }, [enabled, handleIncomingTrace, handleRemoteCleared, refreshTraces]);

  const normalizedQuery = searchQuery.trim().toLowerCase();

  const filteredTraces = useMemo(
    () =>
      traces.filter((entry) =>
        matchesFilters(entry, transportFilter, statusFilter, normalizedQuery),
      ),
    [traces, transportFilter, statusFilter, normalizedQuery],
  );

  const selectedTrace = useMemo(() => {
    if (filteredTraces.length === 0) {
      return null;
    }
    if (selectedSeq !== null) {
      const exact = filteredTraces.find((entry) => entry.seq === selectedSeq);
      if (exact) {
        return exact;
      }
    }
    return filteredTraces[0] ?? null;
  }, [filteredTraces, selectedSeq]);

  const selectByLinkedCallId = useCallback(
    (callId: number): McpProtocolTraceEntry | null => {
      const match =
        tracesRef.current.find((entry) => entry.linked_call_id === callId) ??
        null;
      if (!match) {
        return null;
      }
      setSelectedSeq(match.seq);
      if (
        !matchesFilters(match, transportFilter, statusFilter, normalizedQuery)
      ) {
        setTransportFilter("all");
        setStatusFilter("all");
        setSearchQuery("");
      }
      return match;
    },
    [normalizedQuery, statusFilter, transportFilter],
  );

  const stats = useMemo<DevToolsStats>(() => {
    let errorOrBlockedCount = 0;
    let confirmRequiredCount = 0;
    let httpCount = 0;
    let stdioCount = 0;

    for (const entry of traces) {
      if (isAttentionTrace(entry)) {
        errorOrBlockedCount += 1;
      }
      if (entry.safety_tier === "confirm_required") {
        confirmRequiredCount += 1;
      }
      if (entry.transport === "streamable_http") {
        httpCount += 1;
      } else {
        stdioCount += 1;
      }
    }

    return {
      totalCount: traces.length,
      errorOrBlockedCount,
      confirmRequiredCount,
      httpCount,
      stdioCount,
    };
  }, [traces]);

  return {
    traces,
    filteredTraces,
    isLoading,
    error,
    isPaused,
    pausedCount,
    togglePause,
    resumeCapture,
    searchQuery,
    setSearchQuery,
    transportFilter,
    setTransportFilter,
    statusFilter,
    setStatusFilter,
    selectedSeq,
    setSelectedSeq,
    selectedTrace,
    selectByLinkedCallId,
    stats,
    refreshTraces,
    clearTraces,
    clearError,
  };
}
