import type { CorpusMacro } from "./model";

export interface ObservedRole {
  conceptId?: string;
  role: string;
  symbol: string;
}

export interface ObservedEvidence {
  sourceRanges: readonly unknown[];
}

export function rolesMatch(
  actualRoles: readonly ObservedRole[],
  expectedRoles: Readonly<Record<string, string>> | undefined,
  macros: readonly CorpusMacro[] | undefined,
): boolean {
  if (!expectedRoles) return true;
  if (actualRoles.length === 0) return false;

  const expectedSymbols = Object.values(expectedRoles).map((symbol) =>
    normalizeSymbol(symbol, macros),
  );
  if (
    expectedSymbols.length > 1 &&
    new Set(actualRoles.map((role) => role.role)).size === 1
  ) {
    return sameMultiset(
      actualRoles.map((role) => normalizeSymbol(role.symbol, macros)),
      expectedSymbols,
    );
  }

  const actualByRole = new Map<string, string[]>();
  for (const role of actualRoles) {
    for (const key of [role.role, conceptLeaf(role.conceptId)]) {
      if (!key) continue;
      const normalized = normalizeIdentifier(key);
      const symbols = actualByRole.get(normalized) ?? [];
      symbols.push(normalizeSymbol(role.symbol, macros));
      actualByRole.set(normalized, symbols);
    }
  }
  return Object.entries(expectedRoles).every(([role, symbol]) =>
    (actualByRole.get(normalizeIdentifier(role)) ?? []).includes(
      normalizeSymbol(symbol, macros),
    ),
  );
}

export function roleInstancesMatch(
  actualRoles: readonly ObservedRole[],
  expectedRoles: readonly ObservedRole[],
  macros: readonly CorpusMacro[] | undefined,
): boolean {
  if (actualRoles.length !== expectedRoles.length) return false;
  const actual = groupedRoleSymbols(actualRoles, macros);
  const expected = groupedRoleSymbols(expectedRoles, macros);
  if (actual.size !== expected.size) return false;
  return [...expected].every(([role, symbols]) =>
    sameMultiset(actual.get(role) ?? [], symbols),
  );
}

export function evidenceIsSourceLinked(
  evidence: readonly ObservedEvidence[],
  conditions: readonly unknown[],
): boolean {
  return (
    evidence.length > 0 &&
    evidence.every((item) => item.sourceRanges.length > 0) &&
    conditions.length > 0
  );
}

export function normalizeSymbol(
  symbol: string,
  macros: readonly CorpusMacro[] | undefined,
): string {
  let value = symbol;
  for (const macro of macros ?? []) {
    value = value
      .replaceAll(macro.name, macro.definition);
  }
  for (;;) {
    const next = value.replace(
      /\\(?:mathbf|boldsymbol|vec|mathcal|mathrm|mathit|tilde)\{([^{}]+)\}/gu,
      "$1",
    );
    if (next === value) break;
    value = next;
  }
  return value
    .replace(/\^\{\(1\)\}/gu, "")
    .replace(/_\{([^{}]+)\}/gu, "_$1")
    .replace(/\\(?:rm|mathbf|boldsymbol|vec|mathcal|mathrm|mathit|tilde)\s*/gu, "")
    .replace(/\\([A-Za-z]+)/gu, "$1")
    .replace(/[{}\s]/gu, "");
}

function conceptLeaf(conceptId: string | undefined): string | undefined {
  return conceptId?.split(":").at(-1);
}

function normalizeIdentifier(value: string): string {
  return value
    .replace(/([a-z0-9])([A-Z])/gu, "$1-$2")
    .toLowerCase();
}

function sameMultiset(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length) return false;
  const sortedLeft = [...left].sort();
  const sortedRight = [...right].sort();
  return sortedLeft.every((value, index) => value === sortedRight[index]);
}

function groupedRoleSymbols(
  roles: readonly ObservedRole[],
  macros: readonly CorpusMacro[] | undefined,
): Map<string, string[]> {
  const grouped = new Map<string, string[]>();
  for (const role of roles) {
    const key = normalizeIdentifier(role.role);
    const symbols = grouped.get(key) ?? [];
    symbols.push(normalizeSymbol(role.symbol, macros));
    grouped.set(key, symbols);
  }
  return grouped;
}
