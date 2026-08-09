import type {
  Corpus,
  CorpusCase,
  CorpusSuiteConfig,
  MetamorphicTransform,
  QualityManifest,
} from "./model";

export interface MetamorphicCase {
  case: CorpusCase;
  sourceCaseId: string;
  suiteId: string;
  transform: MetamorphicTransform;
}

export function planMetamorphicCases(
  manifest: QualityManifest,
  corpora: ReadonlyMap<string, Corpus>,
): MetamorphicCase[] {
  const planned: MetamorphicCase[] = [];
  for (const suite of [...manifest.suites].sort((left, right) =>
    left.id.localeCompare(right.id),
  )) {
    const corpus = corpora.get(suite.id);
    if (!corpus) continue;
    for (const source of representatives(
      corpus,
      manifest.metamorphic.casesPerLaw,
    )) {
      for (const transform of manifest.metamorphic.transforms) {
        const transformed = transformCase(source, transform);
        if (!transformed) continue;
        planned.push({
          case: transformed,
          sourceCaseId: source.id,
          suiteId: suite.id,
          transform,
        });
      }
    }
  }
  return planned;
}

function representatives(corpus: Corpus, limit: number): CorpusCase[] {
  const groups = new Map<string, CorpusCase[]>();
  for (const item of corpus.cases) {
    const target = "lawId" in item ? item.lawId : `global:${item.refusalCategory}`;
    const key = `${target}\u0000${item.expectation}`;
    const group = groups.get(key) ?? [];
    group.push(item);
    groups.set(key, group);
  }
  return [...groups]
    .sort(([left], [right]) => left.localeCompare(right))
    .flatMap(([, cases]) =>
      [...cases]
        .sort((left, right) => left.id.localeCompare(right.id))
        .slice(0, limit),
    );
}

function transformCase(
  source: CorpusCase,
  transform: MetamorphicTransform,
): CorpusCase | null {
  const id = `${source.id}-metamorphic-${transform}`;
  if (transform === "document-order") {
    if (source.documents.length < 2) return null;
    return { ...source, documents: [...source.documents].reverse(), id };
  }
  return {
    ...source,
    documents: source.documents.map((document) => {
      if (document.fileId !== source.cursor.fileId) return document;
      if (transform === "neutral-prose") {
        return {
          ...document,
          content: `Context note: identifiers retain their stated meanings.\n${document.content}`,
        };
      }
      const comment = /\.md$/iu.test(document.path)
        ? "<!-- semath metamorphic invariant -->"
        : "% semath metamorphic invariant";
      return { ...document, content: `${document.content}\n${comment}` };
    }),
    id,
  };
}

export function suiteById(
  manifest: QualityManifest,
  suiteId: string,
): CorpusSuiteConfig {
  const suite = manifest.suites.find((item) => item.id === suiteId);
  if (!suite) throw new Error(`unknown corpus suite ${suiteId}`);
  return suite;
}
