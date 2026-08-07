import init, { SemathEngine as WasmSemathEngine } from "../../../lib/wasm/semath_wasm.js";
import { SemathWorkerEngine } from "./index";

interface WorkerRequest {
  id: number;
  method: "apply" | "query" | "reset";
  payload: unknown;
}

let enginePromise: Promise<SemathWorkerEngine> | undefined;

function engine(): Promise<SemathWorkerEngine> {
  enginePromise ??= SemathWorkerEngine.create(async () => ({
    default: init,
    SemathEngine: WasmSemathEngine,
  }));
  return enginePromise;
}

self.addEventListener("message", async (event: MessageEvent<WorkerRequest>) => {
  const { id, method, payload } = event.data;
  try {
    const instance = await engine();
    const result =
      method === "reset"
        ? instance.reset(payload as Parameters<typeof instance.reset>[0])
        : method === "apply"
          ? instance.apply(payload as Parameters<typeof instance.apply>[0])
          : instance.query(payload as Parameters<typeof instance.query>[0]);
    self.postMessage({ id, result });
  } catch (error) {
    self.postMessage({ id, error: error instanceof Error ? error.message : String(error) });
  }
});
