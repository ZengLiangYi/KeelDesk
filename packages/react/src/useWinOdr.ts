import { useCallback, useEffect, useState } from "react";
import {
  commands,
  type OdrActionOutcome,
  type OdrRegisterMode,
  type OdrStatusReport,
} from "./bindings";
import { isTauriRuntime } from "./useKeelRuntime";

const DEFAULT_ENDPOINT_URL = "http://127.0.0.1:19527/mcp";
const PREVIEW_MANIFEST_PATH =
  "C:\\Users\\Developer\\AppData\\Local\\KeelDesk\\odr\\keeldesk.mcpb.manifest.json";

function buildPreviewManifestJson(endpointUrl: string): string {
  return JSON.stringify(
    {
      manifest_version: "0.3",
      name: "keeldesk",
      version: "0.1.0",
      display_name: "KeelDesk",
      description: "KeelDesk Windows 11 local desktop MCP runtime spine",
      author: {
        name: "Leon Zeng",
      },
      server: {
        type: "binary",
        entry_point: "keeldesk.exe",
        mcp_config: {
          command: "C:\\Program Files\\KeelDesk\\keeldesk.exe",
          args: [],
          endpoint: endpointUrl,
        },
      },
      _meta: {
        "com.microsoft.windows": {
          endpoint_url: endpointUrl,
          static_responses: {
            initialize: {
              protocolVersion: "2025-03-26",
              capabilities: {
                tools: {},
              },
              serverInfo: {
                name: "keeldesk",
                version: "0.1.0",
              },
            },
            "tools/list": {
              tools: [
                {
                  name: "keel_ping",
                  description:
                    "Verify connectivity with the local KeelDesk Windows 11 MCP spine and return runtime status.",
                },
                {
                  name: "scan_dev_env",
                  description:
                    "Probe installed Windows 11 developer toolchains without flashing console windows.",
                },
                {
                  name: "list_active_ports",
                  description:
                    "Enumerate active Windows 11 listening TCPv4/TCPv6 ports via native Win32 IP Helper APIs.",
                },
                {
                  name: "kill_port_process",
                  description:
                    "Terminate the process owning a listening TCP port on Windows 11 (ConfirmRequired).",
                },
              ],
            },
          },
        },
      },
    },
    null,
    2,
  );
}

function buildPreviewOdrStatus(
  endpointUrl: string,
  enrolled = false,
): OdrStatusReport {
  return {
    probed_at_iso: new Date().toISOString(),
    readiness: "missing_odr_cli",
    is_windows_11: true,
    os_build: 26300,
    os_ubr: 0,
    display_version: "24H2",
    meets_odr_build_hint: true,
    developer_mode_enabled: true,
    odr_installed: false,
    odr_binary_path: null,
    odr_version: null,
    keel_enrolled: enrolled,
    enrolled_servers: enrolled
      ? [
          {
            identifier: "keeldesk",
            display_line: `keeldesk  ${endpointUrl}`,
            matches_keel: true,
          },
        ]
      : [],
    raw_list_output: null,
    uri_register_command: `odr.exe mcp add --uri ${endpointUrl}`,
    manifest_register_command: `odr.exe mcp add "${PREVIEW_MANIFEST_PATH}"`,
    unregister_command: "odr.exe mcp remove keeldesk",
    list_command: "odr.exe mcp list",
    default_manifest_path: PREVIEW_MANIFEST_PATH,
    manifest_json: buildPreviewManifestJson(endpointUrl),
    guidance_message:
      "odr.exe was not found on this Windows 11 host. Fallback mode is active: export or inspect the generated MCPB 0.3 manifest and register manually once odr.exe is enabled.",
  };
}

export function useWinOdr(endpointUrl = DEFAULT_ENDPOINT_URL) {
  const [odrStatus, setOdrStatus] = useState<OdrStatusReport | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isActing, setIsActing] = useState(false);
  const [lastOutcome, setLastOutcome] = useState<OdrActionOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshOdrStatus = useCallback(async (): Promise<OdrStatusReport | null> => {
    setIsLoading(true);
    setError(null);

    try {
      if (!isTauriRuntime()) {
        const preview = buildPreviewOdrStatus(
          endpointUrl,
          odrStatus?.keel_enrolled ?? false,
        );
        setOdrStatus(preview);
        return preview;
      }

      const res = await commands.getKeelOdrStatus();
      if (res.status === "ok") {
        setOdrStatus(res.data);
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
  }, [endpointUrl, odrStatus?.keel_enrolled]);

  useEffect(() => {
    if (!isTauriRuntime()) {
      setOdrStatus(buildPreviewOdrStatus(endpointUrl, false));
      setIsLoading(false);
      return;
    }

    let cancelled = false;
    setIsLoading(true);
    setError(null);

    void commands
      .getKeelOdrStatus()
      .then((res) => {
        if (cancelled) {
          return;
        }
        if (res.status === "ok") {
          setOdrStatus(res.data);
        } else {
          setError(res.error);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [endpointUrl]);

  const registerWithOdr = useCallback(
    async (mode: OdrRegisterMode): Promise<OdrActionOutcome | null> => {
      setIsActing(true);
      setError(null);

      try {
        if (!isTauriRuntime()) {
          const status = buildPreviewOdrStatus(endpointUrl, false);
          const commandLine =
            mode === "uri"
              ? status.uri_register_command
              : status.manifest_register_command;
          const outcome: OdrActionOutcome = {
            executed_cli: false,
            succeeded: false,
            mode,
            command_line: commandLine,
            exported_manifest_path: status.default_manifest_path,
            stdout: null,
            stderr: null,
            message: `odr.exe is not installed in browser preview. Simulated fallback MCPB 0.3 manifest write at '${status.default_manifest_path}'. Once odr.exe is available, run \`${commandLine}\`.`,
            status,
          };
          setOdrStatus(status);
          setLastOutcome(outcome);
          return outcome;
        }

        const res = await commands.registerKeelWithOdr(mode);
        if (res.status === "ok") {
          setOdrStatus(res.data.status);
          setLastOutcome(res.data);
          return res.data;
        }

        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setIsActing(false);
      }
    },
    [endpointUrl],
  );

  const unregisterFromOdr = useCallback(
    async (serverId: string | null = null): Promise<OdrActionOutcome | null> => {
      setIsActing(true);
      setError(null);

      try {
        if (!isTauriRuntime()) {
          const targetId = serverId?.trim() || "keeldesk";
          const status = buildPreviewOdrStatus(endpointUrl, false);
          const commandLine = `odr.exe mcp remove ${targetId}`;
          const outcome: OdrActionOutcome = {
            executed_cli: false,
            succeeded: false,
            mode: "unregister",
            command_line: commandLine,
            exported_manifest_path: null,
            stdout: null,
            stderr: null,
            message: `odr.exe is not installed in browser preview; skipped \`${commandLine}\`.`,
            status,
          };
          setOdrStatus(status);
          setLastOutcome(outcome);
          return outcome;
        }

        const res = await commands.unregisterKeelFromOdr(serverId);
        if (res.status === "ok") {
          setOdrStatus(res.data.status);
          setLastOutcome(res.data);
          return res.data;
        }

        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setIsActing(false);
      }
    },
    [endpointUrl],
  );

  const exportManifest = useCallback(
    async (targetPath: string | null = null): Promise<OdrActionOutcome | null> => {
      setIsActing(true);
      setError(null);

      try {
        if (!isTauriRuntime()) {
          const status = buildPreviewOdrStatus(
            endpointUrl,
            odrStatus?.keel_enrolled ?? false,
          );
          const resolvedPath =
            targetPath && targetPath.trim().length > 0
              ? targetPath.trim()
              : status.default_manifest_path;
          const outcome: OdrActionOutcome = {
            executed_cli: false,
            succeeded: true,
            mode: "export_manifest",
            command_line: `odr.exe mcp add "${resolvedPath}"`,
            exported_manifest_path: resolvedPath,
            stdout: null,
            stderr: null,
            message: `Exported KeelDesk MCPB 0.3 manifest to '${resolvedPath}' (preview).`,
            status,
          };
          setOdrStatus(status);
          setLastOutcome(outcome);
          return outcome;
        }

        const res = await commands.exportKeelOdrManifest(targetPath);
        if (res.status === "ok") {
          setOdrStatus(res.data.status);
          setLastOutcome(res.data);
          return res.data;
        }

        setError(res.error);
        return null;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setIsActing(false);
      }
    },
    [endpointUrl, odrStatus?.keel_enrolled],
  );

  const clearOutcome = useCallback(() => {
    setLastOutcome(null);
    setError(null);
  }, []);

  return {
    odrStatus,
    isLoading,
    isActing,
    lastOutcome,
    error,
    refreshOdrStatus,
    registerWithOdr,
    unregisterFromOdr,
    exportManifest,
    clearOutcome,
  };
}
