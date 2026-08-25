import { createHash } from "node:crypto";
import type { CorpusDocument } from "./model";

export const CHALLENGE_RECOGNITION_V2_SHA256 =
  "b9a5691048b3292afae8d0c2296b6707f1c8b64f4223b9fc726cb3987808d4d7";
export const CHALLENGE_RECOGNITION_V3_SHA256 =
  "14b4bb40905567bc8f277fc117561a82e1aeb74e5002f03cd892b0845afda8f2";

export const CHALLENGE_LAYERS = [
  "binding",
  "constraint",
  "pack",
  "presentation",
  "resolution",
  "syntax",
] as const;
export const CHALLENGE_METRICS = [
  "association",
  "constraint",
  "evidence",
  "navigation",
  "recognition",
  "refusal",
  "scope",
  "structure",
] as const;
export const CHALLENGE_DECISIONS = [
  "established",
  "partial",
  "ambiguous",
  "conflicting",
  "unsupported",
] as const;
export const CHALLENGE_DOCUMENT_SHAPES = [
  "distant-prose",
  "macro-neighbor",
  "malformed-neighbor",
  "multi-equation",
  "project-neighbor",
  "sectioned",
] as const;
export const CHALLENGE_PROBLEM_POLICIES = ["none", "source-conflict"] as const;
export const CHALLENGE_DECISION_DOMAINS = [
  "cursor-entity",
  "selected-formula",
] as const;
export const CHALLENGE_RELATION_AUTHORITIES = [
  "authoritative",
  "candidate",
] as const;
export const CHALLENGE_INTERPRETATION_SUPPORT = [
  "explicit",
  "derived",
  "supported",
  "tentative",
  "contradicted",
] as const;

export type ChallengeLayer = (typeof CHALLENGE_LAYERS)[number];
export type ChallengeMetric = (typeof CHALLENGE_METRICS)[number];
export type ChallengeOutcome = "positive" | "refusal";
export type ChallengeDecision = (typeof CHALLENGE_DECISIONS)[number];
export type ChallengeDocumentShape = (typeof CHALLENGE_DOCUMENT_SHAPES)[number];
export type ChallengeProblemPolicy =
  (typeof CHALLENGE_PROBLEM_POLICIES)[number];
export type ChallengeDecisionDomain =
  (typeof CHALLENGE_DECISION_DOMAINS)[number];
export type ChallengeRelationAuthority =
  (typeof CHALLENGE_RELATION_AUTHORITIES)[number];
export type ChallengeInterpretationSupport =
  (typeof CHALLENGE_INTERPRETATION_SUPPORT)[number];

export interface ChallengeRecognizedRelation {
  readonly authority: ChallengeRelationAuthority;
  readonly formulaAnchor: "selected-formula";
  readonly relationId: string;
  readonly support: ChallengeInterpretationSupport;
}

export interface ChallengeDecisionExpectation {
  readonly meaning: "absent" | "present";
  readonly problems: ChallengeProblemPolicy;
  readonly status: ChallengeDecision;
}

export interface ChallengeExpectation {
  readonly assumptionValue?: string;
  readonly candidateFamily?: string;
  readonly candidateInterpretation?: string;
  readonly conceptId?: string;
  readonly definitionDescription?: string;
  readonly definitionRuleId?: string;
  readonly excludedConceptId?: string;
  readonly excludedCandidateFamily?: string;
  readonly excludedDefinitionSymbol?: string;
  readonly excludedRelationId?: string;
  readonly relationId?: string;
  readonly shape?: string;
  readonly sourceNotation?: string;
  readonly status?: string;
  readonly symbol?: string;
}

export interface ChallengeCase {
  readonly cursor: {
    readonly edge?: "after" | "before";
    readonly fileId: string;
    readonly needle: string;
  };
  readonly documents: readonly CorpusDocument[];
  readonly decisionExpectation?: ChallengeDecisionExpectation;
  readonly decisionDomain?: ChallengeDecisionDomain;
  readonly documentShape?: ChallengeDocumentShape;
  readonly expectation: ChallengeExpectation;
  readonly id: string;
  readonly metric: ChallengeMetric;
  readonly outcome: ChallengeOutcome;
  readonly owner: ChallengeLayer;
  readonly recognizedRelations?: readonly ChallengeRecognizedRelation[];
  readonly variationTags: readonly string[];
}

export interface ChallengeCorpus {
  readonly cases: readonly ChallengeCase[];
  readonly schemaVersion: 2 | 3 | 4;
}

export interface ChallengeDecisionObservation {
  readonly meaningLabel?: string;
  readonly meaningRelationId?: string | null;
  readonly problemCount: number;
  readonly reasonKinds: readonly string[];
  readonly sourceGrounded: boolean;
  readonly status?: string;
}

export interface ChallengeObservation {
  readonly assumptionValues: readonly string[];
  readonly candidates: readonly {
    readonly family: string;
    readonly interpretation: string;
  }[];
  readonly caseId: string;
  readonly conceptIds: readonly string[];
  readonly definitions: readonly {
    readonly description: string;
    readonly ruleId: string;
    readonly symbol: string;
  }[];
  readonly entityDecision?: ChallengeDecisionObservation;
  readonly formulaDecision?: ChallengeDecisionObservation;
  readonly meaningLabel?: string;
  readonly meaningRelationId?: string | null;
  readonly problemCount: number;
  readonly reasonKinds: readonly string[];
  readonly recognizedRelations?: readonly ChallengeRecognizedRelation[];
  readonly relationIds: readonly string[];
  readonly shapes: readonly string[];
  readonly sourceNotation?: string;
  readonly sourceGrounded: boolean;
  readonly status?: string;
  readonly symbols: readonly string[];
}

export interface ChallengeScorecard {
  readonly cases: number;
  readonly failures: readonly string[];
  readonly decisions: Readonly<
    Record<ChallengeDecision, { passed: number; total: number }>
  >;
  readonly explanation: { passed: number; total: number };
  readonly layers: Readonly<
    Record<ChallengeLayer, { passed: number; total: number }>
  >;
  readonly metrics: Readonly<
    Record<ChallengeMetric, { passed: number; total: number }>
  >;
  readonly outcomes: Readonly<
    Record<ChallengeOutcome, { passed: number; total: number }>
  >;
  readonly passed: number;
  readonly problemPolicy: Readonly<
    Record<ChallengeProblemPolicy, { passed: number; total: number }>
  >;
  readonly reasonIntegrity: { passed: number; total: number };
  readonly schemaVersion: 2 | 3 | 4;
}

interface ChallengeV3Profile {
  readonly caseId: string;
  readonly decision: ChallengeDecisionExpectation;
  readonly documentShape: ChallengeDocumentShape;
}

interface ChallengeV4Profile {
  readonly caseId: string;
  readonly decision: ChallengeDecisionExpectation;
  readonly decisionDomain: ChallengeDecisionDomain;
  readonly recognizedRelations: readonly ChallengeRecognizedRelation[];
}

export interface DevelopmentFixtureCase {
  readonly documents: readonly CorpusDocument[];
  readonly id: string;
}

export function findChallengeFixtureLeaks(
  challenge: readonly ChallengeCase[],
  development: readonly DevelopmentFixtureCase[],
): readonly string[] {
  const developmentIds = new Set(development.map((item) => item.id));
  const developmentSources = new Set(
    development.flatMap((item) =>
      item.documents.map((document) => normalizedSource(document.content)),
    ),
  );
  const leaks = new Set<string>();
  for (const item of challenge) {
    if (developmentIds.has(item.id))
      leaks.add(`${item.id}: duplicate fixture id`);
    for (const document of item.documents) {
      if (developmentSources.has(normalizedSource(document.content))) {
        leaks.add(`${item.id}: duplicate fixture source`);
      }
    }
  }
  return [...leaks].sort();
}

export function parseChallengeCorpus(value: unknown): ChallengeCorpus {
  const root = record(value, "challenge");
  exact(root, ["schemaVersion", "cases"], "challenge");
  if (root.schemaVersion !== 2)
    throw new Error("challenge.schemaVersion: must be 2");
  if (!Array.isArray(root.cases) || root.cases.length < 48) {
    throw new Error("challenge.cases: must contain at least 48 frozen cases");
  }
  const cases = root.cases.map((item, index) =>
    parseCase(item, `challenge.cases[${index}]`),
  );
  unique(
    cases.map((item) => item.id),
    "challenge.cases.id",
  );
  for (const layer of CHALLENGE_LAYERS) {
    for (const outcome of ["positive", "refusal"] as const) {
      if (
        !cases.some((item) => item.owner === layer && item.outcome === outcome)
      ) {
        throw new Error(`challenge.cases: missing ${layer}/${outcome}`);
      }
    }
  }
  for (const metric of CHALLENGE_METRICS) {
    if (!cases.some((item) => item.metric === metric)) {
      throw new Error(`challenge.cases: missing metric ${metric}`);
    }
  }
  validateBoundaryPairs(cases);
  return { cases, schemaVersion: 2 };
}

/**
 * Composes the frozen v2 semantic boundaries with a strict document-shaped v3
 * profile. The profile contains only independent test policy; it never reads
 * production recognition or presentation code.
 */
export function parseChallengeV3(
  baseValue: unknown,
  profileValue: unknown,
): ChallengeCorpus {
  const base = parseChallengeCorpus(baseValue);
  const root = record(profileValue, "challenge-v3");
  exact(
    root,
    ["schemaVersion", "baseSchemaVersion", "profiles"],
    "challenge-v3",
  );
  if (root.schemaVersion !== 3)
    throw new Error("challenge-v3.schemaVersion: must be 3");
  if (root.baseSchemaVersion !== 2) {
    throw new Error("challenge-v3.baseSchemaVersion: must be 2");
  }
  if (
    !Array.isArray(root.profiles) ||
    root.profiles.length !== base.cases.length
  ) {
    throw new Error(
      `challenge-v3.profiles: must contain exactly ${base.cases.length} profiles`,
    );
  }
  const profiles = root.profiles.map((value, index) =>
    parseV3Profile(value, `challenge-v3.profiles[${index}]`),
  );
  unique(
    profiles.map((profile) => profile.caseId),
    "challenge-v3.profiles.caseId",
  );
  const profileById = new Map(
    profiles.map((profile) => [profile.caseId, profile]),
  );
  const unknown = profiles
    .filter((profile) => !base.cases.some((item) => item.id === profile.caseId))
    .map((profile) => profile.caseId);
  if (unknown.length)
    throw new Error(`challenge-v3.profiles: unknown case ${unknown.sort()[0]}`);

  const cases = base.cases.map((item) => {
    const profile = profileById.get(item.id);
    if (!profile)
      throw new Error(`challenge-v3.profiles: missing case ${item.id}`);
    return shapeChallengeCase(item, profile);
  });
  for (const shape of CHALLENGE_DOCUMENT_SHAPES) {
    if (!cases.some((item) => item.documentShape === shape)) {
      throw new Error(`challenge-v3.profiles: missing document shape ${shape}`);
    }
  }
  for (const status of CHALLENGE_DECISIONS) {
    if (!cases.some((item) => item.decisionExpectation?.status === status)) {
      throw new Error(`challenge-v3.profiles: missing decision ${status}`);
    }
  }
  for (const policy of CHALLENGE_PROBLEM_POLICIES) {
    if (!cases.some((item) => item.decisionExpectation?.problems === policy)) {
      throw new Error(
        `challenge-v3.profiles: missing problem policy ${policy}`,
      );
    }
  }
  return { cases, schemaVersion: 3 };
}

/**
 * Composes the frozen v2 cases and v3 document shapes with a strict v4
 * authority overlay. v4 separates cursor-entity decisions from selected-formula
 * decisions and reviews every formula interpretation without changing either
 * predecessor fixture.
 */
export function parseChallengeV4(
  baseSource: string,
  v3Source: string,
  profileValue: unknown,
): ChallengeCorpus {
  const baseValue = parseJsonSource(baseSource, "challenge-v2 source");
  const parsedBase = parseChallengeCorpus(baseValue);
  if (parsedBase.cases.length !== 48) {
    throw new Error("challenge-v4 base: must contain exactly 48 cases");
  }
  const v3Value = parseJsonSource(v3Source, "challenge-v3 source");
  const base = parseChallengeV3(baseValue, v3Value);
  const root = record(profileValue, "challenge-v4");
  exact(
    root,
    ["schemaVersion", "baseSchemaVersion", "baseDigests", "profiles"],
    "challenge-v4",
  );
  if (root.schemaVersion !== 4)
    throw new Error("challenge-v4.schemaVersion: must be 4");
  if (root.baseSchemaVersion !== 3) {
    throw new Error("challenge-v4.baseSchemaVersion: must be 3");
  }
  const digests = record(root.baseDigests, "challenge-v4.baseDigests");
  exact(
    digests,
    ["recognitionV2Sha256", "recognitionV3Sha256"],
    "challenge-v4.baseDigests",
  );
  validatePinnedDigest(
    digests.recognitionV2Sha256,
    CHALLENGE_RECOGNITION_V2_SHA256,
    sha256(baseSource),
    "challenge-v4.baseDigests.recognitionV2Sha256",
  );
  validatePinnedDigest(
    digests.recognitionV3Sha256,
    CHALLENGE_RECOGNITION_V3_SHA256,
    sha256(v3Source),
    "challenge-v4.baseDigests.recognitionV3Sha256",
  );
  if (
    !Array.isArray(root.profiles) ||
    root.profiles.length !== base.cases.length
  ) {
    throw new Error(
      `challenge-v4.profiles: must contain exactly ${base.cases.length} profiles`,
    );
  }
  const profiles = root.profiles.map((value, index) =>
    parseV4Profile(value, `challenge-v4.profiles[${index}]`),
  );
  unique(
    profiles.map((profile) => profile.caseId),
    "challenge-v4.profiles.caseId",
  );
  const baseIds = new Set(base.cases.map((item) => item.id));
  const unknown = profiles
    .map((profile) => profile.caseId)
    .filter((caseId) => !baseIds.has(caseId))
    .sort();
  if (unknown.length)
    throw new Error(`challenge-v4.profiles: unknown case ${unknown[0]}`);
  const profileById = new Map(
    profiles.map((profile) => [profile.caseId, profile]),
  );
  const cases = base.cases.map((item): ChallengeCase => {
    const profile = profileById.get(item.id);
    if (!profile)
      throw new Error(`challenge-v4.profiles: missing case ${item.id}`);
    const relationId = item.expectation.relationId;
    const reviewedRelation = relationId
      ? profile.recognizedRelations.find(
          (recognized) => recognized.relationId === relationId,
        )
      : undefined;
    if (relationId && !reviewedRelation) {
      throw new Error(
        `challenge-v4.profiles.${item.id}: relation ${relationId} must be reviewed`,
      );
    }
    if (
      (relationId || item.expectation.excludedRelationId) &&
      profile.decisionDomain !== "selected-formula"
    ) {
      throw new Error(
        `challenge-v4.profiles.${item.id}: relation decisions must use selected-formula`,
      );
    }
    const expectation =
      reviewedRelation?.authority === "candidate"
        ? withoutRelationExpectation(item.expectation)
        : item.expectation;
    return {
      ...item,
      decisionDomain: profile.decisionDomain,
      decisionExpectation: profile.decision,
      expectation,
      recognizedRelations: profile.recognizedRelations,
      variationTags: [
        ...item.variationTags,
        `decision-domain:${profile.decisionDomain}`,
      ],
    };
  });
  return { cases, schemaVersion: 4 };
}

export function scoreChallenge(
  corpus: ChallengeCorpus,
  observations: readonly ChallengeObservation[],
): ChallengeScorecard {
  const byId = new Map(observations.map((item) => [item.caseId, item]));
  const failures: string[] = [];
  if (byId.size !== observations.length)
    failures.push("challenge: duplicate observations");
  const passed = new Set<string>();
  const decisionPassed = new Set<string>();
  const explanationPassed = new Set<string>();
  const problemPassed = new Set<string>();
  const reasonPassed = new Set<string>();
  for (const item of corpus.cases) {
    const observation = byId.get(item.id);
    if (!observation) {
      failures.push(`${item.id}: missing observation`);
      continue;
    }
    const expected = item.expectation;
    const decision = item.decisionExpectation;
    const decisionObservation = projectedDecision(item, observation);
    const decisionMismatch =
      decision && decisionObservation.status !== decision.status
        ? `decision ${decision.status}`
        : undefined;
    const meaningPresent = decisionObservation.meaningLabel !== undefined;
    const expectedMeaningRelationId =
      expected.relationId ??
      (decision?.meaning === "present" && item.recognizedRelations?.length === 1
        ? item.recognizedRelations.at(0)?.relationId
        : undefined);
    const explanationMismatch = decision
      ? meaningPresent !== (decision.meaning === "present")
        ? `meaning ${decision.meaning}`
        : expectedMeaningRelationId &&
            decisionObservation.meaningRelationId !== expectedMeaningRelationId
          ? `meaning relation ${expectedMeaningRelationId}`
          : decisionObservation.meaningRelationId &&
              !decisionObservation.sourceGrounded
            ? "source-grounded meaning"
            : undefined
      : undefined;
    const problemMismatch =
      decision && !problemPolicyMatches(decision.problems, decisionObservation)
        ? `problems ${decision.problems}`
        : undefined;
    const reasonMismatch =
      decision && !reasonsAreValid(decision.status, decisionObservation)
        ? `reason integrity for ${decision.status}`
        : undefined;
    const recognitionMismatch =
      item.recognizedRelations &&
      (!sameRecognizedRelations(
        item.recognizedRelations,
        observation.recognizedRelations,
      ) || !recognizedRelationAuthorityIsSupportCoherent(observation))
        ? "recognized relations"
        : undefined;
    if (decision && !decisionMismatch) decisionPassed.add(item.id);
    if (decision && !explanationMismatch) explanationPassed.add(item.id);
    if (decision && !problemMismatch) problemPassed.add(item.id);
    if (decision && !reasonMismatch) reasonPassed.add(item.id);
    const mismatches = [
      expected.assumptionValue &&
      !observation.assumptionValues.includes(expected.assumptionValue)
        ? `assumption ${expected.assumptionValue}`
        : undefined,
      expected.candidateFamily &&
      !observation.candidates.some(
        (candidate) =>
          candidate.family === expected.candidateFamily &&
          (!expected.candidateInterpretation ||
            candidate.interpretation === expected.candidateInterpretation),
      )
        ? `candidate ${expected.candidateFamily}/${expected.candidateInterpretation ?? "*"}`
        : undefined,
      expected.conceptId && !observation.conceptIds.includes(expected.conceptId)
        ? `concept ${expected.conceptId}`
        : undefined,
      expected.excludedCandidateFamily &&
      observation.candidates.some(
        (candidate) => candidate.family === expected.excludedCandidateFamily,
      )
        ? `excluded candidate ${expected.excludedCandidateFamily}`
        : undefined,
      expected.definitionDescription &&
      !observation.definitions.some(
        (definition) =>
          definition.description === expected.definitionDescription &&
          (!expected.symbol || definition.symbol === expected.symbol),
      )
        ? `definition ${expected.symbol ?? "*"}/${expected.definitionDescription}`
        : undefined,
      expected.definitionRuleId &&
      !observation.definitions.some(
        (definition) =>
          definition.ruleId === expected.definitionRuleId &&
          (!expected.symbol || definition.symbol === expected.symbol),
      )
        ? `definition evidence ${expected.definitionRuleId}`
        : undefined,
      expected.excludedConceptId &&
      observation.conceptIds.includes(expected.excludedConceptId)
        ? `excluded concept ${expected.excludedConceptId}`
        : undefined,
      expected.excludedDefinitionSymbol &&
      observation.definitions.some(
        (definition) => definition.symbol === expected.excludedDefinitionSymbol,
      )
        ? `excluded definition ${expected.excludedDefinitionSymbol}`
        : undefined,
      expected.excludedRelationId &&
      authoritativeRelationObserved(
        item,
        observation,
        expected.excludedRelationId,
      )
        ? `excluded relation ${expected.excludedRelationId}`
        : undefined,
      expected.relationId &&
      !authoritativeRelationObserved(item, observation, expected.relationId)
        ? `relation ${expected.relationId}`
        : undefined,
      expected.shape && !observation.shapes.includes(expected.shape)
        ? `shape ${expected.shape}`
        : undefined,
      expected.sourceNotation &&
      observation.sourceNotation !== expected.sourceNotation
        ? `source notation ${expected.sourceNotation}`
        : undefined,
      expected.status && decisionObservation.status !== expected.status
        ? `status ${expected.status}`
        : undefined,
      expected.symbol && !observation.symbols.includes(expected.symbol)
        ? `symbol ${expected.symbol}`
        : undefined,
      decisionMismatch,
      explanationMismatch,
      problemMismatch,
      reasonMismatch,
      recognitionMismatch,
    ].filter((item): item is string => Boolean(item));
    if (mismatches.length)
      failures.push(`${item.id}: missing ${mismatches.join(", ")}`);
    else passed.add(item.id);
  }
  for (const item of observations) {
    if (!corpus.cases.some((candidate) => candidate.id === item.caseId)) {
      failures.push(`${item.caseId}: unexpected observation`);
    }
  }
  return {
    cases: corpus.cases.length,
    decisions: tallyExpected(
      corpus.cases,
      decisionPassed,
      CHALLENGE_DECISIONS,
      (item) => item.decisionExpectation?.status,
    ),
    explanation: countedExpected(corpus.cases, explanationPassed),
    failures: [...new Set(failures)].sort(),
    layers: tally(corpus.cases, passed, CHALLENGE_LAYERS, (item) => item.owner),
    metrics: tally(
      corpus.cases,
      passed,
      CHALLENGE_METRICS,
      (item) => item.metric,
    ),
    outcomes: tally(
      corpus.cases,
      passed,
      ["positive", "refusal"] as const,
      (item) => item.outcome,
    ),
    passed: passed.size,
    problemPolicy: tallyExpected(
      corpus.cases,
      problemPassed,
      CHALLENGE_PROBLEM_POLICIES,
      (item) => item.decisionExpectation?.problems,
    ),
    reasonIntegrity: countedExpected(corpus.cases, reasonPassed),
    schemaVersion: corpus.schemaVersion,
  };
}

function parseV3Profile(value: unknown, path: string): ChallengeV3Profile {
  const item = record(value, path);
  exact(item, ["caseId", "decision", "documentShape"], path);
  const decision = record(item.decision, `${path}.decision`);
  exact(decision, ["meaning", "problems", "status"], `${path}.decision`);
  return {
    caseId: text(item.caseId, `${path}.caseId`),
    decision: {
      meaning: oneOf(
        decision.meaning,
        ["absent", "present"] as const,
        `${path}.decision.meaning`,
      ),
      problems: oneOf(
        decision.problems,
        CHALLENGE_PROBLEM_POLICIES,
        `${path}.decision.problems`,
      ),
      status: oneOf(
        decision.status,
        CHALLENGE_DECISIONS,
        `${path}.decision.status`,
      ),
    },
    documentShape: oneOf(
      item.documentShape,
      CHALLENGE_DOCUMENT_SHAPES,
      `${path}.documentShape`,
    ),
  };
}

function parseV4Profile(value: unknown, path: string): ChallengeV4Profile {
  const item = record(value, path);
  const required = [
    "caseId",
    "decision",
    "decisionDomain",
    "recognizedRelations",
  ] as const;
  exact(item, required, path);
  for (const key of required) {
    if (!(key in item))
      throw new Error(`${path}: unknown or missing field ${key}`);
  }
  const decision = record(item.decision, `${path}.decision`);
  exact(decision, ["meaning", "problems", "status"], `${path}.decision`);
  if (!Array.isArray(item.recognizedRelations)) {
    throw new Error(`${path}.recognizedRelations: must be an array`);
  }
  const recognizedRelations = item.recognizedRelations.map((value, index) => {
    const relationPath = `${path}.recognizedRelations[${index}]`;
    const relation = record(value, relationPath);
    exact(
      relation,
      ["authority", "formulaAnchor", "relationId", "support"],
      relationPath,
    );
    return {
      authority: oneOf(
        relation.authority,
        CHALLENGE_RELATION_AUTHORITIES,
        `${relationPath}.authority`,
      ),
      formulaAnchor: oneOf(
        relation.formulaAnchor,
        ["selected-formula"] as const,
        `${relationPath}.formulaAnchor`,
      ),
      relationId: text(relation.relationId, `${relationPath}.relationId`),
      support: oneOf(
        relation.support,
        CHALLENGE_INTERPRETATION_SUPPORT,
        `${relationPath}.support`,
      ),
    };
  });
  unique(
    recognizedRelations.map((relation) => relation.relationId),
    `${path}.recognizedRelations.relationId`,
  );
  return {
    caseId: text(item.caseId, `${path}.caseId`),
    decision: {
      meaning: oneOf(
        decision.meaning,
        ["absent", "present"] as const,
        `${path}.decision.meaning`,
      ),
      problems: oneOf(
        decision.problems,
        CHALLENGE_PROBLEM_POLICIES,
        `${path}.decision.problems`,
      ),
      status: oneOf(
        decision.status,
        CHALLENGE_DECISIONS,
        `${path}.decision.status`,
      ),
    },
    decisionDomain: oneOf(
      item.decisionDomain,
      CHALLENGE_DECISION_DOMAINS,
      `${path}.decisionDomain`,
    ),
    recognizedRelations,
  };
}

function withoutRelationExpectation(
  expectation: ChallengeExpectation,
): ChallengeExpectation {
  const { relationId: _relationId, ...rest } = expectation;
  return rest;
}

function shapeChallengeCase(
  item: ChallengeCase,
  profile: ChallengeV3Profile,
): ChallengeCase {
  const target = item.documents.find(
    (document) => document.fileId === item.cursor.fileId,
  );
  if (!target) throw new Error(`${item.id}: missing cursor document`);
  const markdown =
    target.path.endsWith(".md") || target.path.endsWith(".markdown");
  const marker = item.id.replaceAll(/[^a-zA-Z0-9]/gu, "-");
  const equation = markdown
    ? `\n\nA separate calibration check records $\\xi_{\\mathrm{aux}}=17$.\n`
    : `\n\\[\\xi_{\\mathrm{aux}}=17\\]\n`;
  const prose =
    "The surrounding report compares several independent measurements. " +
    "Only explicit declarations in the current scope may determine the notation below.\n\n";
  const documents = item.documents.map((document) => {
    if (document.fileId !== target.fileId) return document;
    const content = (() => {
      switch (profile.documentShape) {
        case "distant-prose":
          return prose.repeat(3) + document.content + equation;
        case "macro-neighbor":
          return markdown
            ? prose + document.content + equation
            : `\\newcommand{\\auxmetric}{\\xi_{\\mathrm{aux}}}\n$\\auxmetric=17$.\n${document.content}`;
        case "malformed-neighbor":
          return (
            document.content +
            equation +
            (markdown
              ? "\nAn unfinished neighbor is $\\frac{1}{"
              : "\n$\\frac{1}{$")
          );
        case "multi-equation":
          return (
            equation + prose + document.content + equation.replace("17", "19")
          );
        case "project-neighbor":
          return prose + document.content;
        case "sectioned":
          return markdown
            ? `# Background\n\n${equation}\n# Reported result\n\n${document.content}`
            : `\\section{Background}\n${equation}\\section{Reported result}\n${document.content}`;
      }
    })();
    return { ...document, content };
  });
  if (profile.documentShape === "project-neighbor") {
    documents.push({
      content: `Independent appendix for ${marker}.\n${equation}`,
      fileId: `v3-neighbor-${marker}`,
      path: `v3-neighbor-${marker}.${markdown ? "md" : "tex"}`,
    });
  }
  return {
    ...item,
    decisionExpectation: profile.decision,
    documents,
    documentShape: profile.documentShape,
    variationTags: [
      ...item.variationTags,
      `document-shape:${profile.documentShape}`,
    ],
  };
}

function problemPolicyMatches(
  policy: ChallengeProblemPolicy,
  observation: ChallengeDecisionObservation,
): boolean {
  return policy === "none"
    ? observation.problemCount === 0
    : observation.problemCount > 0 &&
        observation.reasonKinds.includes("source-conflict");
}

function reasonsAreValid(
  status: ChallengeDecision,
  observation: ChallengeDecisionObservation,
): boolean {
  const allowed: Readonly<Record<ChallengeDecision, ReadonlySet<string>>> = {
    ambiguous: new Set(["uncertainty", "engine-limit"]),
    conflicting: new Set(["source-conflict"]),
    established: new Set(["proof"]),
    partial: new Set(["uncertainty", "engine-limit"]),
    unsupported: new Set(["uncertainty", "engine-limit"]),
  };
  if (observation.reasonKinds.some((kind) => !allowed[status].has(kind)))
    return false;
  if (status === "established") {
    return (
      observation.reasonKinds.includes("proof") && observation.sourceGrounded
    );
  }
  if (status === "conflicting") {
    return (
      observation.reasonKinds.includes("source-conflict") &&
      observation.sourceGrounded
    );
  }
  return status === "partial" || observation.reasonKinds.length > 0;
}

function projectedDecision(
  item: ChallengeCase,
  observation: ChallengeObservation,
): ChallengeDecisionObservation {
  if (item.decisionDomain === "cursor-entity") {
    return observation.entityDecision ?? missingDecisionObservation();
  }
  if (item.decisionDomain === "selected-formula") {
    return observation.formulaDecision ?? missingDecisionObservation();
  }
  return observation;
}

function missingDecisionObservation(): ChallengeDecisionObservation {
  return {
    problemCount: 0,
    reasonKinds: [],
    sourceGrounded: false,
  };
}

function sameRecognizedRelations(
  expected: readonly ChallengeRecognizedRelation[],
  actual: readonly ChallengeRecognizedRelation[] | undefined,
): boolean {
  if (!actual || expected.length !== actual.length) return false;
  const key = (item: ChallengeRecognizedRelation) =>
    `${item.relationId}\u0000${item.support}\u0000${item.authority}\u0000${item.formulaAnchor}`;
  const expectedKeys = expected.map(key).sort();
  const actualKeys = actual.map(key).sort();
  return expectedKeys.every((value, index) => value === actualKeys[index]);
}

function recognizedRelationAuthorityIsSupportCoherent(
  observation: ChallengeObservation,
): boolean {
  return Boolean(
    observation.recognizedRelations?.every(
      (recognized) =>
        recognized.authority !== "authoritative" ||
        recognized.support === "explicit" ||
        recognized.support === "derived",
    ),
  );
}

function authoritativeRelationObserved(
  item: ChallengeCase,
  observation: ChallengeObservation,
  relationId: string,
): boolean {
  if (item.recognizedRelations) {
    return Boolean(
      observation.recognizedRelations?.some(
        (recognized) =>
          recognized.authority === "authoritative" &&
          recognized.relationId === relationId,
      ),
    );
  }
  return observation.relationIds.includes(relationId);
}

function countedExpected(
  cases: readonly ChallengeCase[],
  passed: ReadonlySet<string>,
): { passed: number; total: number } {
  const expected = cases.filter((item) => item.decisionExpectation);
  return {
    passed: expected.filter((item) => passed.has(item.id)).length,
    total: expected.length,
  };
}

function tallyExpected<const Keys extends readonly string[]>(
  items: readonly ChallengeCase[],
  passed: ReadonlySet<string>,
  keys: Keys,
  select: (item: ChallengeCase) => Keys[number] | undefined,
): Record<Keys[number], { passed: number; total: number }> {
  return Object.fromEntries(
    keys.map((key) => {
      const selected = items.filter((item) => select(item) === key);
      return [
        key,
        {
          passed: selected.filter((item) => passed.has(item.id)).length,
          total: selected.length,
        },
      ];
    }),
  ) as Record<Keys[number], { passed: number; total: number }>;
}

function parseCase(value: unknown, path: string): ChallengeCase {
  const item = record(value, path);
  exact(
    item,
    [
      "id",
      "documents",
      "cursor",
      "expectation",
      "metric",
      "outcome",
      "owner",
      "variationTags",
    ],
    path,
  );
  const id = text(item.id, `${path}.id`);
  if (!Array.isArray(item.documents) || !item.documents.length) {
    throw new Error(`${path}.documents: must be a nonempty array`);
  }
  const documents = item.documents.map((value, index) => {
    const document = record(value, `${path}.documents[${index}]`);
    exact(
      document,
      ["content", "fileId", "path"],
      `${path}.documents[${index}]`,
    );
    return {
      content: text(document.content, `${path}.documents[${index}].content`),
      fileId: text(document.fileId, `${path}.documents[${index}].fileId`),
      path: text(document.path, `${path}.documents[${index}].path`),
    };
  });
  unique(
    documents.map((item) => item.fileId),
    `${path}.documents.fileId`,
  );
  const cursor = record(item.cursor, `${path}.cursor`);
  exact(cursor, ["edge", "fileId", "needle"], `${path}.cursor`);
  const cursorFileId = text(cursor.fileId, `${path}.cursor.fileId`);
  const needle = text(cursor.needle, `${path}.cursor.needle`);
  const cursorDocument = documents.find(
    (document) => document.fileId === cursorFileId,
  );
  if (!cursorDocument)
    throw new Error(`${path}.cursor.fileId: unknown ${cursorFileId}`);
  const occurrences = cursorDocument.content.split(needle).length - 1;
  if (occurrences !== 1) {
    throw new Error(
      `${path}.cursor.needle: must occur exactly once; found ${occurrences}`,
    );
  }
  const expectation = record(item.expectation, `${path}.expectation`);
  const expectationKeys = [
    "assumptionValue",
    "candidateFamily",
    "candidateInterpretation",
    "conceptId",
    "excludedCandidateFamily",
    "definitionDescription",
    "definitionRuleId",
    "excludedConceptId",
    "excludedDefinitionSymbol",
    "excludedRelationId",
    "relationId",
    "shape",
    "sourceNotation",
    "status",
    "symbol",
  ] as const;
  exact(expectation, expectationKeys, `${path}.expectation`);
  if (!Object.keys(expectation).length) {
    throw new Error(`${path}.expectation: must not be empty`);
  }
  return {
    cursor: {
      ...(cursor.edge === undefined
        ? {}
        : {
            edge: oneOf(
              cursor.edge,
              ["after", "before"] as const,
              `${path}.cursor.edge`,
            ),
          }),
      fileId: cursorFileId,
      needle,
    },
    documents,
    expectation: Object.fromEntries(
      expectationKeys.flatMap((key) =>
        expectation[key] === undefined
          ? []
          : [[key, text(expectation[key], `${path}.expectation.${key}`)]],
      ),
    ),
    id,
    metric: oneOf(item.metric, CHALLENGE_METRICS, `${path}.metric`),
    outcome: oneOf(
      item.outcome,
      ["positive", "refusal"] as const,
      `${path}.outcome`,
    ),
    owner: oneOf(item.owner, CHALLENGE_LAYERS, `${path}.owner`),
    variationTags: stringList(item.variationTags, `${path}.variationTags`),
  };
}

function validateBoundaryPairs(cases: readonly ChallengeCase[]): void {
  const pairs = new Map<string, Set<ChallengeOutcome>>();
  for (const item of cases) {
    for (const tag of item.variationTags) {
      if (!tag.startsWith("boundary-pair:")) continue;
      const pair = tag.slice("boundary-pair:".length);
      if (!pair) throw new Error(`${item.id}: boundary pair must have an id`);
      const outcomes = pairs.get(pair) ?? new Set<ChallengeOutcome>();
      outcomes.add(item.outcome);
      pairs.set(pair, outcomes);
    }
  }
  if (pairs.size < 12) {
    throw new Error(
      "challenge.cases: must contain at least 12 semantic boundary pairs",
    );
  }
  for (const [pair, outcomes] of pairs) {
    if (!outcomes.has("positive") || !outcomes.has("refusal")) {
      throw new Error(`challenge.cases: incomplete boundary pair ${pair}`);
    }
  }
}

function normalizedSource(source: string): string {
  return source.replace(/\s+/gu, " ").trim();
}

function parseJsonSource(source: string, path: string): unknown {
  try {
    return JSON.parse(source) as unknown;
  } catch {
    throw new Error(`${path}: must be valid JSON`);
  }
}

function validatePinnedDigest(
  value: unknown,
  pinned: string,
  actual: string,
  path: string,
): void {
  const declared = text(value, path);
  if (declared !== pinned) throw new Error(`${path}: must equal ${pinned}`);
  if (actual !== declared) throw new Error(`${path}: source digest mismatch`);
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function tally<
  const Keys extends readonly string[],
  Item extends { readonly id: string },
>(
  items: readonly Item[],
  passed: ReadonlySet<string>,
  keys: Keys,
  select: (item: Item) => Keys[number],
): Record<Keys[number], { passed: number; total: number }> {
  return Object.fromEntries(
    keys.map((key) => {
      const selected = items.filter((item) => select(item) === key);
      return [
        key,
        {
          passed: selected.filter((item) => passed.has(item.id)).length,
          total: selected.length,
        },
      ];
    }),
  ) as Record<Keys[number], { passed: number; total: number }>;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function exact(
  value: Record<string, unknown>,
  keys: readonly string[],
  path: string,
): void {
  const allowed = new Set(keys);
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  if (unknown.length)
    throw new Error(`${path}: unknown field ${unknown.sort()[0]}`);
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim())
    throw new Error(`${path}: must be text`);
  return value;
}

function stringList(value: unknown, path: string): string[] {
  if (!Array.isArray(value) || !value.length)
    throw new Error(`${path}: must be nonempty text[]`);
  const output = value.map((item, index) => text(item, `${path}[${index}]`));
  unique(output, path);
  return output;
}

function oneOf<const Values extends readonly string[]>(
  value: unknown,
  values: Values,
  path: string,
): Values[number] {
  if (typeof value !== "string" || !values.includes(value)) {
    throw new Error(`${path}: must be one of ${values.join(", ")}`);
  }
  return value;
}

function unique(values: readonly string[], path: string): void {
  if (new Set(values).size !== values.length)
    throw new Error(`${path}: must be unique`);
}
