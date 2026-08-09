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
import classicalMechanics from "../../../packs/classical-mechanics/v1.json" with {
  type: "json",
};
import circuits from "../../../packs/circuits/v1.json" with { type: "json" };
import controlSystems from "../../../packs/control-systems/v1.json" with {
  type: "json",
};
import quantitiesUnits from "../../../packs/quantities-units/v1.json" with {
  type: "json",
};
import type {
  FormulaConstraint,
  FormulaParameter,
  FormulaSideCondition,
} from "../../protocol/src/index";

export const SEMATH_PACK_SCHEMA_VERSION = 3 as const;

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

export type PackKind = "application" | "capability" | "field";

export interface PackDependency {
  packId: string;
  requiredCapabilities: readonly string[];
  versionMajor: number;
}

export interface PackCapabilities {
  provides: readonly string[];
  requires: readonly string[];
}

export interface PackConcept {
  conceptKind: "entity" | "operator" | "quantity" | "relation" | "system";
  description: string;
  id: string;
  parents: readonly string[];
  references: readonly string[];
  title: string;
}

export interface PackDimensionExponent {
  base: string;
  denominator: number;
  numerator: number;
}

export interface PackRational {
  denominator: number;
  numerator: number;
}

export interface PackQuantityKind {
  defaultUnit?: string;
  description: string;
  dimension: readonly PackDimensionExponent[];
  id: string;
  references: readonly string[];
  title: string;
}

export interface PackUnit {
  aliases: readonly string[];
  dimension: readonly PackDimensionExponent[];
  id: string;
  offset?: PackRational;
  references: readonly string[];
  scale: PackRational;
  symbol: string;
}

export interface PackLawRole {
  concept: string;
  description: string;
  id: string;
}

export interface PackLaw {
  conditions: readonly string[];
  description: string;
  id: string;
  references: readonly string[];
  roles: readonly PackLawRole[];
  title: string;
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
  relation?: PackPatternRelation;
  sideConditions: readonly FormulaSideCondition[];
  title: string;
  topic: string;
}

export interface PackPatternRelation {
  law: string;
  roleBindings: readonly PackRelationRoleBinding[];
}

export interface PackRelationRoleBinding {
  parameter: string;
  role: string;
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
  capabilities: PackCapabilities;
  concepts: readonly PackConcept[];
  dependencies: readonly PackDependency[];
  laws: readonly PackLaw[];
  namespace: string;
  operators: readonly PackVocabularyEntry[];
  packId: string;
  packKind: PackKind;
  packVersion: string;
  patterns: readonly PackPattern[];
  quantityKinds: readonly PackQuantityKind[];
  references: readonly PackReference[];
  rewrites: readonly PackRewrite[];
  roles: readonly PackVocabularyEntry[];
  schemaVersion: typeof SEMATH_PACK_SCHEMA_VERSION;
  title: string;
  units: readonly PackUnit[];
}

export interface PackValidationError {
  message: string;
  path: string;
}

export type PackValidationResult =
  | { errors: readonly PackValidationError[]; ok: false }
  | { ok: true; pack: DomainPack };

export type PackCatalogValidationResult =
  | { errors: readonly PackValidationError[]; ok: false }
  | { ok: true; packs: readonly DomainPack[] };

const ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const QUALIFIED_ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*:[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
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
  quantitiesUnits,
  classicalMechanics,
  circuits,
  controlSystems,
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
  if (!identifier(pack.namespace)) {
    return invalid("namespace", "must be a lowercase kebab-case identifier");
  }
  if (!(["application", "capability", "field"] as const).includes(pack.packKind as PackKind)) {
    return invalid("packKind", "is not a supported pack kind");
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

  const dependencies = records(pack.dependencies);
  if (!dependencies) return invalid("dependencies", "must be an array");
  const dependencyIds = new Set<string>();
  for (const [index, dependency] of dependencies.entries()) {
    const path = `dependencies[${index}]`;
    if (!identifier(dependency.packId)) {
      return invalid(`${path}.packId`, "must be a kebab-case ID");
    }
    if (dependency.packId === pack.packId) {
      return invalid(`${path}.packId`, "pack cannot depend on itself");
    }
    if (dependencyIds.has(dependency.packId)) {
      return invalid(`${path}.packId`, "dependency must be unique");
    }
    dependencyIds.add(dependency.packId);
    if (!Number.isInteger(dependency.versionMajor) || Number(dependency.versionMajor) < 0) {
      return invalid(`${path}.versionMajor`, "must be a nonnegative integer");
    }
    const requiredCapabilities = qualifiedStrings(dependency.requiredCapabilities);
    if (!requiredCapabilities) {
      return invalid(`${path}.requiredCapabilities`, "must contain unique qualified IDs");
    }
  }

  const capabilities = record(pack.capabilities);
  if (!capabilities) return invalid("capabilities", "must be an object");
  if (!qualifiedStrings(capabilities.provides)) {
    return invalid("capabilities.provides", "must contain unique qualified IDs");
  }
  if (!qualifiedStrings(capabilities.requires)) {
    return invalid("capabilities.requires", "must contain unique qualified IDs");
  }

  const concepts = records(pack.concepts);
  if (!concepts) return invalid("concepts", "must be an array");
  const conceptIds = new Set<string>();
  for (const [index, concept] of concepts.entries()) {
    const path = `concepts[${index}]`;
    if (!identifier(concept.id) || conceptIds.has(concept.id)) {
      return invalid(`${path}.id`, "must be a unique kebab-case ID");
    }
    conceptIds.add(concept.id);
    if (!(["entity", "operator", "quantity", "relation", "system"] as const).includes(concept.conceptKind as PackConcept["conceptKind"])) {
      return invalid(`${path}.conceptKind`, "is not a supported concept kind");
    }
    if (!text(concept.title) || !text(concept.description)) {
      return invalid(path, "requires title and description");
    }
    if (!qualifiedStrings(concept.parents)) {
      return invalid(`${path}.parents`, "must contain unique qualified IDs");
    }
    const badReference = unknownReference(concept.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  const laws = records(pack.laws);
  if (!laws) return invalid("laws", "must be an array");
  const lawIds = new Set<string>();
  for (const [index, law] of laws.entries()) {
    const path = `laws[${index}]`;
    if (!identifier(law.id) || lawIds.has(law.id)) {
      return invalid(`${path}.id`, "must be a unique kebab-case ID");
    }
    lawIds.add(law.id);
    if (!text(law.title) || !text(law.description)) {
      return invalid(path, "requires title and description");
    }
    const roles = records(law.roles);
    if (!roles) return invalid(`${path}.roles`, "must be an array");
    const roleIds = new Set<string>();
    for (const [roleIndex, role] of roles.entries()) {
      const rolePath = `${path}.roles[${roleIndex}]`;
      if (!identifier(role.id) || roleIds.has(role.id)) {
        return invalid(`${rolePath}.id`, "must be a unique kebab-case ID");
      }
      roleIds.add(role.id);
      if (!qualifiedIdentifier(role.concept) || !text(role.description)) {
        return invalid(rolePath, "requires a qualified concept and description");
      }
      const localPrefix = `${String(pack.namespace)}:`;
      if (role.concept.startsWith(localPrefix) && !conceptIds.has(role.concept.slice(localPrefix.length))) {
        return invalid(`${rolePath}.concept`, `unknown local concept ${String(role.concept)}`);
      }
    }
    if (!strings(law.conditions)) {
      return invalid(`${path}.conditions`, "must contain non-blank strings");
    }
    const badReference = unknownReference(law.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  const quantityKinds = records(pack.quantityKinds);
  if (!quantityKinds) return invalid("quantityKinds", "must be an array");
  const quantityKindIds = new Set<string>();
  for (const [index, quantity] of quantityKinds.entries()) {
    const path = `quantityKinds[${index}]`;
    if (!identifier(quantity.id) || quantityKindIds.has(quantity.id)) {
      return invalid(`${path}.id`, "must be a unique kebab-case ID");
    }
    quantityKindIds.add(quantity.id);
    if (!text(quantity.title) || !text(quantity.description)) {
      return invalid(path, "requires title and description");
    }
    if (!validDimension(quantity.dimension)) {
      return invalid(`${path}.dimension`, "must contain unique exact nonzero exponents");
    }
    if (quantity.defaultUnit !== undefined && !qualifiedIdentifier(quantity.defaultUnit)) {
      return invalid(`${path}.defaultUnit`, "must be a qualified unit ID");
    }
    const badReference = unknownReference(quantity.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  const units = records(pack.units);
  if (!units) return invalid("units", "must be an array");
  const unitIds = new Set<string>();
  for (const [index, unit] of units.entries()) {
    const path = `units[${index}]`;
    if (!identifier(unit.id) || unitIds.has(unit.id)) {
      return invalid(`${path}.id`, "must be a unique kebab-case ID");
    }
    unitIds.add(unit.id);
    if (!text(unit.symbol) || !strings(unit.aliases) || !validDimension(unit.dimension)) {
      return invalid(path, "requires a symbol, aliases, and exact dimension");
    }
    if (!validRational(unit.scale, false) || (unit.offset !== undefined && !validRational(unit.offset, true))) {
      return invalid(path, "requires valid exact scale and offset rationals");
    }
    const badReference = unknownReference(unit.references, referenceIds);
    if (badReference) return invalid(`${path}.references`, badReference);
  }

  const activationRules = records(pack.activationRules);
  if (!activationRules) return invalid("activationRules", "must be an array");
  if (activationRules.length === 0 && pack.packKind !== "capability") {
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
  if (!patterns || patterns.length > 256 || (patterns.length === 0 && laws.length === 0 && pack.packKind !== "capability")) {
    return invalid("patterns", "must contain 1–256 entries unless a capability/law pack");
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
    for (const [parameterIndex, parameter] of (parameters ?? []).entries()) {
      if (!validFormulaConstraint(parameter.constraint)) {
        return invalid(
          `${path}.parameters[${parameterIndex}].constraint`,
          "requires a known structural kind and qualified concept IDs",
        );
      }
    }
    if (!validFormulaConstraint(pattern.result)) {
      return invalid(
        `${path}.result`,
        "requires a known structural kind and qualified concept IDs",
      );
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
    if (pattern.relation !== undefined) {
      const relation = record(pattern.relation);
      if (!relation || !identifier(relation.law)) {
        return invalid(`${path}.relation.law`, "must name a law in this pack");
      }
      const law = laws.find((candidate) => candidate.id === relation.law);
      if (!law) return invalid(`${path}.relation.law`, `unknown law ${relation.law}`);
      const lawRoles = records(law.roles) ?? [];
      const knownRoles = new Set(lawRoles.flatMap((role) => identifier(role.id) ? [role.id] : []));
      const roleBindings = records(relation.roleBindings);
      if (!roleBindings) {
        return invalid(`${path}.relation.roleBindings`, "must be an array");
      }
      const boundRoles = new Set<string>();
      for (const [bindingIndex, binding] of roleBindings.entries()) {
        const bindingPath = `${path}.relation.roleBindings[${bindingIndex}]`;
        if (!identifier(binding.parameter) || !parameterIds.has(binding.parameter)) {
          return invalid(`${bindingPath}.parameter`, `unknown parameter ${String(binding.parameter)}`);
        }
        if (!identifier(binding.role) || !knownRoles.has(binding.role)) {
          return invalid(`${bindingPath}.role`, `unknown law role ${String(binding.role)}`);
        }
        if (boundRoles.has(binding.role)) {
          return invalid(`${bindingPath}.role`, "law role must be bound at most once");
        }
        boundRoles.add(binding.role);
      }
    }
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

const BUILT_IN_CATALOG = validatePackCatalog(BUILT_IN_PACKS);
if (!BUILT_IN_CATALOG.ok) {
  const first = BUILT_IN_CATALOG.errors[0];
  throw new Error(`Invalid built-in Semath catalog: ${first?.path}: ${first?.message}`);
}

export function builtInPacks(): readonly DomainPack[] {
  return BUILT_IN_PACKS;
}

export function validatePackCatalog(
  values: readonly unknown[],
): PackCatalogValidationResult {
  const packs: DomainPack[] = [];
  for (const [index, value] of values.entries()) {
    const result = validatePack(value);
    if (!result.ok) {
      const first = result.errors[0];
      return invalidCatalog(`packs[${index}].${first?.path ?? "pack"}`, first?.message ?? "invalid pack");
    }
    packs.push(result.pack);
  }

  const packById = new Map<string, DomainPack>();
  const namespaceOwners = new Map<string, string>();
  for (const [index, pack] of packs.entries()) {
    if (packById.has(pack.packId)) {
      return invalidCatalog(`packs[${index}].packId`, `duplicate pack ${pack.packId}`);
    }
    const namespaceOwner = namespaceOwners.get(pack.namespace);
    if (namespaceOwner) {
      return invalidCatalog(
        `packs[${index}].namespace`,
        `duplicate namespace ${pack.namespace} owned by ${namespaceOwner}`,
      );
    }
    packById.set(pack.packId, pack);
    namespaceOwners.set(pack.namespace, pack.packId);
  }

  for (const [index, pack] of packs.entries()) {
    for (const [dependencyIndex, dependency] of pack.dependencies.entries()) {
      const path = `packs[${index}].dependencies[${dependencyIndex}]`;
      const dependencyPack = packById.get(dependency.packId);
      if (!dependencyPack) {
        return invalidCatalog(`${path}.packId`, `unknown dependency ${dependency.packId}`);
      }
      if (majorVersion(dependencyPack.packVersion) !== dependency.versionMajor) {
        return invalidCatalog(
          `${path}.versionMajor`,
          `dependency ${dependency.packId} has incompatible version ${dependencyPack.packVersion}`,
        );
      }
      const missing = dependency.requiredCapabilities.find(
        (capability) => !dependencyPack.capabilities.provides.includes(capability),
      );
      if (missing) {
        return invalidCatalog(
          `${path}.requiredCapabilities`,
          `dependency ${dependency.packId} does not provide ${missing}`,
        );
      }
    }
  }

  const visited = new Set<string>();
  const visiting = new Set<string>();
  const visit = (pack: DomainPack): PackValidationError | undefined => {
    if (visited.has(pack.packId)) return undefined;
    if (visiting.has(pack.packId)) {
      return { message: `dependency cycle includes ${pack.packId}`, path: "dependencies" };
    }
    visiting.add(pack.packId);
    for (const dependency of pack.dependencies) {
      const next = packById.get(dependency.packId);
      if (!next) continue;
      const error = visit(next);
      if (error) return error;
    }
    visiting.delete(pack.packId);
    visited.add(pack.packId);
    return undefined;
  };
  for (const pack of packs) {
    const error = visit(pack);
    if (error) return { errors: [error], ok: false };
  }

  const concepts = new Set<string>(
    packs.flatMap((pack) =>
      [
        ...pack.concepts.map((concept) => `${pack.namespace}:${concept.id}`),
        ...pack.quantityKinds.map(
          (quantity) => `${pack.namespace}:${quantity.id}`,
        ),
      ],
    ),
  );
  const units = new Map<string, PackUnit>(
    packs.flatMap((pack) =>
      pack.units.map((unit) => [`${pack.namespace}:${unit.id}`, unit] as const),
    ),
  );
  for (const [packIndex, pack] of packs.entries()) {
    const dependencies = catalogDependencyClosure(pack, packById);
    const allowedNamespaces = new Set([
      pack.namespace,
      ...dependencies.map((dependency) => dependency.namespace),
    ]);
    const providedCapabilities = new Set(
      dependencies.flatMap((dependency) => dependency.capabilities.provides),
    );
    const missingCapability = pack.capabilities.requires.find(
      (capability) => !providedCapabilities.has(capability),
    );
    if (missingCapability) {
      return invalidCatalog(
        `packs[${packIndex}].capabilities.requires`,
        `required capability ${missingCapability} is not provided by a dependency`,
      );
    }
    for (const [conceptIndex, concept] of pack.concepts.entries()) {
      for (const parent of concept.parents) {
        const error = catalogConceptError(parent, concepts, allowedNamespaces);
        if (error) {
          return invalidCatalog(
            `packs[${packIndex}].concepts[${conceptIndex}].parents`,
            error,
          );
        }
      }
    }
    for (const [quantityIndex, quantity] of pack.quantityKinds.entries()) {
      if (!quantity.defaultUnit) continue;
      const unit = units.get(quantity.defaultUnit);
      if (!unit) {
        return invalidCatalog(
          `packs[${packIndex}].quantityKinds[${quantityIndex}].defaultUnit`,
          `unknown unit ${quantity.defaultUnit}`,
        );
      }
      const namespaceError = catalogNamespaceError(
        quantity.defaultUnit,
        allowedNamespaces,
      );
      if (namespaceError) {
        return invalidCatalog(
          `packs[${packIndex}].quantityKinds[${quantityIndex}].defaultUnit`,
          namespaceError,
        );
      }
      if (!sameDimension(quantity.dimension, unit.dimension)) {
        return invalidCatalog(
          `packs[${packIndex}].quantityKinds[${quantityIndex}].defaultUnit`,
          `unit ${quantity.defaultUnit} has an incompatible dimension`,
        );
      }
    }
    for (const [lawIndex, law] of pack.laws.entries()) {
      for (const [roleIndex, role] of law.roles.entries()) {
        const error = catalogConceptError(role.concept, concepts, allowedNamespaces);
        if (error) {
          return invalidCatalog(
            `packs[${packIndex}].laws[${lawIndex}].roles[${roleIndex}].concept`,
            error,
          );
        }
      }
    }
    for (const [patternIndex, pattern] of pack.patterns.entries()) {
      const conceptLists = [
        ...pattern.parameters.map((parameter, parameterIndex) => ({
          concepts: parameter.constraint.concepts ?? [],
          path: `packs[${packIndex}].patterns[${patternIndex}].parameters[${parameterIndex}].constraint.concepts`,
        })),
        {
          concepts: pattern.result.concepts ?? [],
          path: `packs[${packIndex}].patterns[${patternIndex}].result.concepts`,
        },
      ];
      for (const { concepts: referencedConcepts, path } of conceptLists) {
        for (const concept of referencedConcepts) {
          const error = catalogConceptError(concept, concepts, allowedNamespaces);
          if (error) return invalidCatalog(path, error);
        }
      }
    }
  }

  return { ok: true, packs };
}

function catalogDependencyClosure(
  pack: DomainPack,
  packById: ReadonlyMap<string, DomainPack>,
): DomainPack[] {
  const result: DomainPack[] = [];
  const pending = [...pack.dependencies];
  const seen = new Set<string>();
  while (pending.length > 0) {
    const dependency = pending.pop();
    if (!dependency || seen.has(dependency.packId)) continue;
    seen.add(dependency.packId);
    const dependencyPack = packById.get(dependency.packId);
    if (!dependencyPack) continue;
    pending.push(...dependencyPack.dependencies);
    result.push(dependencyPack);
  }
  return result;
}

function catalogConceptError(
  concept: string,
  concepts: ReadonlySet<string>,
  allowedNamespaces: ReadonlySet<string>,
): string | undefined {
  if (!concepts.has(concept)) return `unknown concept ${concept}`;
  return catalogNamespaceError(concept, allowedNamespaces);
}

function catalogNamespaceError(
  qualifiedId: string,
  allowedNamespaces: ReadonlySet<string>,
): string | undefined {
  const namespace = qualifiedId.split(":", 1)[0];
  return namespace && allowedNamespaces.has(namespace)
    ? undefined
    : `${qualifiedId} belongs to an undeclared dependency`;
}

function sameDimension(
  left: readonly PackDimensionExponent[],
  right: readonly PackDimensionExponent[],
): boolean {
  if (left.length !== right.length) return false;
  const exponent = (value: PackDimensionExponent) =>
    `${value.base}:${value.numerator}/${value.denominator}`;
  return [...left].map(exponent).sort().join("|") ===
    [...right].map(exponent).sort().join("|");
}

function invalid(path: string, message: string): PackValidationResult {
  return { errors: [{ message, path }], ok: false };
}

function invalidCatalog(path: string, message: string): PackCatalogValidationResult {
  return { errors: [{ message, path }], ok: false };
}

function majorVersion(version: string): number | undefined {
  const first = version.split(".")[0];
  if (first === undefined || first === "") return undefined;
  const parsed = Number(first);
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : undefined;
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

function qualifiedIdentifier(value: unknown): value is string {
  return typeof value === "string" && QUALIFIED_ID.test(value);
}

function qualifiedStrings(value: unknown): readonly string[] | undefined {
  const values = strings(value);
  if (!values || values.some((entry) => !qualifiedIdentifier(entry))) return undefined;
  return new Set(values).size === values.length ? values : undefined;
}

function validDimension(value: unknown): boolean {
  const entries = records(value);
  if (!entries) return false;
  const bases = new Set<string>();
  return entries.every((entry) => {
    if (!identifier(entry.base) || bases.has(entry.base)) return false;
    bases.add(entry.base);
    return (
      Number.isInteger(entry.numerator) &&
      Number(entry.numerator) !== 0 &&
      Number.isInteger(entry.denominator) &&
      Number(entry.denominator) > 0
    );
  });
}

function validRational(value: unknown, allowZero: boolean): boolean {
  const rational = record(value);
  return Boolean(
    rational &&
      Number.isInteger(rational.numerator) &&
      (allowZero || Number(rational.numerator) !== 0) &&
      Number.isInteger(rational.denominator) &&
      Number(rational.denominator) > 0,
  );
}

function validFormulaConstraint(value: unknown): boolean {
  const constraint = record(value);
  if (!constraint || typeof constraint.kind !== "string") return false;
  const kinds = new Set([
    "distribution",
    "event",
    "expression",
    "function",
    "graph",
    "index",
    "matrix",
    "proposition",
    "random-variable",
    "scalar",
    "set",
    "tensor",
    "vector",
  ]);
  if (!kinds.has(constraint.kind)) return false;
  if (constraint.concepts !== undefined && !qualifiedStrings(constraint.concepts)) return false;
  return (
    (constraint.dimensions === undefined || Boolean(strings(constraint.dimensions))) &&
    (constraint.refinements === undefined || Boolean(strings(constraint.refinements)))
  );
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
