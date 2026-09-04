import { realpath } from "node:fs/promises";

/**
 * Compare filesystem identity, not path spelling. This intentionally treats
 * macOS `/tmp` and `/private/tmp` aliases like any other canonical alias.
 */
export async function assertSameCanonicalPath(
  actualPath: string,
  expectedPath: string,
  label: string,
): Promise<void> {
  const [actual, expected] = await Promise.all([
    realpath(actualPath),
    realpath(expectedPath),
  ]);
  if (actual !== expected) {
    throw new Error(`${label}: canonical path does not match`);
  }
}
