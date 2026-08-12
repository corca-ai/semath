import { mkdir, open, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

export interface FreshBlindReleaseReceipt {
  readonly artifacts?: Readonly<Record<string, string>>;
  readonly completedAt?: string;
  readonly error?: string;
  readonly fixture: {
    readonly id: string;
    readonly seal: string;
  };
  readonly provenance: {
    readonly nativeSha256: string;
    readonly semathCommit: string;
    readonly wasmSha256: string;
    readonly wasmtexCommit: string;
  };
  readonly result?: unknown;
  readonly schemaVersion: 1;
  readonly startedAt: string;
  readonly status:
    | "started"
    | "completed"
    | "safety-failed"
    | "execution-error";
}

/** Reserve the receipt before the first engine query. Exclusive creation is the
 * one-shot boundary: reusing a receipt path is always an operator error. */
export async function reserveFreshBlindReceipt(
  path: string,
  receipt: FreshBlindReleaseReceipt,
): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const file = await open(path, "wx");
  try {
    await file.writeFile(serialize(receipt));
  } finally {
    await file.close();
  }
}

export async function finalizeFreshBlindReceipt(
  path: string,
  receipt: FreshBlindReleaseReceipt,
): Promise<void> {
  if (receipt.status === "started" || !receipt.completedAt) {
    throw new Error("a final receipt requires a terminal status and completedAt");
  }
  await writeFile(path, serialize(receipt));
}

function serialize(receipt: FreshBlindReleaseReceipt): string {
  return `${JSON.stringify(receipt, null, 2)}\n`;
}
