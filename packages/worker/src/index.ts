import type {
  ChangeEnvelope,
  ProjectSnapshot,
  QueryEnvelope,
  QueryResult,
} from "../../protocol/src/index";

interface WasmEngine {
  applyChanges(payload: Uint8Array): Uint8Array;
  free(): void;
  query(payload: Uint8Array): Uint8Array;
  resetProject(payload: Uint8Array): Uint8Array;
}

interface WasmModule {
  default(input?: unknown): Promise<unknown>;
  SemathEngine: new () => WasmEngine;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export class SemathWorkerEngine {
  private constructor(private readonly engine: WasmEngine) {}

  static async create(load: () => Promise<WasmModule>) {
    const wasm = await load();
    await wasm.default();
    return new SemathWorkerEngine(new wasm.SemathEngine());
  }

  reset(snapshot: ProjectSnapshot) {
    return decode(this.engine.resetProject(encoder.encode(JSON.stringify(snapshot))));
  }

  apply(changes: ChangeEnvelope) {
    return decode(this.engine.applyChanges(encoder.encode(JSON.stringify(changes))));
  }

  query(envelope: QueryEnvelope): QueryResult {
    return decode(this.engine.query(encoder.encode(JSON.stringify(envelope))));
  }

  dispose(): void {
    this.engine.free();
  }
}

function decode<T>(payload: Uint8Array): T {
  return JSON.parse(decoder.decode(payload)) as T;
}
