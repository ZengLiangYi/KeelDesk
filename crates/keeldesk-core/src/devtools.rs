//! Bounded JSON-RPC protocol trace store and Specta telemetry types for KeelDesk DevTools.

use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::ToolSafetyTier;

pub const MAX_PROTOCOL_TRACES: usize = 128;
pub const MAX_TRACE_PAYLOAD_BYTES: usize = 16_384;
const TRUNCATION_SUFFIX: &str = "\n/* ... truncated ... */";

pub type ProtocolTraceNotifier = Arc<dyn Fn(McpProtocolTraceEntry) + Send + Sync>;
pub type ProtocolTraceClearedNotifier = Arc<dyn Fn(u32) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum McpTraceTransport {
    StreamableHttp,
    StdioChild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum McpTraceDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum McpTraceStatus {
    Succeeded,
    ToolError,
    ProtocolError,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum McpTraceApprovalOutcome {
    NotRequired,
    ApprovedOnce,
    ApprovedForSession,
    SessionAllowlisted,
    Denied,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct McpLatencyBreakdown {
    pub total_ms: u32,
    pub approval_wait_ms: u32,
    pub execution_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct McpProtocolTraceEntry {
    pub seq: u32,
    pub timestamp_iso: String,
    pub transport: McpTraceTransport,
    pub direction: McpTraceDirection,
    pub target: String,
    pub session_id: Option<String>,
    pub caller_hint: Option<String>,
    pub jsonrpc_id: Option<String>,
    pub method: String,
    pub tool_name: Option<String>,
    pub safety_tier: Option<ToolSafetyTier>,
    pub approval_outcome: McpTraceApprovalOutcome,
    pub status: McpTraceStatus,
    pub status_code: Option<i32>,
    pub error_message: Option<String>,
    pub latency: McpLatencyBreakdown,
    pub request_json: String,
    pub response_json: String,
    pub request_bytes: u32,
    pub response_bytes: u32,
    pub request_truncated: bool,
    pub response_truncated: bool,
    pub linked_call_id: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct NewProtocolTrace {
    pub transport: McpTraceTransport,
    pub direction: McpTraceDirection,
    pub target: String,
    pub session_id: Option<String>,
    pub caller_hint: Option<String>,
    pub jsonrpc_id: Option<String>,
    pub method: String,
    pub tool_name: Option<String>,
    pub safety_tier: Option<ToolSafetyTier>,
    pub approval_outcome: McpTraceApprovalOutcome,
    pub status: McpTraceStatus,
    pub status_code: Option<i32>,
    pub error_message: Option<String>,
    pub latency: McpLatencyBreakdown,
    pub request_value: Value,
    pub response_value: Value,
    pub linked_call_id: Option<u32>,
}

struct TraceRingBuffer {
    next_seq: u32,
    entries: VecDeque<McpProtocolTraceEntry>,
}

pub struct McpProtocolTraceStore {
    inner: RwLock<TraceRingBuffer>,
    trace_notifier: Option<ProtocolTraceNotifier>,
    cleared_notifier: Option<ProtocolTraceClearedNotifier>,
}

impl McpProtocolTraceStore {
    pub fn new(
        trace_notifier: Option<ProtocolTraceNotifier>,
        cleared_notifier: Option<ProtocolTraceClearedNotifier>,
    ) -> Self {
        Self {
            inner: RwLock::new(TraceRingBuffer {
                next_seq: 1,
                entries: VecDeque::with_capacity(MAX_PROTOCOL_TRACES),
            }),
            trace_notifier,
            cleared_notifier,
        }
    }

    pub fn record(&self, input: NewProtocolTrace) -> McpProtocolTraceEntry {
        let (request_json, request_bytes, request_truncated) =
            serialize_and_truncate_json(&input.request_value);
        let (response_json, response_bytes, response_truncated) =
            serialize_and_truncate_json(&input.response_value);

        let entry = {
            let mut guard = self
                .inner
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let seq = guard.next_seq;
            let next = seq.wrapping_add(1);
            guard.next_seq = if next == 0 { 1 } else { next };

            let entry = McpProtocolTraceEntry {
                seq,
                timestamp_iso: Utc::now().to_rfc3339(),
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
                request_json,
                response_json,
                request_bytes,
                response_bytes,
                request_truncated,
                response_truncated,
                linked_call_id: input.linked_call_id,
            };

            if guard.entries.len() >= MAX_PROTOCOL_TRACES {
                guard.entries.pop_back();
            }
            guard.entries.push_front(entry.clone());
            entry
        };

        if let Some(ref notifier) = self.trace_notifier {
            notifier(entry.clone());
        }

        entry
    }

    pub fn list(&self) -> Vec<McpProtocolTraceEntry> {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entries
            .iter()
            .cloned()
            .collect()
    }

    pub fn clear(&self) -> u32 {
        let cleared = {
            let mut guard = self
                .inner
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let count = guard.entries.len().min(u32::MAX as usize) as u32;
            guard.entries.clear();
            count
        };

        if let Some(ref notifier) = self.cleared_notifier {
            notifier(cleared);
        }

        cleared
    }
}

pub fn serialize_and_truncate_json(value: &Value) -> (String, u32, bool) {
    let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    truncate_utf8_payload(&pretty, MAX_TRACE_PAYLOAD_BYTES)
}

pub fn truncate_utf8_payload(raw: &str, max_bytes: usize) -> (String, u32, bool) {
    let total_bytes = raw.len().min(u32::MAX as usize) as u32;
    if raw.len() <= max_bytes {
        return (raw.to_string(), total_bytes, false);
    }

    let prefix_budget = max_bytes.saturating_sub(TRUNCATION_SUFFIX.len());
    let mut cut = prefix_budget.min(raw.len());
    while cut > 0 && !raw.is_char_boundary(cut) {
        cut -= 1;
    }

    let mut out = String::with_capacity(cut + TRUNCATION_SUFFIX.len());
    out.push_str(&raw[..cut]);
    out.push_str(TRUNCATION_SUFFIX);
    (out, total_bytes, true)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn truncates_multibyte_utf8_payloads_safely_at_char_boundaries() {
        // 4-byte UTF-8 emoji and 3-byte CJK characters repeated beyond 16 KiB.
        let chunk = "中🚀文";
        let repeated = chunk.repeat(3_000);
        assert!(repeated.len() > MAX_TRACE_PAYLOAD_BYTES);

        let store = McpProtocolTraceStore::new(None, None);
        let entry = store.record(NewProtocolTrace {
            transport: McpTraceTransport::StreamableHttp,
            direction: McpTraceDirection::Inbound,
            target: "keeldesk".to_string(),
            session_id: Some("sess-utf8".to_string()),
            caller_hint: Some("utf8-test".to_string()),
            jsonrpc_id: Some("1".to_string()),
            method: "tools/call".to_string(),
            tool_name: Some("echo".to_string()),
            safety_tier: Some(ToolSafetyTier::Safe),
            approval_outcome: McpTraceApprovalOutcome::NotRequired,
            status: McpTraceStatus::Succeeded,
            status_code: Some(200),
            error_message: None,
            latency: McpLatencyBreakdown {
                total_ms: 4,
                approval_wait_ms: 0,
                execution_ms: 4,
            },
            request_value: json!({ "payload": repeated }),
            response_value: json!({ "ok": true }),
            linked_call_id: Some(1),
        });

        assert!(entry.request_truncated);
        assert!(!entry.response_truncated);
        assert!(entry.request_bytes as usize > MAX_TRACE_PAYLOAD_BYTES);
        assert!(entry.request_json.len() <= MAX_TRACE_PAYLOAD_BYTES);
        assert!(entry.request_json.ends_with(TRUNCATION_SUFFIX));
        assert!(std::str::from_utf8(entry.request_json.as_bytes()).is_ok());
    }

    #[test]
    fn ring_buffer_bounds_to_max_traces_and_clear_emits_count() {
        let cleared_counts = Arc::new(std::sync::Mutex::new(Vec::new()));
        let cleared_sink = Arc::clone(&cleared_counts);
        let store = McpProtocolTraceStore::new(
            None,
            Some(Arc::new(move |count| {
                cleared_sink.lock().unwrap().push(count);
            })),
        );

        for i in 0..(MAX_PROTOCOL_TRACES + 15) {
            store.record(NewProtocolTrace {
                transport: McpTraceTransport::StreamableHttp,
                direction: McpTraceDirection::Inbound,
                target: "keeldesk".to_string(),
                session_id: None,
                caller_hint: None,
                jsonrpc_id: Some(i.to_string()),
                method: "tools/list".to_string(),
                tool_name: None,
                safety_tier: None,
                approval_outcome: McpTraceApprovalOutcome::NotRequired,
                status: McpTraceStatus::Succeeded,
                status_code: Some(200),
                error_message: None,
                latency: McpLatencyBreakdown {
                    total_ms: 1,
                    approval_wait_ms: 0,
                    execution_ms: 1,
                },
                request_value: json!({ "i": i }),
                response_value: json!({ "ok": true }),
                linked_call_id: None,
            });
        }

        let entries = store.list();
        assert_eq!(entries.len(), MAX_PROTOCOL_TRACES);
        // Newest-first ordering
        assert_eq!(entries[0].seq, (MAX_PROTOCOL_TRACES + 15) as u32);
        assert_eq!(entries[MAX_PROTOCOL_TRACES - 1].seq, 16);

        let cleared = store.clear();
        assert_eq!(cleared, MAX_PROTOCOL_TRACES as u32);
        assert!(store.list().is_empty());
        assert_eq!(
            *cleared_counts.lock().unwrap(),
            vec![MAX_PROTOCOL_TRACES as u32]
        );
    }
}
