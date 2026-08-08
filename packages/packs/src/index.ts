import calculusAnalysis from "../../../packs/calculus-analysis/v1.json" with {
  type: "json",
};
import discreteMath from "../../../packs/discrete-math/v1.json" with {
  type: "json",
};
import linearAlgebra from "../../../packs/linear-algebra/v1.json" with {
  type: "json",
};
import optimizationMl from "../../../packs/optimization-ml/v1.json" with {
  type: "json",
};
import probability from "../../../packs/probability/v1.json" with {
  type: "json",
};
import type {
  FormulaConstraint,
  FormulaParameter,
  FormulaSideCondition,
} from "../../protocol/src/index";

export const SEMATH_PACK_SCHEMA_VERSION = 2 as const;

export type PackMaturity =
  | "completion"
  | "diagnostic"
  | "recognition"
  | "rewrite";

export interface PackActivationRule {
  id: string;
  patterns: readonly string[];
  references: readonly string[];
  topic: string;
}

export interface PackVocabularyEntry {
  description: string;
  id: string;
  notation: readonly string[];
  references: readonly string[];
  topic: string;
}

export interface PackMatcher {
  expression?: string;
  primitive: string;
}

export interface PackPattern {
  conditionDescriptions: readonly string[];
  description: string;
  descriptionKey: string;
  generationTemplate?: string;
  id: string;
  matcher: PackMatcher;
  maturity: PackMaturity;
  parameters: readonly FormulaParameter[];
  references: readonly string[];
  result: FormulaConstraint;
  sideConditions: readonly FormulaSideCondition[];
  title: string;
  topic: string;
}

export interface PackRequiredRefinement {
  parameter: string;
  refinement: string;
}

export interface PackRewrite {
  description: string;
  id: string;
  references: readonly string[];
  replacementTemplate: string;
  requiredRefinements: readonly PackRequiredRefinement[];
  sourcePattern: string;
  title: string;
  topic: string;
}

export interface PackReference {
  citation: string;
  id: string;
  title: string;
  url?: string;
}

export interface DomainPack {
  activationRules: readonly PackActivationRule[];
  description: string;
  operators: readonly PackVocabularyEntry[];
  packId: string;
  packVersion: string;
  patterns: readonly PackPattern[];
  references: readonly PackReference[];
  rewrites: readonly PackRewrite[];
  roles: readonly PackVocabularyEntry[];
  schemaVersion: typeof SEMATH_PACK_SCHEMA_VERSION;
  title: string;
}

export interface PackValidationError {
  message: string;
  path: string;
}

export type PackValidationResult =
  | { errors: readonly PackValidationError[]; ok: false }
  | { ok: true; pack: DomainPack };

const ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const VERSION = /^[0-9]+\.[0-9]+\.[0-9]+(?:-[a-z0-9.-]+)?$/;
const MATURITIES = new Set<PackMaturity>([
  "recognition",
  "completion",
  "diagnostic",
  "rewrite",
]);
const MATCHER_PRIMITIVES = new Set([
  "binary-product",
  "conditional-probability",
  "event-probability",
  "expectation",
  "quadratic-form",
  "regex-captures",
  "transpose",
  "transposed-binary-product",
  "variance",
]);

const RAW_BUILT_INS: readonly unknown[] = [
  linearAlgebra,
  probability,
  calculusAnalysis,
  optimizationMl,
  discreteMath,
];

export function loadPack(source: string | unknown): PackValidationResult {
  if (typeof source === "string" && source.length > 256 * 1024) {
    return invalid("pack", "source exceeds the 256 KiB limit");
  }
  let value: unknown = source;
  if (typeof source === "string") {
    try {
      value = JSON.parse(source);
    } catch (cause) {
      return invalid("pack", `invalid JSON: ${errorMessage(cause)}`);
    }
  }
  return validatePack(value);
}

export function validatePack(value: unknown): PackValidationResult {
  const pack = record(value);
  if (!pack) return invalid("pack", "must be an object");
  if (pack.schemaVersion !== SEMATH_PACK_SCHEMA_VERSION) {
    return invalid(
      "schemaVersion",
      `unsupported schema ${String(pack.schemaVersion)}; expected ${SEMATH_PACK_SCHEMA_VERSION}`,
    );
  }
  if (!identifier(pack.packId)) {
    return invalid("packId", "must be a lowercase kebab-case identifier");
  }
  if (typeof pack.packVersion !== "string" || !VERSION.test(pack.packVersion)) {
    return invalid("packVersion", "must be semantic version x.y.z");
  }
  for (const key of ["title", "description"] as const) {
    if (!text(pack[key])) return invalid(key, "must not be blank");
  }
  const references = records(pack.references);
  if (!references?.length) return invalid("references", "must not be empty");
  const referenceIds = new Set<string>();
  for (const [index, reference] of references.entries()) {
    if (!identifier(reference.id)) {
      return invalid(`references[${index}].id`, "must be a kebab-case ID");
    }
    if (referenceIds.has(reference.id)) {
      return invalid(`references[${index}].id`, "must be unique");
    }
    referenceIds.add(reference.id);
    if (!text(reference.title) || !text(reference.citation)) {
      return invalid(`references[${index}]`, "requires title and citation");
    }
  }

  const activationRules = records(pack.activationRules);
  if (!activationRules?.length) {
    return invalid("activationRules", "must not be empty");
  }
  for (const [index, rule] of activationRules.entries()) {
    const path = `activationRules[${index}]`;
    if (!identifier(rule.id) || !text(rule.topic) || !strings(rule.patterns)?.length) {
      return invalid(path, "requires ID, topic, and literal patterns");
    }
    const badReference = unknownReference(rule.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  const patterns = records(pack.patterns);
  if (!patterns?.length || patterns.length > 256) {
    return invalid("patterns", "must contain 1–256 entries");
  }
  const patternIds = new Set<string>();
  const maturityByPattern = new Map<string, PackMaturity>();
  for (const [index, pattern] of patterns.entries()) {
    const path = `patterns[${index}]`;
    if (!identifier(pattern.id) || patternIds.has(pattern.id)) {
      return invalid(`${path}.id`, "must be a unique kebab-case ID");
    }
    patternIds.add(pattern.id);
    if (!text(pattern.title) || !text(pattern.topic) || !text(pattern.description)) {
      return invalid(path, "requires topic, title, and description");
    }
    if (!identifier(pattern.descriptionKey)) {
      return invalid(`${path}.descriptionKey`, "must be a kebab-case ID");
    }
    if (typeof pattern.maturity !== "string" || !MATURITIES.has(pattern.maturity as PackMaturity)) {
      return invalid(`${path}.maturity`, "is not a supported maturity");
    }
    maturityByPattern.set(pattern.id, pattern.maturity as PackMaturity);
    const matcher = record(pattern.matcher);
    if (!matcher || typeof matcher.primitive !== "string" || !MATCHER_PRIMITIVES.has(matcher.primitive)) {
      return invalid(`${path}.matcher.primitive`, "is not a safe built-in primitive");
    }
    if (matcher.primitive === "regex-captures") {
      if (typeof matcher.expression !== "string" || matcher.expression.length > 512) {
        return invalid(`${path}.matcher.expression`, "requires a bounded regex");
      }
      try {
        const regex = new RegExp(matcher.expression);
        if (regex.test("")) {
          return invalid(`${path}.matcher.expression`, "must not match empty input");
        }
      } catch (cause) {
        return invalid(
          `${path}.matcher.expression`,
          `invalid regex: ${errorMessage(cause)}`,
        );
      }
    } else if (matcher.expression !== undefined) {
      return invalid(`${path}.matcher.expression`, "is only valid for regex-captures");
    }
    const parameters = records(pattern.parameters);
    const parameterIds = new Set(
      parameters?.flatMap((parameter) =>
        identifier(parameter.id) ? [parameter.id] : [],
      ) ?? [],
    );
    if ((parameters?.length ?? 0) !== parameterIds.size) {
      return invalid(`${path}.parameters`, "requires unique kebab-case IDs");
    }
    const maturity = pattern.maturity as PackMaturity;
    if (maturity === "completion" || maturity === "rewrite") {
      if (!text(pattern.generationTemplate)) {
        return invalid(
          `${path}.generationTemplate`,
          "completion/rewrite maturity requires a template",
        );
      }
    } else if (pattern.generationTemplate !== undefined) {
      return invalid(
        `${path}.generationTemplate`,
        "recognition/diagnostic maturity cannot produce edits",
      );
    }
    const badReference = unknownReference(pattern.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  const rewrites = records(pack.rewrites) ?? [];
  const rewriteIds = new Set<string>();
  for (const [index, rewrite] of rewrites.entries()) {
    const path = `rewrites[${index}]`;
    if (!identifier(rewrite.id) || rewriteIds.has(rewrite.id)) {
      return invalid(`${path}.id`, "must be a unique kebab-case ID");
    }
    rewriteIds.add(rewrite.id);
    if (
      typeof rewrite.sourcePattern !== "string" ||
      maturityByPattern.get(rewrite.sourcePattern) !== "rewrite"
    ) {
      return invalid(`${path}.sourcePattern`, "must name a rewrite-mature pattern");
    }
    if (!records(rewrite.requiredRefinements)?.length || !text(rewrite.replacementTemplate)) {
      return invalid(path, "requires side conditions and a replacement template");
    }
    const badReference = unknownReference(rewrite.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  return { ok: true, pack: value as DomainPack };
}

const BUILT_IN_PACKS: readonly DomainPack[] = RAW_BUILT_INS.map((raw) => {
  const result = validatePack(raw);
  if (!result.ok) {
    const first = result.errors[0];
    throw new Error(`Invalid built-in Semath pack: ${first?.path}: ${first?.message}`);
  }
  return freezeDeep(result.pack);
});

export function builtInPacks(): readonly DomainPack[] {
  return BUILT_IN_PACKS;
}

function invalid(path: string, message: string): PackValidationResult {
  return { errors: [{ message, path }], ok: false };
}

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function records(value: unknown): readonly Record<string, unknown>[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const values = value.map(record);
  return values.every((entry) => entry !== undefined)
    ? (values as readonly Record<string, unknown>[])
    : undefined;
}

function strings(value: unknown): readonly string[] | undefined {
  return Array.isArray(value) && value.every(text)
    ? (value as readonly string[])
    : undefined;
}

function identifier(value: unknown): value is string {
  return typeof value === "string" && ID.test(value);
}

function text(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function unknownReference(value: unknown, known: ReadonlySet<string>): string | undefined {
  const references = strings(value);
  if (!references?.length) return "must cite at least one pack reference";
  const unknown = references.find((reference) => !known.has(reference));
  return unknown ? `unknown reference ${unknown}` : undefined;
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

function freezeDeep<T>(value: T): T {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) {
    return value;
  }
  for (const child of Object.values(value)) freezeDeep(child);
  return Object.freeze(value);
}
