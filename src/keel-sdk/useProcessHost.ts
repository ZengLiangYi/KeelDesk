import {
  buildBuiltinEchoProbeSpec,
  useExternalMcpClients,
} from "./useExternalMcpClients";

export { buildBuiltinEchoProbeSpec };

export function useProcessHost() {
  const sdk = useExternalMcpClients();

  return {
    hostSnapshot: sdk.hostSnapshot,
    isLoading: sdk.isLoading,
    isStarting: sdk.isStarting,
    busyServerIds: sdk.busyClientIds,
    callingServerIds: sdk.callingClientIds,
    lastToolCallResult: sdk.lastToolCallResult,
    stderrByClientId: sdk.stderrByClientId,
    stderrStream: sdk.stderrStream,
    error: sdk.error,
    refreshSnapshot: sdk.refreshSnapshot,
    startChildServer: sdk.startClient,
    stopChildServer: sdk.stopClient,
    removeChildServer: sdk.removeClient,
    restartChildServer: sdk.restartClient,
    callChildServerTool: sdk.callClientTool,
    clearError: sdk.clearError,
  };
}
