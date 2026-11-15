import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  events,
  type ChildMcpToolSummary,
  type ChildServerRecord,
  type ProcessHostSnapshot,
  type SpawnChildServerSpec,
} from "@/bindings";
import { isTauriRuntime } from "./useKeelRuntime";

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
    id: sanitizedSuffix ? `win11-stdio-echo-${sanitizedSuffix}` : "win11-stdio-echo",
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

export function useProcessHost() {
  const [hostSnapshot, setHostSnapshot] = useState<ProcessHostSnapshot>(() =>
    isTauriRuntime() ? EMPTY_HOST_SNAPSHOT : PREVIEW_HOST_SNAPSHOT,
  );
  const [isLoading, setIsLoading] = useState(true);
  const [isStarting, setIsStarting] = useState(false);
  const [busyServerIds, setBusyServerIds] = useState<Record<string, boolean>>(
    {},
  );
  const [error, setError] = useState<string | null>(null);

  const submittedSpecsRef = useRef<Map<string, SpawnChildServerSpec>>(new Map());
  const hostSnapshotRef = useRef<ProcessHostSnapshot>(hostSnapshot);
  const previewPidSeqRef = useRef(1);

  useEffect(() => {
    hostSnapshotRef.current = hostSnapshot;
  }, [hostSnapshot]);

  const setServerBusy = useCallback((serverId: string, busy: boolean) => {
    setBusyServerIds((prev) => {
      if (busy) {
        return { ...prev, [serverId]: true };
      }
      if (!prev[serverId]) {
        return prev;
      }
      const next = { ...prev };
      delete next[serverId];
      return next;
    });
  }, []);

  const refreshSnapshot = useCallback(async (): Promise<ProcessHostSnapshot | null> => {
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

  const startChildServer = useCallback(
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
        setServerBusy(normalizedSpec.id, true);
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
          return record;
        }

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
          setServerBusy(normalizedSpec.id, false);
        }
      }
    },
    [setServerBusy],
  );

  const stopChildServer = useCallback(
    async (serverId: string): Promise<ChildServerRecord | null> => {
      setError(null);
      setServerBusy(serverId, true);

      try {
        if (!isTauriRuntime()) {
          let stoppedRecord: ChildServerRecord | null = null;
          setHostSnapshot((prev) => {
            const nextServers = prev.servers.map((item) => {
              if (item.id !== serverId) {
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

        const res = await commands.stopKeelChildServer(serverId);
        if (res.status === "ok") {
          return res.data;
        }
        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setServerBusy(serverId, false);
      }
    },
    [setServerBusy],
  );

  const removeChildServer = useCallback(
    async (serverId: string): Promise<boolean> => {
      setError(null);
      setServerBusy(serverId, true);

      try {
        if (!isTauriRuntime()) {
          submittedSpecsRef.current.delete(serverId);
          setHostSnapshot((prev) => {
            const nextServers = prev.servers.filter(
              (item) => item.id !== serverId,
            );
            return {
              ...prev,
              running_count: recomputeRunningCount(nextServers),
              servers: nextServers,
            };
          });
          return true;
        }

        const res = await commands.removeKeelChildServer(serverId);
        if (res.status === "ok") {
          submittedSpecsRef.current.delete(serverId);
          setHostSnapshot(res.data);
          return true;
        }
        setError(res.error);
        return false;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setServerBusy(serverId, false);
      }
    },
    [setServerBusy],
  );

  const restartChildServer = useCallback(
    async (serverId: string): Promise<ChildServerRecord | null> => {
      const cached = submittedSpecsRef.current.get(serverId);
      if (cached) {
        return startChildServer(cached);
      }

      const existing = hostSnapshotRef.current.servers.find(
        (item) => item.id === serverId,
      );
      if (!existing) {
        setError(`Child server '${serverId}' was not found.`);
        return null;
      }

      if (existing.env_keys.length > 0) {
        setError(
          `子进程 '${existing.name}' 包含敏感环境变量 (${existing.env_keys.join(", ")})，安全起见未在内存中持久化密钥，请通过“添加子进程”重新提交启动。`,
        );
        return null;
      }

      return startChildServer({
        id: existing.id,
        name: existing.name,
        command: existing.command,
        args: existing.args,
        cwd: existing.cwd,
        env: [],
        perform_mcp_handshake: existing.perform_mcp_handshake,
      });
    },
    [startChildServer],
  );

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    hostSnapshot,
    isLoading,
    isStarting,
    busyServerIds,
    error,
    refreshSnapshot,
    startChildServer,
    stopChildServer,
    removeChildServer,
    restartChildServer,
    clearError,
  };
}
