import init, { SemathEngine as WasmSemathEngine } from "../../../lib/wasm/semath_wasm.js";
import type { SemathWorkerRequest } from "../../protocol/src/index";
import { SemathWorkerHost } from "./host";
import { SemathWorkerEngine } from "./index";

const host = new SemathWorkerHost(
  () =>
    SemathWorkerEngine.create(async () => ({
    default: init,
    SemathEngine: WasmSemathEngine,
    })),
  (response) => self.postMessage(response),
);

self.addEventListener("message", (event: MessageEvent<SemathWorkerRequest>) => {
  host.accept(event.data);
});
