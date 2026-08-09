/* tslint:disable */
/* eslint-disable */
export function createPackTemplate(pack_id: string): string;
export function inspectPackCatalog(payload: Uint8Array): Uint8Array;
export class SemathEngine {
  free(): void;
  applyChanges(payload: Uint8Array): Uint8Array;
  resetProject(payload: Uint8Array): Uint8Array;
  constructor();
  query(payload: Uint8Array): Uint8Array;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_semathengine_free: (a: number, b: number) => void;
  readonly createPackTemplate: (a: number, b: number, c: number) => void;
  readonly inspectPackCatalog: (a: number, b: number, c: number) => void;
  readonly semathengine_applyChanges: (a: number, b: number, c: number, d: number) => void;
  readonly semathengine_new: () => number;
  readonly semathengine_query: (a: number, b: number, c: number, d: number) => void;
  readonly semathengine_resetProject: (a: number, b: number, c: number, d: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_export_0: (a: number, b: number) => number;
  readonly __wbindgen_export_1: (a: number, b: number, c: number) => void;
  readonly __wbindgen_export_2: (a: number, b: number, c: number, d: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
