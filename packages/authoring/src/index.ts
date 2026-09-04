export interface PackAuthoringDiagnostic {
  code: string;
  entityId?: string;
  file: string;
  jsonPath: string;
  message: string;
  severity: "error" | "warning";
}

export interface PackCanonicalForm {
  canonical: string;
  formIndex: number;
  lawId: string;
  packId: string;
  source: string;
}

export interface PackAuthoringReport {
  archetypes: readonly {
    adoptedLaws: readonly string[];
    archetypeId: string;
    matchingLaws: readonly string[];
    parameterSlots: readonly string[];
  }[];
  bridges: readonly {
    bridgeId: string;
    ownerPackId: string;
    sourceConceptId: string;
    targetConceptId: string;
  }[];
  collisions: readonly {
    distinguishingEvidence: readonly string[];
    leftRelationId: string;
    rightRelationId: string;
    structuralKey: string;
  }[];
  diagnostics: readonly PackAuthoringDiagnostic[];
  forms: readonly PackCanonicalForm[];
  packs: readonly {
    concepts: number;
    laws: number;
    packId: string;
    packVersion: string;
    quantityKinds: number;
    units: number;
  }[];
  schemaVersion: 3;
  signatures: readonly {
    capabilities: readonly string[];
    dependencies: readonly string[];
    packId: string;
    packKind: "application" | "capability" | "field";
    packVersion: string;
    terms: readonly { source: string; text: string }[];
    structuralKeys: readonly string[];
    title: string;
  }[];
}

export interface PackAuthoringRequest {
  schemaVersion: 3;
  sources: readonly { path: string; source: string }[];
}

export interface RuntimeSource {
  path: string;
  source: string;
}

export interface RuntimeBranchViolation {
  id: string;
  line: number;
  path: string;
  sourceLine: string;
}

export function findForbiddenRuntimeBranches(
  sources: readonly RuntimeSource[],
  forbiddenIds: readonly string[],
): RuntimeBranchViolation[] {
  const ids = [...new Set(forbiddenIds)].sort((left, right) => right.length - left.length);
  const violations: RuntimeBranchViolation[] = [];
  for (const file of sources) {
    if (isTestSourcePath(file.path)) continue;
    let testOnly = false;
    for (const [index, sourceLine] of file.source.split(/\r?\n/u).entries()) {
      const line = sourceLine.trim();
      if (line === "#[cfg(test)]") testOnly = true;
      if (testOnly || line.startsWith("//") || line.startsWith("*")) continue;
      if (!/\b(?:if|else if|match|matches|switch|case)\b|=>|===?|!=|\.contains\(|\.ends_with\(/u.test(line)) {
        continue;
      }
      for (const id of ids) {
        if (quotedIdentifier(line, id)) {
          violations.push({
            id,
            line: index + 1,
            path: file.path,
            sourceLine: line,
          });
        }
      }
    }
  }
  return violations;
}

function isTestSourcePath(path: string): boolean {
  const normalized = path.replaceAll("\\", "/");
  return /\.(?:test|spec)\.[cm]?[jt]sx?$/u.test(normalized) ||
    /(?:^|\/)(?:tests?\.rs|[^/]+_tests?\.rs|tests?\/.*\.rs)$/u.test(normalized);
}

export function packagePackAssets(
  sources: PackAuthoringRequest["sources"],
  report: PackAuthoringReport,
): object {
  if (report.diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    throw new Error("cannot package a catalog with compiler errors");
  }
  return {
    compilerReport: report,
    packs: sources.map((source) => ({
      path: source.path,
      value: JSON.parse(source.source) as unknown,
    })),
    schemaVersion: 2,
  };
}

function quotedIdentifier(line: string, id: string): boolean {
  const escaped = escapeRegExp(id);
  return new RegExp(`["']${escaped}["']`, "u").test(line);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}
