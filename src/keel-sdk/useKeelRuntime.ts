export {
  isTauriRuntime,
  useMcpServerState,
  type UseMcpServerStateResult,
} from "./useMcpServerState";
import { useMcpServerState } from "./useMcpServerState";

export function useKeelRuntime() {
  return useMcpServerState();
}
