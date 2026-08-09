export type SuiteTier = "evaluated" | "probe";
export type CapabilityMaturity = "evaluated" | "probe" | "unsupported";
export const CAPABILITY_IDS = [
  "concept-vocabulary",
  "declarations-roles",
  "shape-quantity-unit",
  "law-recognition",
  "diagnostics-refusal",
  "project-macro",
  "navigation-explanation",
] as const;
export type CapabilityId = (typeof CAPABILITY_IDS)[number];
export type CorpusExpectation = "established" | "refused";
export type MetamorphicTransform =
  | "document-order"
  | "neutral-prose"
  | "trailing-comment";

export interface QualityThresholds {
  evidenceIntegrity: number;
  lawPrecision: number;
  lawRecall: number;
  refusalPreservation: number;
  roleAccuracy: number;
}

export interface CoverageDimension {
  id: string;
  tags: readonly string[];
}

export const DIVERSITY_FACETS = [
  "semanticSkeleton",
  "syntaxStructure",
  "proseFamily",
  "projectTopology",
  "mutationFamily",
] as const;

export type DiversityFacet = (typeof DIVERSITY_FACETS)[number];

export interface DiversityRequirements {
  maximumProfileShare: number;
  minimumDistinct: Readonly<Record<DiversityFacet, number>>;
}

export interface LawCorpusSuiteConfig {
  id: string;
  kind: "law";
  minimumPositiveCasesPerLaw: number;
  minimumRefusalCasesPerLaw: number;
  packId: string;
  path: string;
  requiredDimensions: readonly string[];
  requiredDiversity: DiversityRequirements;
  tier: SuiteTier;
}

export interface GlobalRefusalSuiteConfig {
  id: string;
  kind: "global-refusal";
  minimumCases: number;
  path: string;
  requiredDimensions: readonly string[];
  requiredDiversity: DiversityRequirements;
}

export type CorpusSuiteConfig =
  | LawCorpusSuiteConfig
  | GlobalRefusalSuiteConfig;

export interface FoundationSuiteConfig {
  capability: "scientific-kernel" | "shape-quantity-unit";
  id: string;
  minimumCases: number;
  packId: string;
  path: string;
  requiredDimensions: readonly string[];
  tier: SuiteTier;
}

export interface CapabilitySupport {
  maturity: CapabilityMaturity;
  suiteIds: readonly string[];
}

export interface PackSupport {
  capabilities: Readonly<Record<CapabilityId, CapabilitySupport>>;
  packId: string;
}

export interface QualityManifest {
  dimensions: readonly CoverageDimension[];
  foundationSuites: readonly FoundationSuiteConfig[];
  metamorphic: {
    casesPerLaw: number;
    transforms: readonly MetamorphicTransform[];
  };
  packs: readonly PackSupport[];
  schemaVersion: 3;
  suites: readonly CorpusSuiteConfig[];
  thresholds: QualityThresholds;
}

export interface CorpusDocument {
  content: string;
  fileId: string;
  path: string;
}

export interface CorpusMacro {
  definition: string;
  name: string;
  parameterCount?: number;
}

export interface DiversityProfile {
  batch: string;
  mutationFamily: string;
  projectTopology: string;
  proseFamily: string;
  semanticSkeleton: string;
  syntaxStructure: string;
}

interface CorpusCaseBase {
  cursor: {
    edge?: "after" | "before";
    fileId: string;
    needle: string;
  };
  documents: readonly CorpusDocument[];
  diversity: DiversityProfile;
  expectation: CorpusExpectation;
  id: string;
  macros?: readonly CorpusMacro[];
  mainFileId?: string;
  variationTags: readonly string[];
}

export interface EstablishedCorpusCase extends CorpusCaseBase {
  expectation: "established";
  expectedRoles: Readonly<Record<string, string>>;
  lawId: string;
}

export interface LawRefusalCorpusCase extends CorpusCaseBase {
  expectation: "refused";
  lawId: string;
  refusalCategory: string;
}

export interface GlobalRefusalCorpusCase extends CorpusCaseBase {
  expectation: "refused";
  refusalCategory: string;
}

export type CorpusCase =
  | EstablishedCorpusCase
  | LawRefusalCorpusCase
  | GlobalRefusalCorpusCase;

export interface Corpus {
  cases: readonly CorpusCase[];
  domain: string;
  schemaVersion: 2;
}

export function parseQualityManifest(value: unknown): QualityManifest {
  const root = object(value, "manifest");
  exactKeys(
    root,
    [
      "schemaVersion",
      "thresholds",
      "dimensions",
      "metamorphic",
      "packs",
      "suites",
      "foundationSuites",
    ],
    "manifest",
  );
  if (integer(root.schemaVersion, "manifest.schemaVersion") !== 3) {
    fail("manifest.schemaVersion", "must be 3");
  }
  const thresholds = parseThresholds(root.thresholds);
  const dimensions = array(root.dimensions, "manifest.dimensions").map(
    (item, index) => parseDimension(item, `manifest.dimensions[${index}]`),
  );
  unique(dimensions.map((item) => item.id), "manifest.dimensions");
  const dimensionIds = new Set(dimensions.map((item) => item.id));
  const metamorphic = parseMetamorphic(root.metamorphic);
  const packs = array(root.packs, "manifest.packs").map((item, index) =>
    parsePackSupport(item, `manifest.packs[${index}]`),
  );
  unique(packs.map((item) => item.packId), "manifest.packs");
  const suites = array(root.suites, "manifest.suites").map((item, index) =>
    parseSuite(item, `manifest.suites[${index}]`, dimensionIds),
  );
  unique(suites.map((item) => item.id), "manifest.suites");
  unique(suites.map((item) => item.path), "manifest.suites paths");
  const foundationSuites = array(
    root.foundationSuites,
    "manifest.foundationSuites",
  ).map((item, index) =>
    parseFoundationSuite(item, `manifest.foundationSuites[${index}]`, dimensionIds),
  );
  unique(foundationSuites.map((item) => item.id), "manifest.foundationSuites");
  unique(foundationSuites.map((item) => item.path), "manifest.foundationSuites paths");
  const suiteIds = new Set([
    ...suites.map((item) => item.id),
    ...foundationSuites.map((item) => item.id),
  ]);
  if (suiteIds.size !== suites.length + foundationSuites.length) {
    fail("manifest suites", "suite ids must be globally unique");
  }
  for (const [index, pack] of packs.entries()) {
    for (const capability of CAPABILITY_IDS) {
      for (const suiteId of pack.capabilities[capability].suiteIds) {
        if (!suiteIds.has(suiteId)) {
          fail(
            `manifest.packs[${index}].capabilities.${capability}.suiteIds`,
            `unknown suite ${suiteId}`,
          );
        }
      }
    }
  }
  return {
    dimensions,
    foundationSuites,
    metamorphic,
    packs,
    schemaVersion: 3,
    suites,
    thresholds,
  };
}

export function parseCorpus(value: unknown, suite: CorpusSuiteConfig): Corpus {
  const root = object(value, `corpus ${suite.id}`);
  exactKeys(root, ["schemaVersion", "domain", "cases"], `corpus ${suite.id}`);
  if (integer(root.schemaVersion, `${suite.id}.schemaVersion`) !== 2) {
    fail(`${suite.id}.schemaVersion`, "must be 2");
  }
  const domain = text(root.domain, `${suite.id}.domain`);
  if (domain !== suite.id) {
    fail(`${suite.id}.domain`, `must equal suite id ${suite.id}`);
  }
  const cases = array(root.cases, `${suite.id}.cases`).map((item, index) =>
    parseCase(item, `${suite.id}.cases[${index}]`, suite),
  );
  if (cases.length === 0) fail(`${suite.id}.cases`, "must not be empty");
  unique(cases.map((item) => item.id), `${suite.id}.cases`);
  return { cases, domain, schemaVersion: 2 };
}

function parseThresholds(value: unknown): QualityThresholds {
  const item = object(value, "manifest.thresholds");
  const keys = [
    "evidenceIntegrity",
    "lawPrecision",
    "lawRecall",
    "refusalPreservation",
    "roleAccuracy",
  ] as const;
  exactKeys(item, [...keys], "manifest.thresholds");
  const threshold = (key: (typeof keys)[number]): number => {
    const result = number(item[key], `manifest.thresholds.${key}`);
    if (result < 0 || result > 100) {
      fail(`manifest.thresholds.${key}`, "must be between 0 and 100");
    }
    return result;
  };
  return {
    evidenceIntegrity: threshold("evidenceIntegrity"),
    lawPrecision: threshold("lawPrecision"),
    lawRecall: threshold("lawRecall"),
    refusalPreservation: threshold("refusalPreservation"),
    roleAccuracy: threshold("roleAccuracy"),
  };
}

function parseDimension(value: unknown, path: string): CoverageDimension {
  const item = object(value, path);
  exactKeys(item, ["id", "tags"], path);
  const tags = strings(item.tags, `${path}.tags`);
  if (tags.length === 0) fail(`${path}.tags`, "must not be empty");
  unique(tags, `${path}.tags`);
  return { id: identifier(item.id, `${path}.id`), tags };
}

function parseMetamorphic(value: unknown): QualityManifest["metamorphic"] {
  const item = object(value, "manifest.metamorphic");
  exactKeys(item, ["casesPerLaw", "transforms"], "manifest.metamorphic");
  const casesPerLaw = positiveInteger(
    item.casesPerLaw,
    "manifest.metamorphic.casesPerLaw",
  );
  const allowed = new Set<MetamorphicTransform>([
    "document-order",
    "neutral-prose",
    "trailing-comment",
  ]);
  const transforms = strings(
    item.transforms,
    "manifest.metamorphic.transforms",
  ).map((transform) => {
    if (!allowed.has(transform as MetamorphicTransform)) {
      fail("manifest.metamorphic.transforms", `unknown transform ${transform}`);
    }
    return transform as MetamorphicTransform;
  });
  unique(transforms, "manifest.metamorphic.transforms");
  if (transforms.length === 0) {
    fail("manifest.metamorphic.transforms", "must not be empty");
  }
  return { casesPerLaw, transforms };
}

function parsePackSupport(value: unknown, path: string): PackSupport {
  const item = object(value, path);
  exactKeys(item, ["packId", "capabilities"], path);
  const capabilityValues = object(item.capabilities, `${path}.capabilities`);
  exactKeys(capabilityValues, CAPABILITY_IDS, `${path}.capabilities`);
  const capabilities = Object.fromEntries(
    CAPABILITY_IDS.map((capability) => {
      const capabilityPath = `${path}.capabilities.${capability}`;
      const entry = object(capabilityValues[capability], capabilityPath);
      exactKeys(entry, ["maturity", "suiteIds"], capabilityPath);
      const maturity = oneOf(
        entry.maturity,
        ["evaluated", "probe", "unsupported"],
        `${capabilityPath}.maturity`,
      );
      const suiteIds = strings(entry.suiteIds, `${capabilityPath}.suiteIds`);
      unique(suiteIds, `${capabilityPath}.suiteIds`);
      if (maturity === "unsupported" && suiteIds.length) {
        fail(`${capabilityPath}.suiteIds`, "unsupported capabilities cannot cite suites");
      }
      if (
        maturity !== "unsupported" &&
        capability !== "concept-vocabulary" &&
        suiteIds.length === 0
      ) {
        fail(`${capabilityPath}.suiteIds`, `${maturity} capability requires evidence`);
      }
      return [capability, { maturity, suiteIds }];
    }),
  ) as unknown as Readonly<Record<CapabilityId, CapabilitySupport>>;
  return {
    capabilities,
    packId: identifier(item.packId, `${path}.packId`),
  };
}

function parseFoundationSuite(
  value: unknown,
  path: string,
  dimensionIds: ReadonlySet<string>,
): FoundationSuiteConfig {
  const item = object(value, path);
  exactKeys(
    item,
    [
      "id",
      "packId",
      "capability",
      "path",
      "tier",
      "minimumCases",
      "requiredDimensions",
    ],
    path,
  );
  const corpusPath = text(item.path, `${path}.path`);
  if (
    corpusPath.startsWith("/") ||
    corpusPath.split("/").includes("..") ||
    !corpusPath.endsWith(".json")
  ) {
    fail(`${path}.path`, "must be a safe relative JSON path");
  }
  const requiredDimensions = strings(
    item.requiredDimensions,
    `${path}.requiredDimensions`,
  );
  unique(requiredDimensions, `${path}.requiredDimensions`);
  for (const dimension of requiredDimensions) {
    if (!dimensionIds.has(dimension)) {
      fail(`${path}.requiredDimensions`, `unknown dimension ${dimension}`);
    }
  }
  return {
    capability: oneOf(
      item.capability,
      ["scientific-kernel", "shape-quantity-unit"],
      `${path}.capability`,
    ),
    id: identifier(item.id, `${path}.id`),
    minimumCases: positiveInteger(item.minimumCases, `${path}.minimumCases`),
    packId: identifier(item.packId, `${path}.packId`),
    path: corpusPath,
    requiredDimensions,
    tier: oneOf(item.tier, ["evaluated", "probe"], `${path}.tier`),
  };
}

function parseSuite(
  value: unknown,
  path: string,
  dimensionIds: ReadonlySet<string>,
): CorpusSuiteConfig {
  const item = object(value, path);
  const kind = oneOf(item.kind, ["law", "global-refusal"], `${path}.kind`);
  const commonKeys = [
    "id",
    "kind",
    "path",
    "requiredDimensions",
    "requiredDiversity",
  ];
  exactKeys(
    item,
    kind === "law"
      ? [
          ...commonKeys,
          "packId",
          "tier",
          "minimumPositiveCasesPerLaw",
          "minimumRefusalCasesPerLaw",
        ]
      : [...commonKeys, "minimumCases"],
    path,
  );
  const corpusPath = text(item.path, `${path}.path`);
  if (
    corpusPath.startsWith("/") ||
    corpusPath.split("/").includes("..") ||
    !corpusPath.endsWith(".json")
  ) {
    fail(`${path}.path`, "must be a safe relative JSON path");
  }
  const requiredDimensions = strings(
    item.requiredDimensions,
    `${path}.requiredDimensions`,
  );
  unique(requiredDimensions, `${path}.requiredDimensions`);
  for (const dimension of requiredDimensions) {
    if (!dimensionIds.has(dimension)) {
      fail(`${path}.requiredDimensions`, `unknown dimension ${dimension}`);
    }
  }
  const common = {
    id: identifier(item.id, `${path}.id`),
    kind,
    path: corpusPath,
    requiredDimensions,
    requiredDiversity: parseDiversityRequirements(
      item.requiredDiversity,
      `${path}.requiredDiversity`,
    ),
  };
  if (kind === "global-refusal") {
    return {
      ...common,
      kind,
      minimumCases: positiveInteger(item.minimumCases, `${path}.minimumCases`),
    };
  }
  return {
    ...common,
    kind,
    minimumPositiveCasesPerLaw: positiveInteger(
      item.minimumPositiveCasesPerLaw,
      `${path}.minimumPositiveCasesPerLaw`,
    ),
    minimumRefusalCasesPerLaw: positiveInteger(
      item.minimumRefusalCasesPerLaw,
      `${path}.minimumRefusalCasesPerLaw`,
    ),
    packId: identifier(item.packId, `${path}.packId`),
    tier: oneOf(item.tier, ["evaluated", "probe"], `${path}.tier`),
  };
}

function parseDiversityRequirements(
  value: unknown,
  path: string,
): DiversityRequirements {
  const item = object(value, path);
  exactKeys(item, ["minimumDistinct", "maximumProfileShare"], path);
  const minimum = object(item.minimumDistinct, `${path}.minimumDistinct`);
  exactKeys(minimum, DIVERSITY_FACETS, `${path}.minimumDistinct`);
  const minimumDistinct = Object.fromEntries(
    DIVERSITY_FACETS.map((facet) => [
      facet,
      positiveInteger(minimum[facet], `${path}.minimumDistinct.${facet}`),
    ]),
  ) as unknown as Readonly<Record<DiversityFacet, number>>;
  const maximumProfileShare = number(
    item.maximumProfileShare,
    `${path}.maximumProfileShare`,
  );
  if (maximumProfileShare <= 0 || maximumProfileShare > 1) {
    fail(`${path}.maximumProfileShare`, "must be greater than 0 and at most 1");
  }
  return { maximumProfileShare, minimumDistinct };
}

function parseCase(
  value: unknown,
  path: string,
  suite: CorpusSuiteConfig,
): CorpusCase {
  const item = object(value, path);
  exactKeys(
    item,
    [
      "id",
      "lawId",
      "expectation",
      "documents",
      "diversity",
      "mainFileId",
      "cursor",
      "expectedRoles",
      "macros",
      "refusalCategory",
      "variationTags",
    ],
    path,
  );
  const expectation = oneOf(
    item.expectation,
    ["established", "refused"],
    `${path}.expectation`,
  );
  if (suite.kind === "global-refusal" && expectation !== "refused") {
    fail(`${path}.expectation`, "global-refusal suites only accept refused cases");
  }
  const documents = array(item.documents, `${path}.documents`).map(
    (document, index) => parseDocument(document, `${path}.documents[${index}]`),
  );
  if (documents.length === 0) fail(`${path}.documents`, "must not be empty");
  unique(documents.map((document) => document.fileId), `${path}.documents fileId`);
  unique(documents.map((document) => document.path), `${path}.documents path`);
  const cursor = parseCursor(item.cursor, `${path}.cursor`);
  const cursorDocument = documents.find(
    (document) => document.fileId === cursor.fileId,
  );
  if (!cursorDocument) {
    fail(`${path}.cursor.fileId`, `unknown document ${cursor.fileId}`);
  }
  const occurrences = cursorDocument.content.split(cursor.needle).length - 1;
  if (occurrences !== 1) {
    fail(`${path}.cursor.needle`, `must occur exactly once; found ${occurrences}`);
  }
  const expectedRoles = optionalStringRecord(item.expectedRoles, `${path}.expectedRoles`);
  const refusalCategory = optionalText(item.refusalCategory, `${path}.refusalCategory`);
  if (expectation === "established" && !expectedRoles) {
    fail(`${path}.expectedRoles`, "established cases require expected roles");
  }
  if (expectation === "established" && refusalCategory) {
    fail(`${path}.refusalCategory`, "established cases cannot have a refusal category");
  }
  if (expectation === "refused" && !refusalCategory) {
    fail(`${path}.refusalCategory`, "refused cases require a category");
  }
  if (expectation === "refused" && expectedRoles) {
    fail(`${path}.expectedRoles`, "refused cases cannot declare expected roles");
  }
  const macros = optionalArray(item.macros, `${path}.macros`)?.map((macro, index) =>
    parseMacro(macro, `${path}.macros[${index}]`),
  );
  if (macros) unique(macros.map((macro) => macro.name), `${path}.macros`);
  const variationTags = strings(item.variationTags, `${path}.variationTags`);
  if (variationTags.length === 0) fail(`${path}.variationTags`, "must not be empty");
  unique(variationTags, `${path}.variationTags`);
  const common = {
    cursor,
    documents,
    diversity: parseDiversityProfile(item.diversity, `${path}.diversity`),
    expectation,
    id: identifier(item.id, `${path}.id`),
    ...(macros?.length ? { macros } : {}),
    ...(item.mainFileId === undefined
      ? {}
      : {
          mainFileId: documentId(
            item.mainFileId,
            documents,
            `${path}.mainFileId`,
          ),
        }),
    variationTags,
  };
  if (suite.kind === "global-refusal") {
    if (item.lawId !== undefined) {
      fail(`${path}.lawId`, "global-refusal cases must not target a law");
    }
    return { ...common, expectation: "refused", refusalCategory: refusalCategory! };
  }
  const lawId = identifier(item.lawId, `${path}.lawId`);
  return expectation === "established"
    ? { ...common, expectation, expectedRoles: expectedRoles!, lawId }
    : { ...common, expectation, lawId, refusalCategory: refusalCategory! };
}

function parseDiversityProfile(value: unknown, path: string): DiversityProfile {
  const item = object(value, path);
  exactKeys(item, ["batch", ...DIVERSITY_FACETS], path);
  return {
    batch: identifier(item.batch, `${path}.batch`),
    mutationFamily: identifier(item.mutationFamily, `${path}.mutationFamily`),
    projectTopology: identifier(item.projectTopology, `${path}.projectTopology`),
    proseFamily: identifier(item.proseFamily, `${path}.proseFamily`),
    semanticSkeleton: identifier(item.semanticSkeleton, `${path}.semanticSkeleton`),
    syntaxStructure: identifier(item.syntaxStructure, `${path}.syntaxStructure`),
  };
}

function parseDocument(value: unknown, path: string): CorpusDocument {
  const item = object(value, path);
  exactKeys(item, ["content", "fileId", "path"], path);
  const documentPath = text(item.path, `${path}.path`);
  if (documentPath.startsWith("/") || documentPath.split("/").includes("..")) {
    fail(`${path}.path`, "must be a safe relative path");
  }
  return {
    content: string(item.content, `${path}.content`),
    fileId: text(item.fileId, `${path}.fileId`),
    path: documentPath,
  };
}

function parseCursor(value: unknown, path: string): CorpusCase["cursor"] {
  const item = object(value, path);
  exactKeys(item, ["fileId", "needle", "edge"], path);
  const edge =
    item.edge === undefined
      ? undefined
      : oneOf(item.edge, ["after", "before"], `${path}.edge`);
  return {
    ...(edge ? { edge } : {}),
    fileId: text(item.fileId, `${path}.fileId`),
    needle: text(item.needle, `${path}.needle`),
  };
}

function parseMacro(value: unknown, path: string): CorpusMacro {
  const item = object(value, path);
  exactKeys(item, ["name", "definition", "parameterCount"], path);
  const name = text(item.name, `${path}.name`);
  if (!/^\\[A-Za-z@]+$/u.test(name)) {
    fail(`${path}.name`, "must be a control-sequence name");
  }
  const parameterCount =
    item.parameterCount === undefined
      ? undefined
      : integer(item.parameterCount, `${path}.parameterCount`);
  if (parameterCount !== undefined && (parameterCount < 0 || parameterCount > 9)) {
    fail(`${path}.parameterCount`, "must be between 0 and 9");
  }
  return {
    definition: string(item.definition, `${path}.definition`),
    name,
    ...(parameterCount === undefined ? {} : { parameterCount }),
  };
}

function documentId(
  value: unknown,
  documents: readonly CorpusDocument[],
  path: string,
): string {
  const result = text(value, path);
  if (!documents.some((document) => document.fileId === result)) {
    fail(path, `unknown document ${result}`);
  }
  return result;
}

function object(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(path, "must be an object");
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) fail(path, "must be an array");
  return value;
}

function optionalArray(value: unknown, path: string): unknown[] | undefined {
  return value === undefined ? undefined : array(value, path);
}

function strings(value: unknown, path: string): string[] {
  return array(value, path).map((item, index) => text(item, `${path}[${index}]`));
}

function string(value: unknown, path: string): string {
  if (typeof value !== "string") fail(path, "must be a string");
  return value;
}

function text(value: unknown, path: string): string {
  const result = string(value, path);
  if (!result.trim()) fail(path, "must not be empty");
  return result;
}

function optionalString(value: unknown, path: string): string | undefined {
  return value === undefined ? undefined : string(value, path);
}

function optionalText(value: unknown, path: string): string | undefined {
  return value === undefined ? undefined : text(value, path);
}

function number(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    fail(path, "must be a finite number");
  }
  return value;
}

function integer(value: unknown, path: string): number {
  const result = number(value, path);
  if (!Number.isInteger(result)) fail(path, "must be an integer");
  return result;
}

function positiveInteger(value: unknown, path: string): number {
  const result = integer(value, path);
  if (result < 1) fail(path, "must be positive");
  return result;
}

function identifier(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u.test(result)) {
    fail(path, "must be a lowercase kebab-case identifier");
  }
  return result;
}

function oneOf<const Values extends readonly string[]>(
  value: unknown,
  values: Values,
  path: string,
): Values[number] {
  if (typeof value !== "string" || !values.includes(value)) {
    fail(path, `must be one of ${values.join(", ")}`);
  }
  return value as Values[number];
}

function optionalStringRecord(
  value: unknown,
  path: string,
): Record<string, string> | undefined {
  if (value === undefined) return undefined;
  const item = object(value, path);
  const result: Record<string, string> = {};
  for (const [key, entry] of Object.entries(item)) {
    if (!key.trim()) fail(path, "role keys must not be empty");
    result[key] = text(entry, `${path}.${key}`);
  }
  if (Object.keys(result).length === 0) fail(path, "must not be empty");
  return result;
}

function exactKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
  path: string,
): void {
  const allowedKeys = new Set(allowed);
  const unknown = Object.keys(value).filter((key) => !allowedKeys.has(key));
  if (unknown.length) fail(path, `unknown fields: ${unknown.sort().join(", ")}`);
}

function unique(values: readonly string[], path: string): void {
  const seen = new Set<string>();
  for (const value of values) {
    if (seen.has(value)) fail(path, `duplicate value ${value}`);
    seen.add(value);
  }
}

function fail(path: string, message: string): never {
  throw new Error(`${path}: ${message}`);
}
