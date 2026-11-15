import type {
  McpLatencyBreakdown,
  McpProtocolTraceEntry,
  McpTraceApprovalOutcome,
  McpTraceDirection,
  McpTraceStatus,
  McpTraceTransport,
  ToolSafetyTier,
} from "@/bindings";

export const MAX_PREVIEW_PROTOCOL_TRACES = 128;
const MAX_PREVIEW_PAYLOAD_BYTES = 16_384;
const TRUNCATION_SUFFIX = "\n/* ... truncated ... */";

export interface NewPreviewProtocolTrace {
  transport: McpTraceTransport;
  direction: McpTraceDirection;
  target: string;
  session_id: string | null;
  caller_hint: string | null;
  jsonrpc_id: string | null;
  method: string;
  tool_name: string | null;
  safety_tier: ToolSafetyTier | null;
  approval_outcome: McpTraceApprovalOutcome;
  status: McpTraceStatus;
  status_code: number | null;
  error_message: string | null;
  latency: McpLatencyBreakdown;
  request_value: unknown;
  response_value: unknown;
  linked_call_id: number | null;
  timestamp_iso?: string;
}

export interface PreviewTraceBusListeners {
  onTrace?: (entry: McpProtocolTraceEntry) => void;
  onCleared?: (clearedCount: number) => void;
}

const textEncoder =
  typeof TextEncoder !== "undefined" ? new TextEncoder() : null;

function utf8ByteLength(raw: string): number {
  if (textEncoder) {
    return textEncoder.encode(raw).length;
  }
  return raw.length;
}

function serializeAndTruncatePreviewJson(value: unknown): {
  json: string;
  bytes: number;
  truncated: boolean;
} {
  let pretty: string;
  if (typeof value === "string") {
    pretty = value;
  } else {
    try {
      pretty = JSON.stringify(value, null, 2) ?? String(value);
    } catch {
      pretty = String(value);
    }
  }

  const totalBytes = utf8ByteLength(pretty);
  if (totalBytes <= MAX_PREVIEW_PAYLOAD_BYTES) {
    return { json: pretty, bytes: totalBytes, truncated: false };
  }

  const budget = Math.max(0, MAX_PREVIEW_PAYLOAD_BYTES - TRUNCATION_SUFFIX.length);
  let cut = Math.min(pretty.length, budget);
  while (cut > 0 && utf8ByteLength(pretty.slice(0, cut)) > budget) {
    cut = Math.max(0, cut - Math.ceil((utf8ByteLength(pretty.slice(0, cut)) - budget) / 2));
  }

  return {
    json: `${pretty.slice(0, cut)}${TRUNCATION_SUFFIX}`,
    bytes: totalBytes,
    truncated: true,
  };
}

function buildEntry(
  seq: number,
  input: NewPreviewProtocolTrace,
): McpProtocolTraceEntry {
  const req = serializeAndTruncatePreviewJson(input.request_value);
  const res = serializeAndTruncatePreviewJson(input.response_value);

  return {
    seq,
    timestamp_iso: input.timestamp_iso ?? new Date().toISOString(),
    transport: input.transport,
    direction: input.direction,
    target: input.target,
    session_id: input.session_id,
    caller_hint: input.caller_hint,
    jsonrpc_id: input.jsonrpc_id,
    method: input.method,
    tool_name: input.tool_name,
    safety_tier: input.safety_tier,
    approval_outcome: input.approval_outcome,
    status: input.status,
    status_code: input.status_code,
    error_message: input.error_message,
    latency: input.latency,
    request_json: req.json,
    response_json: res.json,
    request_bytes: req.bytes,
    response_bytes: res.bytes,
    request_truncated: req.truncated,
    response_truncated: res.truncated,
    linked_call_id: input.linked_call_id,
  };
}

function createSeededEntries(): McpProtocolTraceEntry[] {
  const now = Date.now();
  const sessId = "preview-sess-claude-01";
  const caller = "claude-code/2.1.0";

  const seedInputs: Array<{ seq: number; input: NewPreviewProtocolTrace }> = [
    {
      seq: 1,
      input: {
        transport: "streamable_http",
        direction: "inbound",
        target: "keeldesk",
        session_id: sessId,
        caller_hint: caller,
        jsonrpc_id: "1",
        method: "initialize",
        tool_name: null,
        safety_tier: null,
        approval_outcome: "not_required",
        status: "succeeded",
        status_code: 200,
        error_message: null,
        latency: { total_ms: 2, approval_wait_ms: 0, execution_ms: 2 },
        timestamp_iso: new Date(now - 95_000).toISOString(),
        request_value: {
          jsonrpc: "2.0",
          id: 1,
          method: "initialize",
          params: {
            protocolVersion: "2025-03-26",
            capabilities: { roots: { listChanged: true } },
            clientInfo: { name: "claude-code", version: "2.1.0" },
          },
        },
        response_value: {
          jsonrpc: "2.0",
          id: 1,
          result: {
            protocolVersion: "2025-03-26",
            capabilities: { tools: { listChanged: false } },
            serverInfo: { name: "keeldesk", version: "0.1.0" },
          },
        },
        linked_call_id: null,
      },
    },
    {
      seq: 2,
      input: {
        transport: "streamable_http",
        direction: "inbound",
        target: "keeldesk",
        session_id: sessId,
        caller_hint: caller,
        jsonrpc_id: "2",
        method: "tools/list",
        tool_name: null,
        safety_tier: null,
        approval_outcome: "not_required",
        status: "succeeded",
        status_code: 200,
        error_message: null,
        latency: { total_ms: 1, approval_wait_ms: 0, execution_ms: 1 },
        timestamp_iso: new Date(now - 82_000).toISOString(),
        request_value: {
          jsonrpc: "2.0",
          id: 2,
          method: "tools/list",
          params: {},
        },
        response_value: {
          jsonrpc: "2.0",
          id: 2,
          result: {
            tools: [
              { name: "keel_ping" },
              { name: "scan_dev_env" },
              { name: "list_active_ports" },
              { name: "kill_port_process" },
            ],
          },
        },
        linked_call_id: null,
      },
    },
    {
      seq: 3,
      input: {
        transport: "streamable_http",
        direction: "inbound",
        target: "keeldesk",
        session_id: sessId,
        caller_hint: caller,
        jsonrpc_id: "3",
        method: "tools/call",
        tool_name: "keel_ping",
        safety_tier: "safe",
        approval_outcome: "not_required",
        status: "succeeded",
        status_code: 200,
        error_message: null,
        latency: { total_ms: 3, approval_wait_ms: 0, execution_ms: 3 },
        timestamp_iso: new Date(now - 54_000).toISOString(),
        request_value: {
          jsonrpc: "2.0",
          id: 3,
          method: "tools/call",
          params: {
            name: "keel_ping",
            arguments: { message: "handshake-check" },
          },
        },
        response_value: {
          jsonrpc: "2.0",
          id: 3,
          result: {
            isError: false,
            structuredContent: {
              ok: true,
              server: "keeldesk",
              version: "0.1.0",
              transport: "streamable-http",
              echo: "handshake-check",
            },
          },
        },
        linked_call_id: 1,
      },
    },
    {
      seq: 4,
      input: {
        transport: "streamable_http",
        direction: "inbound",
        target: "keeldesk",
        session_id: sessId,
        caller_hint: caller,
        jsonrpc_id: "4",
        method: "tools/call",
        tool_name: "kill_port_process",
        safety_tier: "confirm_required",
        approval_outcome: "approved_once",
        status: "succeeded",
        status_code: 200,
        error_message: null,
        latency: { total_ms: 2414, approval_wait_ms: 2410, execution_ms: 4 },
        timestamp_iso: new Date(now - 24_000).toISOString(),
        request_value: {
          jsonrpc: "2.0",
          id: 4,
          method: "tools/call",
          params: {
            name: "kill_port_process",
            arguments: {
              port: 3000,
              dry_run: true,
              reason: "Verify ConfirmRequired approval gate latency breakdown",
            },
          },
        },
        response_value: {
          jsonrpc: "2.0",
          id: 4,
          result: {
            isError: false,
            structuredContent: {
              port: 3000,
              dry_run: true,
              target_pids: [18420],
              terminated_pids: [],
              process_names: ["node.exe"],
              message:
                "Dry-run check passed on TCP port 3000 (node.exe); no process was terminated.",
            },
          },
        },
        linked_call_id: 2,
      },
    },
  ];

  // Store newest-first
  return seedInputs
    .map(({ seq, input }) => buildEntry(seq, input))
    .reverse();
}

let previewSeqCounter = 5;
let previewTraceEntries: McpProtocolTraceEntry[] = createSeededEntries();
const subscribers = new Set<PreviewTraceBusListeners>();

export function getPreviewProtocolTraces(): McpProtocolTraceEntry[] {
  return [...previewTraceEntries];
}

export function pushPreviewProtocolTrace(
  input: NewPreviewProtocolTrace,
): McpProtocolTraceEntry {
  const seq = previewSeqCounter++;
  const entry = buildEntry(seq, input);
  previewTraceEntries = [entry, ...previewTraceEntries].slice(
    0,
    MAX_PREVIEW_PROTOCOL_TRACES,
  );

  for (const listener of subscribers) {
    listener.onTrace?.(entry);
  }

  return entry;
}

export function clearPreviewProtocolTraces(): number {
  const clearedCount = previewTraceEntries.length;
  previewTraceEntries = [];

  for (const listener of subscribers) {
    listener.onCleared?.(clearedCount);
  }

  return clearedCount;
}

export function subscribePreviewProtocolTraces(
  listeners: PreviewTraceBusListeners,
): () => void {
  subscribers.add(listeners);
  return () => {
    subscribers.delete(listeners);
  };
}
