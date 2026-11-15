//! Supervised Windows 11 child stdio MCP process host with Win32 Job Object lifecycle management.

use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use chrono::Utc;
use rmcp::{
    model::{CallToolRequestParams, CallToolResult, JsonObject},
    service::RunningService,
    RoleClient, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    sync::{oneshot, Mutex as AsyncMutex, Notify},
    task::JoinHandle,
};

use super::{
    devtools::{
        McpLatencyBreakdown, McpProtocolTraceStore, McpTraceApprovalOutcome, McpTraceDirection,
        McpTraceStatus, McpTraceTransport, NewProtocolTrace,
    },
    win32_env::{
        build_merged_windows_path, decode_cli_bytes, new_hidden_std_command,
        resolve_executable_in_path,
    },
};

const MAX_STDERR_LINES: usize = 100;
const MAX_STDERR_LINE_CHARS: usize = 400;
const MCP_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(12);
const CHILD_TOOL_CALL_TIMEOUT: Duration = Duration::from_secs(15);
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(2);

const STOP_REASON_NONE: u8 = 0;
const STOP_REASON_USER: u8 = 1;
const STOP_REASON_HANDSHAKE_FAILED: u8 = 2;

pub type ProcessHostNotifier = Arc<dyn Fn(ProcessHostSnapshot) + Send + Sync>;

fn default_perform_handshake() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ChildServerStatus {
    Stopped,
    Starting,
    Running,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ChildEnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SpawnChildServerSpec {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Vec<ChildEnvVar>,
    #[serde(default = "default_perform_handshake")]
    pub perform_mcp_handshake: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ChildMcpToolSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ChildServerRecord {
    pub id: String,
    pub name: String,
    pub command: String,
    pub resolved_executable: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env_keys: Vec<String>,
    pub perform_mcp_handshake: bool,
    pub status: ChildServerStatus,
    pub pid: Option<u32>,
    pub job_object_attached: bool,
    pub started_at_iso: Option<String>,
    pub stopped_at_iso: Option<String>,
    pub last_exit_code: Option<i32>,
    pub last_error: Option<String>,
    pub server_info_name: Option<String>,
    pub server_info_version: Option<String>,
    pub handshake_ms: Option<u32>,
    pub discovered_tools: Vec<ChildMcpToolSummary>,
    pub stderr_tail: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProcessHostSnapshot {
    pub merged_path_entries: u32,
    pub running_count: u32,
    pub servers: Vec<ChildServerRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ChildToolCallResult {
    pub server_id: String,
    pub tool_name: String,
    pub duration_ms: u32,
    pub is_error: bool,
    pub response_json: String,
    pub trace_seq: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct ProcessHostUpdatedEvent(pub ProcessHostSnapshot);

#[cfg(target_os = "windows")]
struct Win32JobGuard(isize);

#[cfg(target_os = "windows")]
impl Win32JobGuard {
    fn create_kill_on_close() -> Result<Self, String> {
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::CloseHandle,
                System::JobObjects::{
                    CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
                    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                },
            },
        };

        // SAFETY: Creates an unnamed Win32 Job Object with default security attributes.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|err| format!("CreateJobObjectW failed: {err}"))?;

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        // SAFETY: `info` matches `JobObjectExtendedLimitInformation` and `handle` is valid.
        let set_res = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if let Err(err) = set_res {
            // SAFETY: Close partially initialized handle before returning error.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(format!("SetInformationJobObject failed: {err}"));
        }

        Ok(Self(handle.0 as isize))
    }

    fn assign_process(&self, raw_handle: std::os::windows::io::RawHandle) -> Result<(), String> {
        use windows::Win32::{Foundation::HANDLE, System::JobObjects::AssignProcessToJobObject};

        // SAFETY: `self.0` is an open Job Object handle and `raw_handle` is the spawned child process handle.
        unsafe { AssignProcessToJobObject(HANDLE(self.0 as *mut _), HANDLE(raw_handle)) }
            .map_err(|err| format!("AssignProcessToJobObject failed: {err}"))
    }

    fn terminate(&self, exit_code: u32) {
        use windows::Win32::{Foundation::HANDLE, System::JobObjects::TerminateJobObject};

        if self.0 != 0 {
            // SAFETY: `self.0` remains open until `Drop`.
            unsafe {
                let _ = TerminateJobObject(HANDLE(self.0 as *mut _), exit_code);
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for Win32JobGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};

        if self.0 != 0 {
            // SAFETY: `self.0` was returned by `CreateJobObjectW` and is closed once on drop.
            unsafe {
                let _ = CloseHandle(HANDLE(self.0 as *mut _));
            }
            self.0 = 0;
        }
    }
}

#[cfg(not(target_os = "windows"))]
struct Win32JobGuard;

#[cfg(not(target_os = "windows"))]
impl Win32JobGuard {
    fn create_kill_on_close() -> Result<Self, String> {
        Ok(Self)
    }

    fn assign_process(&self, _raw_handle: std::os::windows::io::RawHandle) -> Result<(), String> {
        Ok(())
    }

    fn terminate(&self, _exit_code: u32) {}
}

enum ActiveTransport {
    McpClient(RunningService<RoleClient, ()>),
    PlainStdio {
        _stdin: ChildStdin,
        stdout_task: JoinHandle<()>,
    },
}

struct ActiveChildHandle {
    generation: u64,
    job_guard: Option<Arc<Win32JobGuard>>,
    stop_reason: Arc<AtomicU8>,
    kill_tx: Option<oneshot::Sender<()>>,
    exited_notify: Arc<Notify>,
    exited_flag: Arc<AtomicBool>,
    transport: Option<ActiveTransport>,
    stderr_task: Option<JoinHandle<()>>,
}

impl ActiveChildHandle {
    fn terminate_and_signal(&mut self, reason: u8) {
        self.stop_reason.store(reason, Ordering::Release);

        if let Some(transport) = self.transport.take() {
            match transport {
                ActiveTransport::McpClient(client) => {
                    client.cancellation_token().cancel();
                    drop(client);
                }
                ActiveTransport::PlainStdio {
                    _stdin,
                    stdout_task,
                } => {
                    drop(_stdin);
                    stdout_task.abort();
                }
            }
        }

        if let Some(ref job) = self.job_guard {
            job.terminate(0);
        }

        if let Some(tx) = self.kill_tx.take() {
            let _ = tx.send(());
        }
    }
}

struct StoredServerEntry {
    record: ChildServerRecord,
    stderr_lines: VecDeque<String>,
    generation: u64,
}

struct ProcessHostInner {
    merged_path_entries: u32,
    next_generation: u64,
    order: Vec<String>,
    entries: HashMap<String, StoredServerEntry>,
    active: HashMap<String, ActiveChildHandle>,
}

pub struct KeelProcessHost {
    inner: Arc<Mutex<ProcessHostInner>>,
    op_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    notifier: Option<ProcessHostNotifier>,
    trace_store: Option<Arc<McpProtocolTraceStore>>,
}

impl KeelProcessHost {
    pub fn new(
        notifier: Option<ProcessHostNotifier>,
        trace_store: Option<Arc<McpProtocolTraceStore>>,
    ) -> Self {
        let (_, merged_path_entries) = build_merged_windows_path();
        Self {
            inner: Arc::new(Mutex::new(ProcessHostInner {
                merged_path_entries,
                next_generation: 1,
                order: Vec::new(),
                entries: HashMap::new(),
                active: HashMap::new(),
            })),
            op_locks: Mutex::new(HashMap::new()),
            notifier,
            trace_store,
        }
    }

    pub fn snapshot(&self) -> ProcessHostSnapshot {
        build_snapshot_from_inner(&self.inner)
    }

    fn emit_snapshot(&self) {
        if let Some(ref notifier) = self.notifier {
            notifier(self.snapshot());
        }
    }

    fn server_op_lock(&self, server_id: &str) -> Arc<AsyncMutex<()>> {
        let mut guard = self
            .op_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .entry(server_id.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    pub async fn start_server(
        &self,
        spec: SpawnChildServerSpec,
    ) -> Result<ChildServerRecord, String> {
        let validated = validate_spawn_spec(spec)?;
        let op_lock = self.server_op_lock(&validated.id);
        let _op_guard = op_lock.lock().await;

        // If an active handle already exists for this ID, stop it cleanly before restarting.
        let prev_handle = {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.active.remove(&validated.id)
        };
        if let Some(mut handle) = prev_handle {
            stop_active_handle(&mut handle).await;
        }

        let (merged_path, merged_path_entries) = build_merged_windows_path();
        let env_keys: Vec<String> = validated.env.iter().map(|e| e.key.clone()).collect();

        let generation = {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.merged_path_entries = merged_path_entries;
            let generation = guard.next_generation;
            guard.next_generation = guard.next_generation.saturating_add(1);

            if !guard.order.iter().any(|existing| existing == &validated.id) {
                guard.order.push(validated.id.clone());
            }

            guard.entries.insert(
                validated.id.clone(),
                StoredServerEntry {
                    record: ChildServerRecord {
                        id: validated.id.clone(),
                        name: validated.name.clone(),
                        command: validated.command.clone(),
                        resolved_executable: None,
                        args: validated.args.clone(),
                        cwd: validated.cwd.clone(),
                        env_keys: env_keys.clone(),
                        perform_mcp_handshake: validated.perform_mcp_handshake,
                        status: ChildServerStatus::Starting,
                        pid: None,
                        job_object_attached: false,
                        started_at_iso: Some(Utc::now().to_rfc3339()),
                        stopped_at_iso: None,
                        last_exit_code: None,
                        last_error: None,
                        server_info_name: None,
                        server_info_version: None,
                        handshake_ms: None,
                        discovered_tools: Vec::new(),
                        stderr_tail: Vec::new(),
                    },
                    stderr_lines: VecDeque::with_capacity(MAX_STDERR_LINES),
                    generation,
                },
            );

            generation
        };
        self.emit_snapshot();

        let Some(resolved_exe) = resolve_executable_in_path(&validated.command, &merged_path)
        else {
            let err_msg = format!(
                "Executable '{}' could not be resolved in the merged Windows PATH.",
                validated.command
            );
            self.mark_entry_failed(&validated.id, generation, err_msg.clone());
            return Err(err_msg);
        };
        let resolved_exe_str = resolved_exe.to_string_lossy().into_owned();

        let std_cmd = match build_child_std_command(
            &resolved_exe,
            &validated.args,
            validated.cwd.as_deref(),
            &merged_path,
            &validated.env,
        ) {
            Ok(cmd) => cmd,
            Err(err_msg) => {
                self.mark_entry_failed_with_exe(
                    &validated.id,
                    generation,
                    Some(resolved_exe_str),
                    err_msg.clone(),
                );
                return Err(err_msg);
            }
        };

        let mut tokio_cmd = Command::from(std_cmd);
        tokio_cmd.kill_on_drop(true);

        let mut child = match tokio_cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                let err_msg = format!(
                    "Failed to spawn child server '{}' ({}): {err}",
                    validated.id, resolved_exe_str
                );
                self.mark_entry_failed_with_exe(
                    &validated.id,
                    generation,
                    Some(resolved_exe_str),
                    err_msg.clone(),
                );
                return Err(err_msg);
            }
        };

        let pid = child.id();

        let (job_guard, job_object_attached) = match Win32JobGuard::create_kill_on_close() {
            Ok(job) => {
                let attached = if let Some(raw_handle) = child.raw_handle() {
                    job.assign_process(raw_handle).is_ok()
                } else {
                    false
                };
                (Some(Arc::new(job)), attached)
            }
            Err(_) => (None, false),
        };

        let Some(stdin) = child.stdin.take() else {
            let err_msg = "Spawned child process did not provide a piped stdin handle.".to_string();
            if let Some(ref job) = job_guard {
                job.terminate(1);
            }
            let _ = child.start_kill();
            self.mark_entry_failed_with_exe(
                &validated.id,
                generation,
                Some(resolved_exe_str),
                err_msg.clone(),
            );
            return Err(err_msg);
        };
        let Some(stdout) = child.stdout.take() else {
            let err_msg =
                "Spawned child process did not provide a piped stdout handle.".to_string();
            if let Some(ref job) = job_guard {
                job.terminate(1);
            }
            let _ = child.start_kill();
            self.mark_entry_failed_with_exe(
                &validated.id,
                generation,
                Some(resolved_exe_str),
                err_msg.clone(),
            );
            return Err(err_msg);
        };
        let stderr = child.stderr.take();

        {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(entry) = guard.entries.get_mut(&validated.id) {
                if entry.generation == generation {
                    entry.record.resolved_executable = Some(resolved_exe_str.clone());
                    entry.record.pid = pid;
                    entry.record.job_object_attached = job_object_attached;
                }
            }
        }

        let stderr_task = stderr.map(|err_pipe| {
            spawn_stderr_reader(
                err_pipe,
                validated.id.clone(),
                generation,
                Arc::clone(&self.inner),
                self.notifier.clone(),
            )
        });

        let stop_reason = Arc::new(AtomicU8::new(STOP_REASON_NONE));
        let exited_notify = Arc::new(Notify::new());
        let exited_flag = Arc::new(AtomicBool::new(false));
        let (kill_tx, kill_rx) = oneshot::channel();

        spawn_exit_watcher(
            child,
            validated.id.clone(),
            generation,
            Arc::clone(&self.inner),
            self.notifier.clone(),
            Arc::clone(&stop_reason),
            kill_rx,
            Arc::clone(&exited_notify),
            Arc::clone(&exited_flag),
        );

        let mut active_handle = ActiveChildHandle {
            generation,
            job_guard: job_guard.clone(),
            stop_reason: Arc::clone(&stop_reason),
            kill_tx: Some(kill_tx),
            exited_notify: Arc::clone(&exited_notify),
            exited_flag: Arc::clone(&exited_flag),
            transport: None,
            stderr_task,
        };

        if validated.perform_mcp_handshake {
            let handshake_started = Instant::now();
            let init_completed = Arc::new(AtomicBool::new(false));
            let handshake_res = tokio::time::timeout(
                MCP_HANDSHAKE_TIMEOUT,
                perform_stdio_mcp_handshake(
                    &validated.id,
                    stdout,
                    stdin,
                    self.trace_store.as_ref(),
                    &init_completed,
                ),
            )
            .await;

            match handshake_res {
                Ok(Ok((client, server_info_name, server_info_version, discovered_tools))) => {
                    let handshake_ms = handshake_started
                        .elapsed()
                        .as_millis()
                        .min(u32::MAX as u128) as u32;

                    active_handle.transport = Some(ActiveTransport::McpClient(client));

                    let outcome = {
                        let mut guard = self
                            .inner
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if active_handle.exited_flag.load(Ordering::Acquire) {
                            let err_msg = guard
                                .entries
                                .get(&validated.id)
                                .and_then(|entry| entry.record.last_error.clone())
                                .unwrap_or_else(|| {
                                    "Child server exited immediately after MCP handshake."
                                        .to_string()
                                });
                            Err(err_msg)
                        } else {
                            guard.active.insert(validated.id.clone(), active_handle);
                            let entry = guard
                                .entries
                                .get_mut(&validated.id)
                                .expect("server entry must exist");
                            entry.record.status = ChildServerStatus::Running;
                            entry.record.server_info_name = server_info_name;
                            entry.record.server_info_version = server_info_version;
                            entry.record.handshake_ms = Some(handshake_ms);
                            entry.record.discovered_tools = discovered_tools;
                            entry.record.last_error = None;
                            Ok(entry.record.clone())
                        }
                    };
                    self.emit_snapshot();
                    outcome
                }
                Ok(Err(err_msg)) => {
                    active_handle.terminate_and_signal(STOP_REASON_HANDSHAKE_FAILED);
                    wait_for_exit_or_stderr(&mut active_handle).await;
                    self.mark_entry_failed(&validated.id, generation, err_msg.clone());
                    Err(err_msg)
                }
                Err(_) => {
                    let err_msg = format!(
                        "Timed out after {}s waiting for MCP stdio initialize/tools/list handshake from '{}'.",
                        MCP_HANDSHAKE_TIMEOUT.as_secs(),
                        validated.id
                    );
                    if let Some(ref store) = self.trace_store {
                        let elapsed_ms = handshake_started
                            .elapsed()
                            .as_millis()
                            .min(u32::MAX as u128) as u32;
                        let method = if init_completed.load(Ordering::Acquire) {
                            "tools/list"
                        } else {
                            "initialize"
                        };
                        store.record(NewProtocolTrace {
                            transport: McpTraceTransport::StdioChild,
                            direction: McpTraceDirection::Outbound,
                            target: format!("child:{}", validated.id),
                            session_id: None,
                            caller_hint: Some("keeldesk-process-host".to_string()),
                            jsonrpc_id: None,
                            method: method.to_string(),
                            tool_name: None,
                            safety_tier: None,
                            approval_outcome: McpTraceApprovalOutcome::NotRequired,
                            status: McpTraceStatus::ProtocolError,
                            status_code: None,
                            error_message: Some(err_msg.clone()),
                            latency: McpLatencyBreakdown {
                                total_ms: elapsed_ms,
                                approval_wait_ms: 0,
                                execution_ms: elapsed_ms,
                            },
                            request_value: json!({
                                "jsonrpc": "2.0",
                                "method": method,
                                "params": {}
                            }),
                            response_value: json!({
                                "jsonrpc": "2.0",
                                "error": {
                                    "message": err_msg,
                                }
                            }),
                            linked_call_id: None,
                        });
                    }
                    active_handle.terminate_and_signal(STOP_REASON_HANDSHAKE_FAILED);
                    wait_for_exit_or_stderr(&mut active_handle).await;
                    self.mark_entry_failed(&validated.id, generation, err_msg.clone());
                    Err(err_msg)
                }
            }
        } else {
            let stdout_task = spawn_quiet_stdout_drainer(stdout);
            let outcome = {
                let mut guard = self
                    .inner
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if active_handle.exited_flag.load(Ordering::Acquire) {
                    stdout_task.abort();
                    let err_msg = guard
                        .entries
                        .get(&validated.id)
                        .and_then(|entry| entry.record.last_error.clone())
                        .unwrap_or_else(|| {
                            "Child process exited immediately after spawn.".to_string()
                        });
                    Err(err_msg)
                } else {
                    active_handle.transport = Some(ActiveTransport::PlainStdio {
                        _stdin: stdin,
                        stdout_task,
                    });
                    guard.active.insert(validated.id.clone(), active_handle);
                    let entry = guard
                        .entries
                        .get_mut(&validated.id)
                        .expect("server entry must exist");
                    entry.record.status = ChildServerStatus::Running;
                    entry.record.last_error = None;
                    Ok(entry.record.clone())
                }
            };
            self.emit_snapshot();
            outcome
        }
    }

    pub async fn stop_server(&self, server_id: &str) -> Result<ChildServerRecord, String> {
        let trimmed_id = server_id.trim();
        validate_server_id(trimmed_id)?;

        let op_lock = self.server_op_lock(trimmed_id);
        let _op_guard = op_lock.lock().await;

        let active_handle = {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !guard.entries.contains_key(trimmed_id) {
                return Err(format!("Child MCP server '{trimmed_id}' was not found."));
            }
            guard.active.remove(trimmed_id)
        };

        if let Some(mut handle) = active_handle {
            let generation = handle.generation;
            stop_active_handle(&mut handle).await;

            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(entry) = guard.entries.get_mut(trimmed_id) {
                if entry.generation == generation
                    && entry.record.status != ChildServerStatus::Stopped
                {
                    entry.record.status = ChildServerStatus::Stopped;
                    if entry.record.stopped_at_iso.is_none() {
                        entry.record.stopped_at_iso = Some(Utc::now().to_rfc3339());
                    }
                }
            }
        }

        let record = {
            let guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard
                .entries
                .get(trimmed_id)
                .map(|e| e.record.clone())
                .ok_or_else(|| format!("Child MCP server '{trimmed_id}' was not found."))?
        };
        self.emit_snapshot();
        Ok(record)
    }

    pub async fn remove_server(&self, server_id: &str) -> Result<ProcessHostSnapshot, String> {
        let trimmed_id = server_id.trim();
        validate_server_id(trimmed_id)?;

        let op_lock = self.server_op_lock(trimmed_id);
        let _op_guard = op_lock.lock().await;

        let active_handle = {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !guard.entries.contains_key(trimmed_id) {
                return Err(format!("Child MCP server '{trimmed_id}' was not found."));
            }
            guard.active.remove(trimmed_id)
        };

        if let Some(mut handle) = active_handle {
            stop_active_handle(&mut handle).await;
        }

        {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.entries.remove(trimmed_id);
            guard.order.retain(|id| id != trimmed_id);
        }

        if let Ok(mut locks) = self.op_locks.lock() {
            locks.remove(trimmed_id);
        }

        let snap = self.snapshot();
        self.emit_snapshot();
        Ok(snap)
    }

    pub async fn call_server_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments_json: Option<String>,
    ) -> Result<ChildToolCallResult, String> {
        let trimmed_id = server_id.trim();
        validate_server_id(trimmed_id)?;

        let trimmed_tool = tool_name.trim();
        if trimmed_tool.is_empty() || trimmed_tool.contains(['\0', '\r', '\n']) {
            return Err(
                "Tool name must be non-empty without NUL or newline characters.".to_string(),
            );
        }

        let args: JsonObject = match arguments_json.as_deref().map(str::trim) {
            None | Some("") => JsonObject::new(),
            Some(raw) => {
                let parsed: serde_json::Value = serde_json::from_str(raw)
                    .map_err(|err| format!("Invalid tool arguments JSON: {err}"))?;
                match parsed {
                    serde_json::Value::Object(map) => map,
                    _ => {
                        return Err(
                            "Tool arguments JSON must be a JSON object (e.g. {\"key\": \"value\"})."
                                .to_string(),
                        );
                    }
                }
            }
        };

        let peer = {
            let guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(entry) = guard.entries.get(trimmed_id) else {
                return Err(format!("Child MCP server '{trimmed_id}' was not found."));
            };
            match guard.active.get(trimmed_id) {
                Some(ActiveChildHandle {
                    transport: Some(ActiveTransport::McpClient(ref client)),
                    ..
                }) => client.peer().clone(),
                Some(ActiveChildHandle {
                    transport: Some(ActiveTransport::PlainStdio { .. }),
                    ..
                }) => {
                    return Err(format!(
                        "Child server '{trimmed_id}' was started without an MCP stdio handshake."
                    ));
                }
                _ => {
                    return Err(format!(
                        "Child MCP server '{trimmed_id}' is not currently running (status: {:?}).",
                        entry.record.status
                    ));
                }
            }
        };

        let target = format!("child:{trimmed_id}");
        let req_envelope = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": trimmed_tool,
                "arguments": &args,
            }
        });

        let started = Instant::now();
        let call_outcome = tokio::time::timeout(
            CHILD_TOOL_CALL_TIMEOUT,
            peer.call_tool(
                CallToolRequestParams::new(trimmed_tool.to_string()).with_arguments(args),
            ),
        )
        .await;
        let duration_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

        match call_outcome {
            Ok(Ok(call_res)) => {
                let is_error = call_res.is_error == Some(true);
                let resp_value = serde_json::to_value(&call_res).unwrap_or_else(|_| json!({}));
                let response_json = serde_json::to_string_pretty(&resp_value)
                    .unwrap_or_else(|_| resp_value.to_string());
                let status = if is_error {
                    McpTraceStatus::ToolError
                } else {
                    McpTraceStatus::Succeeded
                };
                let error_message = if is_error {
                    extract_child_tool_error_text(&call_res)
                } else {
                    None
                };

                let trace_seq = self.trace_store.as_ref().map(|store| {
                    store
                        .record(NewProtocolTrace {
                            transport: McpTraceTransport::StdioChild,
                            direction: McpTraceDirection::Outbound,
                            target: target.clone(),
                            session_id: None,
                            caller_hint: Some("keeldesk-process-host".to_string()),
                            jsonrpc_id: None,
                            method: "tools/call".to_string(),
                            tool_name: Some(trimmed_tool.to_string()),
                            safety_tier: None,
                            approval_outcome: McpTraceApprovalOutcome::NotRequired,
                            status,
                            status_code: None,
                            error_message,
                            latency: McpLatencyBreakdown {
                                total_ms: duration_ms,
                                approval_wait_ms: 0,
                                execution_ms: duration_ms,
                            },
                            request_value: req_envelope,
                            response_value: json!({
                                "jsonrpc": "2.0",
                                "result": resp_value,
                            }),
                            linked_call_id: None,
                        })
                        .seq
                });

                Ok(ChildToolCallResult {
                    server_id: trimmed_id.to_string(),
                    tool_name: trimmed_tool.to_string(),
                    duration_ms,
                    is_error,
                    response_json,
                    trace_seq,
                })
            }
            Ok(Err(err)) => {
                let err_msg =
                    format!("Child server '{trimmed_id}' tool '{trimmed_tool}' call failed: {err}");
                if let Some(ref store) = self.trace_store {
                    store.record(NewProtocolTrace {
                        transport: McpTraceTransport::StdioChild,
                        direction: McpTraceDirection::Outbound,
                        target,
                        session_id: None,
                        caller_hint: Some("keeldesk-process-host".to_string()),
                        jsonrpc_id: None,
                        method: "tools/call".to_string(),
                        tool_name: Some(trimmed_tool.to_string()),
                        safety_tier: None,
                        approval_outcome: McpTraceApprovalOutcome::NotRequired,
                        status: McpTraceStatus::ProtocolError,
                        status_code: None,
                        error_message: Some(err_msg.clone()),
                        latency: McpLatencyBreakdown {
                            total_ms: duration_ms,
                            approval_wait_ms: 0,
                            execution_ms: duration_ms,
                        },
                        request_value: req_envelope,
                        response_value: json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "message": err.to_string(),
                            }
                        }),
                        linked_call_id: None,
                    });
                }
                Err(err_msg)
            }
            Err(_) => {
                let err_msg = format!(
                    "Timed out after {}s calling tool '{trimmed_tool}' on child server '{trimmed_id}'.",
                    CHILD_TOOL_CALL_TIMEOUT.as_secs()
                );
                if let Some(ref store) = self.trace_store {
                    store.record(NewProtocolTrace {
                        transport: McpTraceTransport::StdioChild,
                        direction: McpTraceDirection::Outbound,
                        target,
                        session_id: None,
                        caller_hint: Some("keeldesk-process-host".to_string()),
                        jsonrpc_id: None,
                        method: "tools/call".to_string(),
                        tool_name: Some(trimmed_tool.to_string()),
                        safety_tier: None,
                        approval_outcome: McpTraceApprovalOutcome::NotRequired,
                        status: McpTraceStatus::ProtocolError,
                        status_code: None,
                        error_message: Some(err_msg.clone()),
                        latency: McpLatencyBreakdown {
                            total_ms: duration_ms,
                            approval_wait_ms: 0,
                            execution_ms: duration_ms,
                        },
                        request_value: req_envelope,
                        response_value: json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "message": err_msg,
                            }
                        }),
                        linked_call_id: None,
                    });
                }
                Err(err_msg)
            }
        }
    }

    pub fn shutdown_all(&self) {
        let now_iso = Utc::now().to_rfc3339();
        let drained = {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let drained: Vec<(String, ActiveChildHandle)> = guard.active.drain().collect();
            for (id, _) in &drained {
                if let Some(entry) = guard.entries.get_mut(id) {
                    if matches!(
                        entry.record.status,
                        ChildServerStatus::Starting | ChildServerStatus::Running
                    ) {
                        entry.record.status = ChildServerStatus::Stopped;
                        entry.record.stopped_at_iso = Some(now_iso.clone());
                    }
                }
            }
            drained
        };

        for (_, mut handle) in drained {
            handle.terminate_and_signal(STOP_REASON_USER);
            if let Some(stderr_task) = handle.stderr_task.take() {
                stderr_task.abort();
            }
        }

        self.emit_snapshot();
    }

    fn mark_entry_failed(&self, server_id: &str, generation: u64, err_msg: String) {
        self.mark_entry_failed_with_exe(server_id, generation, None, err_msg);
    }

    fn mark_entry_failed_with_exe(
        &self,
        server_id: &str,
        generation: u64,
        resolved_executable: Option<String>,
        err_msg: String,
    ) {
        {
            let mut guard = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.active.remove(server_id);
            if let Some(entry) = guard.entries.get_mut(server_id) {
                if entry.generation == generation {
                    if resolved_executable.is_some() {
                        entry.record.resolved_executable = resolved_executable;
                    }
                    entry.record.status = ChildServerStatus::Failed;
                    entry.record.last_error = Some(err_msg);
                    if entry.record.stopped_at_iso.is_none() {
                        entry.record.stopped_at_iso = Some(Utc::now().to_rfc3339());
                    }
                }
            }
        }
        self.emit_snapshot();
    }
}

fn build_snapshot_from_inner(inner: &Arc<Mutex<ProcessHostInner>>) -> ProcessHostSnapshot {
    let guard = inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut servers = Vec::with_capacity(guard.order.len());
    let mut running_count: u32 = 0;

    for id in &guard.order {
        if let Some(entry) = guard.entries.get(id) {
            if entry.record.status == ChildServerStatus::Running {
                running_count = running_count.saturating_add(1);
            }
            servers.push(entry.record.clone());
        }
    }

    ProcessHostSnapshot {
        merged_path_entries: guard.merged_path_entries,
        running_count,
        servers,
    }
}

fn extract_child_tool_error_text(res: &CallToolResult) -> Option<String> {
    res.content
        .iter()
        .find_map(|block| block.as_text().map(|t| t.text.trim()))
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

async fn perform_stdio_mcp_handshake(
    server_id: &str,
    stdout: ChildStdout,
    stdin: ChildStdin,
    trace_store: Option<&Arc<McpProtocolTraceStore>>,
    init_completed: &Arc<AtomicBool>,
) -> Result<
    (
        RunningService<RoleClient, ()>,
        Option<String>,
        Option<String>,
        Vec<ChildMcpToolSummary>,
    ),
    String,
> {
    let target = format!("child:{server_id}");
    let init_started = Instant::now();
    let client = match ().serve((stdout, stdin)).await {
        Ok(client) => {
            init_completed.store(true, Ordering::Release);
            let init_ms = init_started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            if let Some(store) = trace_store {
                store.record(NewProtocolTrace {
                    transport: McpTraceTransport::StdioChild,
                    direction: McpTraceDirection::Outbound,
                    target: target.clone(),
                    session_id: None,
                    caller_hint: Some("keeldesk-process-host".to_string()),
                    jsonrpc_id: None,
                    method: "initialize".to_string(),
                    tool_name: None,
                    safety_tier: None,
                    approval_outcome: McpTraceApprovalOutcome::NotRequired,
                    status: McpTraceStatus::Succeeded,
                    status_code: None,
                    error_message: None,
                    latency: McpLatencyBreakdown {
                        total_ms: init_ms,
                        approval_wait_ms: 0,
                        execution_ms: init_ms,
                    },
                    request_value: json!({
                        "jsonrpc": "2.0",
                        "method": "initialize",
                        "params": {
                            "clientInfo": {
                                "name": "keeldesk",
                                "version": env!("CARGO_PKG_VERSION"),
                            },
                            "capabilities": {}
                        }
                    }),
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "result": client.peer_info(),
                    }),
                    linked_call_id: None,
                });
            }
            client
        }
        Err(err) => {
            let init_ms = init_started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            let err_msg = format!("MCP stdio initialize handshake failed: {err}");
            if let Some(store) = trace_store {
                store.record(NewProtocolTrace {
                    transport: McpTraceTransport::StdioChild,
                    direction: McpTraceDirection::Outbound,
                    target,
                    session_id: None,
                    caller_hint: Some("keeldesk-process-host".to_string()),
                    jsonrpc_id: None,
                    method: "initialize".to_string(),
                    tool_name: None,
                    safety_tier: None,
                    approval_outcome: McpTraceApprovalOutcome::NotRequired,
                    status: McpTraceStatus::ProtocolError,
                    status_code: None,
                    error_message: Some(err_msg.clone()),
                    latency: McpLatencyBreakdown {
                        total_ms: init_ms,
                        approval_wait_ms: 0,
                        execution_ms: init_ms,
                    },
                    request_value: json!({
                        "jsonrpc": "2.0",
                        "method": "initialize",
                        "params": {
                            "clientInfo": {
                                "name": "keeldesk",
                                "version": env!("CARGO_PKG_VERSION"),
                            },
                            "capabilities": {}
                        }
                    }),
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "message": err.to_string(),
                        }
                    }),
                    linked_call_id: None,
                });
            }
            return Err(err_msg);
        }
    };

    let (server_info_name, server_info_version) = match client.peer_info() {
        Some(info) => (
            info.server_info.as_ref().map(|s| s.name.clone()),
            info.server_info.as_ref().map(|s| s.version.clone()),
        ),
        None => (None, None),
    };

    let list_started = Instant::now();
    let tools = match client.list_all_tools().await {
        Ok(list) => {
            let list_ms = list_started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            if let Some(store) = trace_store {
                store.record(NewProtocolTrace {
                    transport: McpTraceTransport::StdioChild,
                    direction: McpTraceDirection::Outbound,
                    target,
                    session_id: None,
                    caller_hint: Some("keeldesk-process-host".to_string()),
                    jsonrpc_id: None,
                    method: "tools/list".to_string(),
                    tool_name: None,
                    safety_tier: None,
                    approval_outcome: McpTraceApprovalOutcome::NotRequired,
                    status: McpTraceStatus::Succeeded,
                    status_code: None,
                    error_message: None,
                    latency: McpLatencyBreakdown {
                        total_ms: list_ms,
                        approval_wait_ms: 0,
                        execution_ms: list_ms,
                    },
                    request_value: json!({
                        "jsonrpc": "2.0",
                        "method": "tools/list",
                        "params": {}
                    }),
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "tools": &list,
                        }
                    }),
                    linked_call_id: None,
                });
            }
            list.into_iter()
                .map(|tool| ChildMcpToolSummary {
                    name: tool.name.to_string(),
                    description: tool.description.as_deref().unwrap_or_default().to_string(),
                })
                .collect()
        }
        Err(err) => {
            let list_ms = list_started.elapsed().as_millis().min(u32::MAX as u128) as u32;
            let err_msg = format!("MCP stdio tools/list failed: {err}");
            if let Some(store) = trace_store {
                store.record(NewProtocolTrace {
                    transport: McpTraceTransport::StdioChild,
                    direction: McpTraceDirection::Outbound,
                    target,
                    session_id: None,
                    caller_hint: Some("keeldesk-process-host".to_string()),
                    jsonrpc_id: None,
                    method: "tools/list".to_string(),
                    tool_name: None,
                    safety_tier: None,
                    approval_outcome: McpTraceApprovalOutcome::NotRequired,
                    status: McpTraceStatus::ProtocolError,
                    status_code: None,
                    error_message: Some(err_msg.clone()),
                    latency: McpLatencyBreakdown {
                        total_ms: list_ms,
                        approval_wait_ms: 0,
                        execution_ms: list_ms,
                    },
                    request_value: json!({
                        "jsonrpc": "2.0",
                        "method": "tools/list",
                        "params": {}
                    }),
                    response_value: json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "message": err.to_string(),
                        }
                    }),
                    linked_call_id: None,
                });
            }
            client.cancellation_token().cancel();
            drop(client);
            return Err(err_msg);
        }
    };

    Ok((client, server_info_name, server_info_version, tools))
}

fn spawn_quiet_stdout_drainer(mut stdout: ChildStdout) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = tokio::io::copy(&mut stdout, &mut tokio::io::sink()).await;
    })
}

fn spawn_stderr_reader(
    stderr: ChildStderr,
    server_id: String,
    generation: u64,
    inner: Arc<Mutex<ProcessHostInner>>,
    notifier: Option<ProcessHostNotifier>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line_buf = Vec::with_capacity(256);

        loop {
            line_buf.clear();
            match reader.read_until(b'\n', &mut line_buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let decoded = decode_cli_bytes(&line_buf);
                    let trimmed = decoded.trim_end_matches(['\r', '\n']);
                    if trimmed.is_empty() {
                        continue;
                    }
                    let line = truncate_stderr_line(trimmed);

                    let updated = {
                        let mut guard = inner
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if let Some(entry) = guard.entries.get_mut(&server_id) {
                            if entry.generation == generation {
                                if entry.stderr_lines.len() >= MAX_STDERR_LINES {
                                    entry.stderr_lines.pop_front();
                                }
                                entry.stderr_lines.push_back(line);
                                entry.record.stderr_tail =
                                    entry.stderr_lines.iter().cloned().collect();
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };

                    if updated {
                        if let Some(ref cb) = notifier {
                            cb(build_snapshot_from_inner(&inner));
                        }
                    }
                }
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_exit_watcher(
    mut child: Child,
    server_id: String,
    generation: u64,
    inner: Arc<Mutex<ProcessHostInner>>,
    notifier: Option<ProcessHostNotifier>,
    stop_reason: Arc<AtomicU8>,
    mut kill_rx: oneshot::Receiver<()>,
    exited_notify: Arc<Notify>,
    exited_flag: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let exit_status = tokio::select! {
            res = child.wait() => res,
            _ = &mut kill_rx => {
                let _ = child.start_kill();
                child.wait().await
            }
        };

        let exit_code = exit_status.as_ref().ok().and_then(|s| s.code());
        let reason = stop_reason.load(Ordering::Acquire);

        let should_emit = {
            let mut guard = inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            if guard
                .active
                .get(&server_id)
                .is_some_and(|handle| handle.generation == generation)
            {
                guard.active.remove(&server_id);
            }

            if let Some(entry) = guard.entries.get_mut(&server_id) {
                if entry.generation == generation {
                    entry.record.last_exit_code = exit_code;
                    if entry.record.stopped_at_iso.is_none() {
                        entry.record.stopped_at_iso = Some(Utc::now().to_rfc3339());
                    }

                    match reason {
                        STOP_REASON_USER => {
                            entry.record.status = ChildServerStatus::Stopped;
                        }
                        STOP_REASON_HANDSHAKE_FAILED => {
                            entry.record.status = ChildServerStatus::Failed;
                        }
                        _ => {
                            entry.record.status = ChildServerStatus::Failed;
                            if entry.record.last_error.is_none() {
                                entry.record.last_error = Some(match exit_code {
                                    Some(code) => {
                                        format!(
                                            "Child process exited unexpectedly with code {code}."
                                        )
                                    }
                                    None => match &exit_status {
                                        Ok(_) => {
                                            "Child process terminated unexpectedly.".to_string()
                                        }
                                        Err(err) => format!("Failed awaiting child exit: {err}"),
                                    },
                                });
                            }
                        }
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        exited_flag.store(true, Ordering::Release);
        exited_notify.notify_waiters();

        if should_emit {
            if let Some(ref cb) = notifier {
                cb(build_snapshot_from_inner(&inner));
            }
        }
    });
}

async fn stop_active_handle(handle: &mut ActiveChildHandle) {
    handle.terminate_and_signal(STOP_REASON_USER);
    wait_for_exit_or_stderr(handle).await;
}

async fn wait_for_exit_or_stderr(handle: &mut ActiveChildHandle) {
    if !handle.exited_flag.load(Ordering::Acquire) {
        let _ = tokio::time::timeout(STOP_WAIT_TIMEOUT, handle.exited_notify.notified()).await;
    }

    if let Some(stderr_task) = handle.stderr_task.take() {
        let _ = tokio::time::timeout(Duration::from_millis(200), stderr_task).await;
    }
}

fn truncate_stderr_line(input: &str) -> String {
    let mut chars = input.chars();
    let head: String = chars.by_ref().take(MAX_STDERR_LINE_CHARS).collect();
    if chars.next().is_some() {
        format!(
            "{}…",
            head.chars()
                .take(MAX_STDERR_LINE_CHARS.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        head
    }
}

fn validate_server_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 64
        || id == "."
        || id == ".."
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(format!(
            "Invalid child server id '{id}': expected 1-64 characters matching [a-zA-Z0-9._-] (not '.' or '..')."
        ));
    }
    Ok(())
}

fn validate_env_key(key: &str) -> Result<(), String> {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return Err("Environment variable key cannot be empty.".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "Invalid environment variable key '{key}': must match [a-zA-Z_][a-zA-Z0-9_]*."
        ));
    }
    if key.contains('\0')
        || key.contains('=')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(format!(
            "Invalid environment variable key '{key}': must match [a-zA-Z_][a-zA-Z0-9_]*."
        ));
    }
    Ok(())
}

fn validate_spawn_spec(spec: SpawnChildServerSpec) -> Result<SpawnChildServerSpec, String> {
    let id = spec.id.trim().to_string();
    validate_server_id(&id)?;

    let name = spec.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 || name.contains(['\0', '\r', '\n']) {
        return Err(
            "Invalid child server name: must be 1-80 characters without control line breaks."
                .to_string(),
        );
    }

    let command = spec.command.trim().to_string();
    if command.is_empty() || command.contains(['\0', '\r', '\n']) {
        return Err(
            "Invalid child server command: must be non-empty without NUL or newline characters."
                .to_string(),
        );
    }

    for arg in &spec.args {
        if arg.contains('\0') {
            return Err("Child server arguments cannot contain NUL bytes.".to_string());
        }
    }

    let cwd = match spec.cwd {
        Some(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                if trimmed.contains(['\0', '\r', '\n']) {
                    return Err(
                        "Working directory cannot contain NUL or newline characters.".to_string(),
                    );
                }
                if !Path::new(&trimmed).is_dir() {
                    return Err(format!(
                        "Working directory '{trimmed}' does not exist or is not a directory."
                    ));
                }
                Some(trimmed)
            }
        }
        None => None,
    };

    let mut env = Vec::with_capacity(spec.env.len());
    for pair in spec.env {
        let key = pair.key.trim().to_string();
        validate_env_key(&key)?;
        if pair.value.contains('\0') {
            return Err(format!(
                "Environment variable '{key}' value cannot contain NUL bytes."
            ));
        }
        env.push(ChildEnvVar {
            key,
            value: pair.value,
        });
    }

    Ok(SpawnChildServerSpec {
        id,
        name,
        command,
        args: spec.args,
        cwd,
        env,
        perform_mcp_handshake: spec.perform_mcp_handshake,
    })
}

fn contains_cmd_shell_metachar(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '&' | '|' | ';' | '<' | '>' | '^' | '%' | '!' | '"' | '\r' | '\n' | '\0'
        )
    })
}

fn build_child_std_command(
    resolved_exe: &Path,
    args: &[String],
    cwd: Option<&str>,
    merged_path: &std::ffi::OsStr,
    env: &[ChildEnvVar],
) -> Result<std::process::Command, String> {
    let exe_str = resolved_exe.to_string_lossy();
    let lower = exe_str.to_ascii_lowercase();
    let is_batch_script = lower.ends_with(".cmd") || lower.ends_with(".bat");

    #[cfg(target_os = "windows")]
    let mut cmd = if is_batch_script {
        use std::os::windows::process::CommandExt;

        if contains_cmd_shell_metachar(&exe_str) {
            return Err(format!(
                "Refusing to execute batch shim '{exe_str}' containing unsafe shell metacharacters."
            ));
        }

        let mut quoted_tokens = Vec::with_capacity(args.len() + 1);
        quoted_tokens.push(format!("\"{exe_str}\""));

        for arg in args {
            if contains_cmd_shell_metachar(arg) {
                return Err(format!(
                    "Refusing argument '{arg}' for .cmd/.bat launcher: shell metacharacters (& | ; < > ^ % ! \" CR LF) are disallowed."
                ));
            }
            quoted_tokens.push(format!("\"{arg}\""));
        }

        let mut cmd = new_hidden_std_command("cmd.exe");
        cmd.arg("/d").arg("/s").arg("/c");
        // `cmd.exe /s /c` strips the outermost quote pair and preserves inner quotes verbatim.
        cmd.raw_arg(format!("\"{}\"", quoted_tokens.join(" ")));
        cmd
    } else {
        let mut cmd = new_hidden_std_command(resolved_exe);
        cmd.args(args);
        cmd
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let _ = (is_batch_script, contains_cmd_shell_metachar(&exe_str));
        let mut cmd = new_hidden_std_command(resolved_exe);
        cmd.args(args);
        cmd
    };

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.env("PATH", merged_path);
    for pair in env {
        cmd.env(&pair.key, &pair.value);
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::super::{KeelConfig, KeelRuntime, KeelToolRegistry};
    use super::*;

    struct RuntimeShutdownGuard(KeelRuntime);

    impl Drop for RuntimeShutdownGuard {
        fn drop(&mut self) {
            self.0.shutdown();
        }
    }

    fn resolve_powershell_for_test() -> String {
        let (merged_path, _) = build_merged_windows_path();
        if let Some(pwsh) = resolve_executable_in_path("pwsh", &merged_path) {
            let lower = pwsh.to_string_lossy().to_ascii_lowercase();
            if !lower.contains(r"\microsoft\windowsapps\") {
                return pwsh.to_string_lossy().into_owned();
            }
        }
        resolve_executable_in_path("powershell.exe", &merged_path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "powershell.exe".to_string())
    }

    #[tokio::test]
    async fn validates_spec_ids_env_keys_and_cmd_metacharacters() {
        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let _guard = RuntimeShutdownGuard(runtime.clone());

        let bad_id = runtime
            .start_child_server(SpawnChildServerSpec {
                id: "../bad".to_string(),
                name: "Bad".to_string(),
                command: "cmd.exe".to_string(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
                perform_mcp_handshake: false,
            })
            .await
            .expect_err("path traversal server id must be rejected");
        assert!(bad_id.contains("Invalid child server id"));

        let bad_env = runtime
            .start_child_server(SpawnChildServerSpec {
                id: "env-check".to_string(),
                name: "Env Check".to_string(),
                command: "cmd.exe".to_string(),
                args: Vec::new(),
                cwd: None,
                env: vec![ChildEnvVar {
                    key: "BAD=KEY".to_string(),
                    value: "secret".to_string(),
                }],
                perform_mcp_handshake: false,
            })
            .await
            .expect_err("invalid env key must be rejected");
        assert!(bad_env.contains("Invalid environment variable key"));

        // Verify `.cmd` metacharacter injection guard.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("keel_cmd_guard_{nonce}"));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let fake_cmd = temp_dir.join("probe.cmd");
        std::fs::write(&fake_cmd, "@echo off\r\n").unwrap();

        let cmd_err = runtime
            .start_child_server(SpawnChildServerSpec {
                id: "cmd-inject".to_string(),
                name: "Cmd Inject".to_string(),
                command: fake_cmd.to_string_lossy().into_owned(),
                args: vec!["ok & calc.exe".to_string()],
                cwd: None,
                env: Vec::new(),
                perform_mcp_handshake: false,
            })
            .await
            .expect_err("shell metacharacter in .cmd argument must be rejected");
        assert!(cmd_err.contains("shell metacharacters"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn live_stdio_mcp_handshake_captures_tools_and_stderr_then_stops_cleanly() {
        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let _guard = RuntimeShutdownGuard(runtime.clone());

        let ps_exe = resolve_powershell_for_test();
        let ps_script = r#"
[Console]::Error.WriteLine('keeldesk-stdio-probe: starting')
while ($null -ne ($line = [Console]::In.ReadLine())) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $req = $line | ConvertFrom-Json
    if ($null -eq $req.id) { continue }
    if ($req.method -eq 'initialize') {
        $resp = @{
            jsonrpc = '2.0'
            id = $req.id
            result = @{
                protocolVersion = '2025-03-26'
                capabilities = @{ tools = @{} }
                serverInfo = @{ name = 'probe-stdio-mcp'; version = '1.2.3' }
            }
        } | ConvertTo-Json -Compress -Depth 10
        [Console]::Out.WriteLine($resp)
        [Console]::Out.Flush()
    } elseif ($req.method -eq 'tools/list') {
        $resp = @{
            jsonrpc = '2.0'
            id = $req.id
            result = @{
                tools = @(
                    @{
                        name = 'echo_stdio_probe'
                        description = 'Echo probe tool from child stdio server'
                        inputSchema = @{
                            type = 'object'
                            properties = @{ text = @{ type = 'string' } }
                        }
                    }
                )
            }
        } | ConvertTo-Json -Compress -Depth 10
        [Console]::Out.WriteLine($resp)
        [Console]::Out.Flush()
    } else {
        $resp = @{
            jsonrpc = '2.0'
            id = $req.id
            result = @{}
        } | ConvertTo-Json -Compress -Depth 5
        [Console]::Out.WriteLine($resp)
        [Console]::Out.Flush()
    }
}
"#;

        let record = runtime
            .start_child_server(SpawnChildServerSpec {
                id: "probe-stdio".to_string(),
                name: "Probe Stdio MCP".to_string(),
                command: ps_exe,
                args: vec![
                    "-NoLogo".to_string(),
                    "-NoProfile".to_string(),
                    "-NonInteractive".to_string(),
                    "-Command".to_string(),
                    ps_script.to_string(),
                ],
                cwd: None,
                env: vec![ChildEnvVar {
                    key: "KEEL_SECRET_TOKEN".to_string(),
                    value: "super-secret-value-must-not-leak".to_string(),
                }],
                perform_mcp_handshake: true,
            })
            .await
            .expect("live stdio MCP handshake should succeed");

        assert_eq!(record.status, ChildServerStatus::Running);
        assert!(record.pid.is_some());
        assert!(record.job_object_attached);
        assert_eq!(record.server_info_name.as_deref(), Some("probe-stdio-mcp"));
        assert_eq!(record.server_info_version.as_deref(), Some("1.2.3"));
        assert_eq!(record.env_keys, vec!["KEEL_SECRET_TOKEN".to_string()]);
        assert!(record
            .discovered_tools
            .iter()
            .any(|t| t.name == "echo_stdio_probe"));

        // Wait briefly for stderr reader task if needed.
        let mut saw_stderr = false;
        for _ in 0..40 {
            let snap = runtime.process_host_snapshot();
            if let Some(entry) = snap.servers.iter().find(|s| s.id == "probe-stdio") {
                if entry
                    .stderr_tail
                    .iter()
                    .any(|line| line.contains("keeldesk-stdio-probe: starting"))
                {
                    saw_stderr = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(saw_stderr, "expected diagnostic line in stderr_tail");

        let stopped = runtime
            .stop_child_server("probe-stdio")
            .await
            .expect("stop_child_server should succeed");
        assert_eq!(stopped.status, ChildServerStatus::Stopped);

        let snap = runtime.process_host_snapshot();
        assert_eq!(snap.running_count, 0);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn win32_job_object_reclaims_parent_and_grandchild_process_tree_on_shutdown() {
        use windows::Win32::{
            Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT},
            System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
        };

        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let _guard = RuntimeShutdownGuard(runtime.clone());

        let ps_exe = resolve_powershell_for_test();
        let ps_tree_script = r#"
$psi = [System.Diagnostics.ProcessStartInfo]::new('ping.exe', '127.0.0.1 -n 60')
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$p = [System.Diagnostics.Process]::Start($psi)
[Console]::Error.WriteLine("GRANDCHILD_PID=$($p.Id)")
$p.WaitForExit()
"#;

        let record = runtime
            .start_child_server(SpawnChildServerSpec {
                id: "job-tree-probe".to_string(),
                name: "Job Tree Probe".to_string(),
                command: ps_exe,
                args: vec![
                    "-NoLogo".to_string(),
                    "-NoProfile".to_string(),
                    "-NonInteractive".to_string(),
                    "-Command".to_string(),
                    ps_tree_script.to_string(),
                ],
                cwd: None,
                env: Vec::new(),
                perform_mcp_handshake: false,
            })
            .await
            .expect("plain stdio process tree probe should start");

        assert_eq!(record.status, ChildServerStatus::Running);
        assert!(record.job_object_attached);

        let mut grandchild_pid: Option<u32> = None;
        for _ in 0..80 {
            let snap = runtime.process_host_snapshot();
            if let Some(entry) = snap.servers.iter().find(|s| s.id == "job-tree-probe") {
                for line in &entry.stderr_tail {
                    if let Some(raw_pid) = line.strip_prefix("GRANDCHILD_PID=") {
                        if let Ok(parsed) = raw_pid.trim().parse::<u32>() {
                            grandchild_pid = Some(parsed);
                            break;
                        }
                    }
                }
            }
            if grandchild_pid.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let gc_pid = grandchild_pid.expect("grandchild PID must appear in stderr_tail");
        // SAFETY: Open grandchild with `PROCESS_SYNCHRONIZE` to check liveness before and after shutdown.
        let gc_handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, gc_pid) }
            .expect("grandchild process must be openable while alive");

        // Confirm grandchild is currently alive (`WAIT_TIMEOUT`).
        let alive_wait = unsafe { WaitForSingleObject(gc_handle, 0) };
        assert_eq!(
            alive_wait, WAIT_TIMEOUT,
            "grandchild process must still be alive before runtime shutdown"
        );

        // Shut down runtime synchronously, which must terminate the entire Win32 Job Object tree.
        runtime.shutdown();

        let exit_wait = unsafe { WaitForSingleObject(gc_handle, 3000) };
        unsafe {
            let _ = CloseHandle(gc_handle);
        }
        assert_eq!(
            exit_wait, WAIT_OBJECT_0,
            "Win32 Job Object must terminate grandchild process on runtime shutdown"
        );
    }

    #[tokio::test]
    async fn live_stdio_child_handshake_and_tool_call_record_protocol_traces() {
        use super::super::devtools::{McpTraceDirection, McpTraceStatus, McpTraceTransport};

        let runtime =
            KeelRuntime::start_headless(KeelConfig::ephemeral_for_test(), KeelToolRegistry::new());
        let _guard = RuntimeShutdownGuard(runtime.clone());

        let ps_exe = resolve_powershell_for_test();
        let ps_script = r#"
while ($null -ne ($line = [Console]::In.ReadLine())) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $req = $line | ConvertFrom-Json
    if ($null -eq $req.id) { continue }
    if ($req.method -eq 'initialize') {
        $resp = @{
            jsonrpc = '2.0'
            id = $req.id
            result = @{
                protocolVersion = '2025-03-26'
                capabilities = @{ tools = @{} }
                serverInfo = @{ name = 'devtools-child-mcp'; version = '0.9.0' }
            }
        } | ConvertTo-Json -Compress -Depth 10
        [Console]::Out.WriteLine($resp)
        [Console]::Out.Flush()
    } elseif ($req.method -eq 'tools/list') {
        $resp = @{
            jsonrpc = '2.0'
            id = $req.id
            result = @{
                tools = @(
                    @{
                        name = 'echo_child'
                        description = 'Echoes text from child stdio server'
                        inputSchema = @{
                            type = 'object'
                            properties = @{ text = @{ type = 'string' } }
                        }
                    }
                )
            }
        } | ConvertTo-Json -Compress -Depth 10
        [Console]::Out.WriteLine($resp)
        [Console]::Out.Flush()
    } elseif ($req.method -eq 'tools/call') {
        $argText = ''
        if ($null -ne $req.params.arguments -and $null -ne $req.params.arguments.text) {
            $argText = [string]$req.params.arguments.text
        }
        $isErr = ($argText -eq 'boom')
        $resp = @{
            jsonrpc = '2.0'
            id = $req.id
            result = @{
                isError = $isErr
                content = @(
                    @{
                        type = 'text'
                        text = "child-echo:$argText"
                    }
                )
            }
        } | ConvertTo-Json -Compress -Depth 10
        [Console]::Out.WriteLine($resp)
        [Console]::Out.Flush()
    }
}
"#;

        let record = runtime
            .start_child_server(SpawnChildServerSpec {
                id: "trace-child".to_string(),
                name: "Trace Child MCP".to_string(),
                command: ps_exe,
                args: vec![
                    "-NoLogo".to_string(),
                    "-NoProfile".to_string(),
                    "-NonInteractive".to_string(),
                    "-Command".to_string(),
                    ps_script.to_string(),
                ],
                cwd: None,
                env: Vec::new(),
                perform_mcp_handshake: true,
            })
            .await
            .expect("child MCP server should start and complete handshake");
        assert_eq!(record.status, ChildServerStatus::Running);

        // Verify invalid non-object JSON arguments fail cleanly before sending.
        let bad_args_err = runtime
            .call_child_server_tool("trace-child", "echo_child", Some("[1, 2]".to_string()))
            .await
            .expect_err("non-object JSON arguments must be rejected");
        assert!(bad_args_err.contains("JSON object"));

        // Successful tool invocation
        let ok_res = runtime
            .call_child_server_tool(
                "trace-child",
                "echo_child",
                Some(r#"{"text":"win11-stdio"}"#.to_string()),
            )
            .await
            .expect("child tool call should succeed");
        assert_eq!(ok_res.server_id, "trace-child");
        assert_eq!(ok_res.tool_name, "echo_child");
        assert!(!ok_res.is_error);
        assert!(ok_res.trace_seq.is_some());
        assert!(ok_res.response_json.contains("child-echo:win11-stdio"));

        // Tool-level error response
        let err_res = runtime
            .call_child_server_tool(
                "trace-child",
                "echo_child",
                Some(r#"{"text":"boom"}"#.to_string()),
            )
            .await
            .expect("tool error still returns ChildToolCallResult");
        assert!(err_res.is_error);
        assert!(err_res.trace_seq.is_some());

        let traces = runtime.protocol_traces();
        assert!(traces
            .iter()
            .any(|t| t.transport == McpTraceTransport::StdioChild
                && t.direction == McpTraceDirection::Outbound
                && t.target == "child:trace-child"
                && t.method == "initialize"
                && t.status == McpTraceStatus::Succeeded));
        assert!(traces
            .iter()
            .any(|t| t.transport == McpTraceTransport::StdioChild
                && t.target == "child:trace-child"
                && t.method == "tools/list"
                && t.status == McpTraceStatus::Succeeded));
        assert!(traces
            .iter()
            .any(|t| t.transport == McpTraceTransport::StdioChild
                && t.target == "child:trace-child"
                && t.method == "tools/call"
                && t.tool_name.as_deref() == Some("echo_child")
                && t.status == McpTraceStatus::Succeeded
                && t.seq == ok_res.trace_seq.unwrap()));
        assert!(traces
            .iter()
            .any(|t| t.transport == McpTraceTransport::StdioChild
                && t.target == "child:trace-child"
                && t.method == "tools/call"
                && t.status == McpTraceStatus::ToolError
                && t.seq == err_res.trace_seq.unwrap()));

        runtime
            .stop_child_server("trace-child")
            .await
            .expect("stop should succeed");

        let stopped_err = runtime
            .call_child_server_tool("trace-child", "echo_child", None)
            .await
            .expect_err("calling a stopped child server must error");
        assert!(stopped_err.contains("not currently running"));
    }
}
