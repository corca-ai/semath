import {
  mkdir,
  mkdtemp,
  open,
  readFile,
  rename,
  rm,
} from "node:fs/promises";
import { dirname, join } from "node:path";

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
  if (!isCleanStartedReceipt(receipt)) {
    throw new Error("receipt reservation requires a clean started receipt");
  }
  await mkdir(dirname(path), { recursive: true });
  const file = await open(path, "wx");
  try {
    await file.writeFile(serialize(receipt));
    await file.sync();
  } finally {
    await file.close();
  }
}

export async function finalizeFreshBlindReceipt(
  path: string,
  receipt: FreshBlindReleaseReceipt,
): Promise<void> {
  const current = JSON.parse(await readFile(path, "utf8")) as FreshBlindReleaseReceipt;
  const planned = planFreshBlindReceiptTransition(current, receipt);
  const temporaryDirectory = await mkdtemp(
    join(dirname(path), ".fresh-blind-receipt-"),
  );
  const temporaryPath = join(temporaryDirectory, "receipt.json");
  try {
    const file = await open(temporaryPath, "wx");
    try {
      await file.writeFile(serialize(planned));
      await file.sync();
    } finally {
      await file.close();
    }
    await rename(temporaryPath, path);
  } finally {
    await rm(temporaryDirectory, { force: true, recursive: true });
  }
}

export function planFreshBlindReceiptTransition(
  current: FreshBlindReleaseReceipt,
  terminal: FreshBlindReleaseReceipt,
): FreshBlindReleaseReceipt {
  if (current.status !== "started") {
    throw new Error(
      "a terminal transition requires an existing started receipt",
    );
  }
  if (!isCleanStartedReceipt(current)) {
    throw new Error("a terminal transition requires a clean started receipt");
  }
  if (!isTerminalStatus(terminal.status) || !terminal.completedAt) {
    throw new Error(
      "a final receipt requires a terminal status and completedAt",
    );
  }
  if (!sameReservedExecution(current, terminal)) {
    throw new Error(
      "a terminal receipt must describe the same reserved execution",
    );
  }
  return terminal;
}

function isCleanStartedReceipt(receipt: FreshBlindReleaseReceipt): boolean {
  return (
    receipt.status === "started" &&
    receipt.completedAt === undefined &&
    receipt.error === undefined &&
    receipt.result === undefined
  );
}

function isTerminalStatus(
  status: FreshBlindReleaseReceipt["status"],
): status is Exclude<FreshBlindReleaseReceipt["status"], "started"> {
  return (
    status === "completed" ||
    status === "safety-failed" ||
    status === "execution-error"
  );
}

function sameReservedExecution(
  current: FreshBlindReleaseReceipt,
  terminal: FreshBlindReleaseReceipt,
): boolean {
  return (
    current.schemaVersion === terminal.schemaVersion &&
    current.startedAt === terminal.startedAt &&
    current.fixture.id === terminal.fixture.id &&
    current.fixture.seal === terminal.fixture.seal &&
    current.provenance.nativeSha256 === terminal.provenance.nativeSha256 &&
    current.provenance.semathCommit === terminal.provenance.semathCommit &&
    current.provenance.wasmSha256 === terminal.provenance.wasmSha256 &&
    current.provenance.wasmtexCommit === terminal.provenance.wasmtexCommit
  );
}

function serialize(receipt: FreshBlindReleaseReceipt): string {
  return `${JSON.stringify(receipt, null, 2)}\n`;
}
