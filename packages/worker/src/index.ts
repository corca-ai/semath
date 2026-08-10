import type {
  ChangeEnvelope,
  ProjectSnapshot,
  QueryEnvelope,
  QueryResult,
} from "../../protocol/src/index";

interface WasmEngine {
  applyChanges(payload: Uint8Array): Uint8Array;
  beginReset(payload: Uint8Array): void;
  finishReset(): Uint8Array;
  free(): void;
  ingestResetDocument(payload: Uint8Array): void;
  query(payload: Uint8Array): Uint8Array;
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
    const { documents, ...metadata } = snapshot;
    this.engine.beginReset(encode(metadata));
    for (const document of documents) this.engine.ingestResetDocument(encode(document));
    return decode(this.engine.finishReset());
  }

  apply(changes: ChangeEnvelope) {
    return decode(this.engine.applyChanges(encode(changes)));
  }

  query(envelope: QueryEnvelope): QueryResult {
    return decode(this.engine.query(encode(envelope)));
  }

  dispose(): void {
    this.engine.free();
  }
}

function encode(value: unknown): Uint8Array {
  return encoder.encode(JSON.stringify(value));
}

function decode<T>(payload: Uint8Array): T {
  return JSON.parse(decoder.decode(payload)) as T;
}
