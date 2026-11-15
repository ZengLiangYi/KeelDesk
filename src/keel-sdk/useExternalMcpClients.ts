import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  commands,
  events,
  type ChildMcpToolSummary,
  type ChildServerRecord,
  type ChildToolCallResult,
  type ProcessHostSnapshot,
  type SpawnChildServerSpec,
} from "@/bindings";
import { pushPreviewProtocolTrace } from "./previewTraceBus";
import { isTauriRuntime } from "./useMcpServerState";

export interface StderrStreamEntry {
  clientId: string;
  clientName: string;
  line: string;
  lineIndex: number;
}

export interface UseExternalMcpClientsResult {
  hostSnapshot: ProcessHostSnapshot;
  clients: ChildServerRecord[];
  runningCount: number;
  mergedPathEntries: number;
  stderrByClientId: Record<string, string[]>;
  stderrStream: StderrStreamEntry[];
  isLoading: boolean;
  isStarting: boolean;
  busyClientIds: Record<string, boolean>;
  callingClientIds: Record<string, boolean>;
  lastToolCallResult: ChildToolCallResult | null;
  error: string | null;
  refreshSnapshot: () => Promise<ProcessHostSnapshot | null>;
  startClient: (spec: SpawnChildServerSpec) => Promise<ChildServerRecord | null>;
  stopClient: (clientId: string) => Promise<ChildServerRecord | null>;
  restartClient: (clientId: string) => Promise<ChildServerRecord | null>;
  removeClient: (clientId: string) => Promise<boolean>;
  callClientTool: (
    clientId: string,
    toolName: string,
    argumentsJson?: string | null,
  ) => Promise<ChildToolCallResult | null>;
  clearError: () => void;
}

const EMPTY_HOST_SNAPSHOT: ProcessHostSnapshot = {
  merged_path_entries: 0,
  running_count: 0,
  servers: [],
};

const PREVIEW_HOST_SNAPSHOT: ProcessHostSnapshot = {
  merged_path_entries: 34,
  running_count: 0,
  servers: [],
};

const BUILTIN_ECHO_PROBE_SCRIPT = [
  "[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)",
  "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)",
  "[Console]::Error.WriteLine('[keeldesk-echo] ready on stdio')",
  "while ($null -ne ($line = [Console]::In.ReadLine())) {",
  "    if ([string]::IsNullOrWhiteSpace($line)) { continue }",
  "    $req = $line | ConvertFrom-Json",
  "    if ($null -eq $req.id) { continue }",
  "    if ($req.method -eq 'initialize') {",
  "        $resp = @{",
  "            jsonrpc = '2.0'",
  "            id = $req.id",
  "            result = @{",
  "                protocolVersion = '2025-03-26'",
  "                capabilities = @{ tools = @{} }",
  "                serverInfo = @{ name = 'win11-stdio-echo'; version = '0.1.0' }",
  "            }",
  "        } | ConvertTo-Json -Compress -Depth 10",
  "        [Console]::Out.WriteLine($resp)",
  "        [Console]::Out.Flush()",
  "    } elseif ($req.method -eq 'tools/list') {",
  "        $resp = @{",
  "            jsonrpc = '2.0'",
  "            id = $req.id",
  "            result = @{",
  "                tools = @(",
  "                    @{",
  "                        name = 'echo_stdio_probe'",
  "                        description = 'Echo diagnostic probe tool served over supervised Win11 stdio MCP'",
  "                        inputSchema = @{",
  "                            type = 'object'",
  "                            properties = @{ text = @{ type = 'string' } }",
  "                        }",
  "                    }",
  "                )",
  "            }",
  "        } | ConvertTo-Json -Compress -Depth 10",
  "        [Console]::Out.WriteLine($resp)",
  "        [Console]::Out.Flush()",
  "    } elseif ($req.method -eq 'tools/call') {",
  "        $text = if ($null -ne $req.params.arguments -and $null -ne $req.params.arguments.text) { [string]$req.params.arguments.text } else { 'pong' }",
  "        [Console]::Error.WriteLine(\"[keeldesk-echo] tools/call $($req.params.name): $text\")",
  "        $resp = @{",
  "            jsonrpc = '2.0'",
  "            id = $req.id",
  "            result = @{",
  "                content = @( @{ type = 'text'; text = \"[echo_stdio_probe] $text\" } )",
  "                isError = $false",
  "            }",
  "        } | ConvertTo-Json -Compress -Depth 10",
  "        [Console]::Out.WriteLine($resp)",
  "        [Console]::Out.Flush()",
  "    } else {",
  "        $resp = @{",
  "            jsonrpc = '2.0'",
  "            id = $req.id",
  "            result = @{}",
  "        } | ConvertTo-Json -Compress -Depth 5",
  "        [Console]::Out.WriteLine($resp)",
  "        [Console]::Out.Flush()",
  "    }",
  "}",
].join("\n");

export function buildBuiltinEchoProbeSpec(
  idSuffix?: string,
): SpawnChildServerSpec {
  const sanitizedSuffix = idSuffix
    ?.trim()
    .replace(/[^a-zA-Z0-9._-]/g, "");

  return {
    id: sanitizedSuffix
      ? `win11-stdio-echo-${sanitizedSuffix}`
      : "win11-stdio-echo",
    name: sanitizedSuffix
      ? `Win11 Stdio Echo Probe (${sanitizedSuffix})`
      : "Win11 Stdio Echo Probe",
    command: "powershell.exe",
    args: [
      "-NoLogo",
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      BUILTIN_ECHO_PROBE_SCRIPT,
    ],
    cwd: null,
    env: [],
    perform_mcp_handshake: true,
  };
}

function recomputeRunningCount(servers: ChildServerRecord[]): number {
  return servers.filter((server) => server.status === "running").length;
}

function resolvePreviewExecutable(command: string): string {
  const normalized = command.trim();
  const lower = normalized.toLowerCase();
  if (lower === "powershell" || lower === "powershell.exe") {
    return "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
  }
  if (lower === "pwsh" || lower === "pwsh.exe") {
    return "C:\\Program Files\\PowerShell\\7\\pwsh.exe";
  }
  if (lower === "npx" || lower === "npx.cmd") {
    return "C:\\Program Files\\nodejs\\npx.cmd";
  }
  if (lower === "uvx" || lower === "uvx.exe") {
    return "C:\\Users\\Developer\\.local\\bin\\uvx.exe";
  }
  if (lower === "node" || lower === "node.exe") {
    return "C:\\Program Files\\nodejs\\node.exe";
  }
  return normalized;
}

function buildPreviewDiscoveredTools(
  spec: SpawnChildServerSpec,
): ChildMcpToolSummary[] {
  if (spec.perform_mcp_handshake === false) {
    return [];
  }
  if (spec.id.startsWith("win11-stdio-echo")) {
    return [
      {
        name: "echo_stdio_probe",
        description:
          "Echo diagnostic probe tool served over supervised Win11 stdio MCP",
      },
    ];
  }
  if (spec.id.includes("filesystem")) {
    return [
      {
        name: "read_file",
        description: "Read file contents from an allowed workspace directory.",
      },
      {
        name: "list_directory",
        description: "List files and directories under an allowed root.",
      },
    ];
  }
  if (spec.id.includes("fetch")) {
    return [
      {
        name: "fetch",
        description: "Fetch a URL and extract readable content.",
      },
    ];
  }
  return [
    {
      name: `${spec.id.replace(/[^a-zA-Z0-9_]/g, "_")}_ping`,
      description: `Preview MCP tool exported by ${spec.name}.`,
    },
  ];
}

export function useExternalMcpClients(): UseExternalMcpClientsResult {
  const [hostSnapshot, setHostSnapshot] = useState<ProcessHostSnapshot>(() =>
    isTauriRuntime() ? EMPTY_HOST_SNAPSHOT : PREVIEW_HOST_SNAPSHOT,
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isStarting, setIsStarting] = useState(false);
  const [busyClientIds, setBusyClientIds] = useState<Record<string, boolean>>(
    {},
  );
  const [callingClientIds, setCallingClientIds] = useState<
    Record<string, boolean>
  >({});
  const [lastToolCallResult, setLastToolCallResult] =
    useState<ChildToolCallResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const submittedSpecsRef = useRef<Map<string, SpawnChildServerSpec>>(new Map());
  const hostSnapshotRef = useRef<ProcessHostSnapshot>(hostSnapshot);
  const previewPidSeqRef = useRef(1);
  const previewRpcSeqRef = useRef(1);

  useEffect(() => {
    hostSnapshotRef.current = hostSnapshot;
  }, [hostSnapshot]);

  const setClientBusy = useCallback((clientId: string, busy: boolean) => {
    setBusyClientIds((prev) => {
      if (busy) {
        return { ...prev, [clientId]: true };
      }
      if (!prev[clientId]) {
        return prev;
      }
      const next = { ...prev };
      delete next[clientId];
      return next;
    });
  }, []);

  const setClientCalling = useCallback((clientId: string, calling: boolean) => {
    setCallingClientIds((prev) => {
      if (calling) {
        return { ...prev, [clientId]: true };
      }
      if (!prev[clientId]) {
        return prev;
      }
      const next = { ...prev };
      delete next[clientId];
      return next;
    });
  }, []);

  const refreshSnapshot =
    useCallback(async (): Promise<ProcessHostSnapshot | null> => {
      if (!isTauriRuntime()) {
        setIsLoading(false);
        return hostSnapshotRef.current;
      }

      setIsLoading(true);
      try {
        const res = await commands.getKeelProcessHostSnapshot();
        if (res.status === "ok") {
          setHostSnapshot(res.data);
          return res.data;
        }
        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setIsLoading(false);
      }
    }, []);

  useEffect(() => {
    void refreshSnapshot();

    if (!isTauriRuntime()) {
      return;
    }

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void events.processHostUpdatedEvent
      .listen((event) => {
        if (!cancelled) {
          setHostSnapshot(event.payload);
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

  const startClient = useCallback(
    async (spec: SpawnChildServerSpec): Promise<ChildServerRecord | null> => {
      const normalizedSpec: SpawnChildServerSpec = {
        ...spec,
        id: spec.id.trim(),
        name: spec.name.trim(),
        command: spec.command.trim(),
        args: spec.args ?? [],
        cwd: spec.cwd?.trim() || null,
        env: spec.env ?? [],
        perform_mcp_handshake: spec.perform_mcp_handshake ?? true,
      };

      setIsStarting(true);
      setError(null);
      if (normalizedSpec.id) {
        setClientBusy(normalizedSpec.id, true);
      }

      try {
        if (!isTauriRuntime()) {
          if (
            !normalizedSpec.id ||
            !/^[a-zA-Z0-9._-]{1,64}$/.test(normalizedSpec.id) ||
            normalizedSpec.id === "." ||
            normalizedSpec.id === ".."
          ) {
            setError(
              `Invalid child server id '${normalizedSpec.id}': expected 1-64 characters matching [a-zA-Z0-9._-].`,
            );
            return null;
          }
          if (!normalizedSpec.name || !normalizedSpec.command) {
            setError("Child server name and executable command are required.");
            return null;
          }

          // Strict secret-redaction invariant: never cache specs with env variables
          if ((normalizedSpec.env?.length ?? 0) === 0) {
            submittedSpecsRef.current.set(normalizedSpec.id, normalizedSpec);
          } else {
            submittedSpecsRef.current.delete(normalizedSpec.id);
          }

          const pid = 14200 + previewPidSeqRef.current++ * 37;
          const performHandshake =
            normalizedSpec.perform_mcp_handshake !== false;
          const discoveredTools = buildPreviewDiscoveredTools(normalizedSpec);
          const record: ChildServerRecord = {
            id: normalizedSpec.id,
            name: normalizedSpec.name,
            command: normalizedSpec.command,
            resolved_executable: resolvePreviewExecutable(normalizedSpec.command),
            args: normalizedSpec.args ?? [],
            cwd: normalizedSpec.cwd ?? null,
            env_keys: (normalizedSpec.env ?? []).map((pair) => pair.key),
            perform_mcp_handshake: performHandshake,
            status: "running",
            pid,
            job_object_attached: true,
            started_at_iso: new Date().toISOString(),
            stopped_at_iso: null,
            last_exit_code: null,
            last_error: null,
            server_info_name: performHandshake ? normalizedSpec.id : null,
            server_info_version: performHandshake ? "0.1.0" : null,
            handshake_ms: performHandshake ? 28 : null,
            discovered_tools: discoveredTools,
            stderr_tail: [
              normalizedSpec.id.startsWith("win11-stdio-echo")
                ? "[keeldesk-echo] ready on stdio"
                : `[${normalizedSpec.id}] child process attached to Win32 Job Object (PID ${pid})`,
            ],
          };

          setHostSnapshot((prev) => {
            const exists = prev.servers.some((item) => item.id === record.id);
            const nextServers = exists
              ? prev.servers.map((item) =>
                  item.id === record.id ? record : item,
                )
              : [...prev.servers, record];
            return {
              ...prev,
              running_count: recomputeRunningCount(nextServers),
              servers: nextServers,
            };
          });

          if (performHandshake) {
            const initId = String(previewRpcSeqRef.current++);
            const listId = String(previewRpcSeqRef.current++);
            pushPreviewProtocolTrace({
              transport: "stdio_child",
              direction: "outbound",
              target: record.id,
              session_id: null,
              caller_hint: `keeldesk-host -> ${record.name}`,
              jsonrpc_id: initId,
              method: "initialize",
              tool_name: null,
              safety_tier: null,
              approval_outcome: "not_required",
              status: "succeeded",
              status_code: null,
              error_message: null,
              latency: { total_ms: 19, approval_wait_ms: 0, execution_ms: 19 },
              request_value: {
                jsonrpc: "2.0",
                id: Number(initId),
                method: "initialize",
                params: {
                  protocolVersion: "2025-03-26",
                  capabilities: {},
                  clientInfo: { name: "keeldesk-host", version: "0.1.0" },
                },
              },
              response_value: {
                jsonrpc: "2.0",
                id: Number(initId),
                result: {
                  protocolVersion: "2025-03-26",
                  capabilities: { tools: {} },
                  serverInfo: {
                    name: record.server_info_name ?? record.id,
                    version: record.server_info_version ?? "0.1.0",
                  },
                },
              },
              linked_call_id: null,
            });
            pushPreviewProtocolTrace({
              transport: "stdio_child",
              direction: "outbound",
              target: record.id,
              session_id: null,
              caller_hint: `keeldesk-host -> ${record.name}`,
              jsonrpc_id: listId,
              method: "tools/list",
              tool_name: null,
              safety_tier: null,
              approval_outcome: "not_required",
              status: "succeeded",
              status_code: null,
              error_message: null,
              latency: { total_ms: 9, approval_wait_ms: 0, execution_ms: 9 },
              request_value: {
                jsonrpc: "2.0",
                id: Number(listId),
                method: "tools/list",
                params: {},
              },
              response_value: {
                jsonrpc: "2.0",
                id: Number(listId),
                result: { tools: discoveredTools },
              },
              linked_call_id: null,
            });
          }

          return record;
        }

        // Strict secret-redaction invariant: never cache specs with env variables
        if ((normalizedSpec.env?.length ?? 0) === 0) {
          submittedSpecsRef.current.set(normalizedSpec.id, normalizedSpec);
        } else {
          submittedSpecsRef.current.delete(normalizedSpec.id);
        }
        const res = await commands.startKeelChildServer(normalizedSpec);
        if (res.status === "ok") {
          return res.data;
        }
        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setIsStarting(false);
        if (normalizedSpec.id) {
          setClientBusy(normalizedSpec.id, false);
        }
      }
    },
    [setClientBusy],
  );

  const stopClient = useCallback(
    async (clientId: string): Promise<ChildServerRecord | null> => {
      setError(null);
      setClientBusy(clientId, true);

      try {
        if (!isTauriRuntime()) {
          let stoppedRecord: ChildServerRecord | null = null;
          setHostSnapshot((prev) => {
            const nextServers = prev.servers.map((item) => {
              if (item.id !== clientId) {
                return item;
              }
              stoppedRecord = {
                ...item,
                status: "stopped",
                stopped_at_iso: new Date().toISOString(),
                last_exit_code: 0,
                stderr_tail: [
                  ...item.stderr_tail,
                  `[${item.id}] terminated via Win32 Job Object`,
                ],
              };
              return stoppedRecord;
            });
            return {
              ...prev,
              running_count: recomputeRunningCount(nextServers),
              servers: nextServers,
            };
          });
          return stoppedRecord;
        }

        const res = await commands.stopKeelChildServer(clientId);
        if (res.status === "ok") {
          return res.data;
        }
        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setClientBusy(clientId, false);
      }
    },
    [setClientBusy],
  );

  const removeClient = useCallback(
    async (clientId: string): Promise<boolean> => {
      setError(null);
      setClientBusy(clientId, true);

      try {
        if (!isTauriRuntime()) {
          submittedSpecsRef.current.delete(clientId);
          setHostSnapshot((prev) => {
            const nextServers = prev.servers.filter(
              (item) => item.id !== clientId,
            );
            return {
              ...prev,
              running_count: recomputeRunningCount(nextServers),
              servers: nextServers,
            };
          });
          return true;
        }

        const res = await commands.removeKeelChildServer(clientId);
        if (res.status === "ok") {
          submittedSpecsRef.current.delete(clientId);
          setHostSnapshot(res.data);
          return true;
        }
        setError(res.error);
        return false;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setClientBusy(clientId, false);
      }
    },
    [setClientBusy],
  );

  const restartClient = useCallback(
    async (clientId: string): Promise<ChildServerRecord | null> => {
      const cached = submittedSpecsRef.current.get(clientId);
      if (cached) {
        return startClient(cached);
      }

      const existing = hostSnapshotRef.current.servers.find(
        (item) => item.id === clientId,
      );
      if (!existing) {
        setError(`Child server '${clientId}' was not found.`);
        return null;
      }

      if (existing.env_keys.length > 0) {
        setError(
          `子进程 '${existing.name}' 包含敏感环境变量 (${existing.env_keys.join(", ")})，安全起见未在内存中持久化密钥，请通过“添加子进程”重新提交启动。`,
        );
        return null;
      }

      return startClient({
        id: existing.id,
        name: existing.name,
        command: existing.command,
        args: existing.args,
        cwd: existing.cwd,
        env: [],
        perform_mcp_handshake: existing.perform_mcp_handshake,
      });
    },
    [startClient],
  );

  const callClientTool = useCallback(
    async (
      clientId: string,
      toolName: string,
      argumentsJson?: string | null,
    ): Promise<ChildToolCallResult | null> => {
      setError(null);
      setClientCalling(clientId, true);

      try {
        if (!isTauriRuntime()) {
          const client = hostSnapshotRef.current.servers.find(
            (item) => item.id === clientId,
          );
          if (!client) {
            setError(`Child server '${clientId}' was not found.`);
            return null;
          }
          if (client.status !== "running") {
            setError(
              `Child server '${client.name}' is currently '${client.status}' and cannot accept tools/call.`,
            );
            return null;
          }

          let parsedArgs: Record<string, unknown> = {};
          if (argumentsJson && argumentsJson.trim().length > 0) {
            try {
              const rawParsed: unknown = JSON.parse(argumentsJson);
              if (
                rawParsed !== null &&
                typeof rawParsed === "object" &&
                !Array.isArray(rawParsed)
              ) {
                parsedArgs = rawParsed as Record<string, unknown>;
              } else {
                setError("Tool arguments JSON must be an object.");
                return null;
              }
            } catch (parseErr) {
              setError(
                `Invalid tool arguments JSON: ${parseErr instanceof Error ? parseErr.message : String(parseErr)}`,
              );
              return null;
            }
          }

          const rpcId = previewRpcSeqRef.current++;
          let responseText: string;
          if (toolName === "echo_stdio_probe") {
            const text =
              typeof parsedArgs.text === "string" && parsedArgs.text.length > 0
                ? parsedArgs.text
                : "pong";
            responseText = `[echo_stdio_probe] ${text}`;
            const stderrLine = `[keeldesk-echo] tools/call ${toolName}: ${text}`;
            setHostSnapshot((prev) => ({
              ...prev,
              servers: prev.servers.map((item) =>
                item.id === clientId
                  ? {
                      ...item,
                      stderr_tail: [...item.stderr_tail, stderrLine].slice(
                        -64,
                      ),
                    }
                  : item,
              ),
            }));
          } else {
            responseText = `[${clientId}] executed ${toolName} with ${JSON.stringify(parsedArgs)}`;
          }

          const callResultPayload = {
            content: [{ type: "text", text: responseText }],
            isError: false,
          };
          const responseJson = JSON.stringify(callResultPayload, null, 2);
          const durationMs = 12;

          const traceEntry = pushPreviewProtocolTrace({
            transport: "stdio_child",
            direction: "outbound",
            target: clientId,
            session_id: null,
            caller_hint: `keeldesk-host -> ${client.name}`,
            jsonrpc_id: String(rpcId),
            method: "tools/call",
            tool_name: toolName,
            safety_tier: null,
            approval_outcome: "not_required",
            status: "succeeded",
            status_code: null,
            error_message: null,
            latency: {
              total_ms: durationMs,
              approval_wait_ms: 0,
              execution_ms: durationMs,
            },
            request_value: {
              jsonrpc: "2.0",
              id: rpcId,
              method: "tools/call",
              params: {
                name: toolName,
                arguments: parsedArgs,
              },
            },
            response_value: {
              jsonrpc: "2.0",
              id: rpcId,
              result: callResultPayload,
            },
            linked_call_id: null,
          });

          const result: ChildToolCallResult = {
            server_id: clientId,
            tool_name: toolName,
            duration_ms: durationMs,
            is_error: false,
            response_json: responseJson,
            trace_seq: traceEntry.seq,
          };
          setLastToolCallResult(result);
          return result;
        }

        const res = await commands.callKeelChildServerTool(
          clientId,
          toolName,
          argumentsJson ?? null,
        );
        if (res.status === "ok") {
          setLastToolCallResult(res.data);
          return res.data;
        }
        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setClientCalling(clientId, false);
      }
    },
    [setClientCalling],
  );

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const stderrByClientId = useMemo<Record<string, string[]>>(() => {
    const map: Record<string, string[]> = {};
    for (const server of hostSnapshot.servers) {
      map[server.id] = server.stderr_tail;
    }
    return map;
  }, [hostSnapshot.servers]);

  const stderrStream = useMemo<StderrStreamEntry[]>(() => {
    const rows: StderrStreamEntry[] = [];
    for (const server of hostSnapshot.servers) {
      server.stderr_tail.forEach((line, lineIndex) => {
        rows.push({
          clientId: server.id,
          clientName: server.name,
          line,
          lineIndex,
        });
      });
    }
    return rows;
  }, [hostSnapshot.servers]);

  return {
    hostSnapshot,
    clients: hostSnapshot.servers,
    runningCount: hostSnapshot.running_count,
    mergedPathEntries: hostSnapshot.merged_path_entries,
    stderrByClientId,
    stderrStream,
    isLoading,
    isStarting,
    busyClientIds,
    callingClientIds,
    lastToolCallResult,
    error,
    refreshSnapshot,
    startClient,
    stopClient,
    restartClient,
    removeClient,
    callClientTool,
    clearError,
  };
}
