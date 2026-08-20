import type { QualityRunPlan } from "../packages/evaluation/src/index";
import type {
  ProjectSnapshot,
  QueryEnvelope,
} from "../packages/protocol/src/index";

export interface CorpusRunBatch {
  indices: readonly number[];
  queries: readonly QueryEnvelope[];
  snapshot: ProjectSnapshot;
  suiteId: string;
}

export function planCorpusRunBatches(plan: QualityRunPlan): CorpusRunBatch[] {
  if (plan.planned.length !== plan.queries.length) {
    throw new Error(
      `quality run plan has ${plan.planned.length} cases but ${plan.queries.length} queries`,
    );
  }
  if (plan.snapshot.mainFileId !== undefined) {
    throw new Error("quality corpus batching requires a project without a main file");
  }

  const indicesBySuite = new Map<string, number[]>();
  for (const [index, item] of plan.planned.entries()) {
    const indices = indicesBySuite.get(item.suiteId) ?? [];
    indices.push(index);
    indicesBySuite.set(item.suiteId, indices);
  }

  return [...indicesBySuite].map(([suiteId, indices]) => {
    const prefix = `${suiteId}/`;
    const queries = indices.map((index) => plan.queries[index]!);
    const documents = plan.snapshot.documents.filter((document) =>
      document.fileId.startsWith(prefix)
    );
    if (!documents.length) {
      throw new Error(`${suiteId}: quality run batch has no documents`);
    }
    if (plan.snapshot.documents.length > 1 && documents.length === 1) {
      throw new Error(
        `${suiteId}: cannot isolate a singleton suite without changing project semantics`,
      );
    }
    for (const query of queries) {
      if (!query.query.fileId.startsWith(prefix)) {
        throw new Error(`${suiteId}: quality run batch contains a foreign query`);
      }
    }
    return {
      indices,
      queries,
      snapshot: { ...plan.snapshot, documents },
      suiteId,
    };
  });
}

export function qualityRunPlanForBatch(
  plan: QualityRunPlan,
  batch: CorpusRunBatch,
): QualityRunPlan {
  return {
    planned: batch.indices.map((index) => plan.planned[index]!),
    queries: batch.queries,
    snapshot: batch.snapshot,
  };
}

export function flattenCorpusRunValues<Value>(
  resultCount: number,
  batches: readonly CorpusRunBatch[],
  batchValues: readonly (readonly Value[])[],
): Value[] {
  if (batches.length !== batchValues.length) {
    throw new Error(
      `quality run returned ${batchValues.length}/${batches.length} batches`,
    );
  }
  const results: Array<Value | undefined> = Array(resultCount);
  for (const [batchIndex, batch] of batches.entries()) {
    const returned = batchValues[batchIndex]!;
    if (returned.length !== batch.indices.length) {
      throw new Error(
        `${batch.suiteId}: quality run returned ${returned.length}/${batch.indices.length} results`,
      );
    }
    for (const [index, resultIndex] of batch.indices.entries()) {
      if (
        resultIndex < 0 ||
        resultIndex >= resultCount ||
        results[resultIndex] !== undefined
      ) {
        throw new Error(`${batch.suiteId}: invalid quality run result index ${resultIndex}`);
      }
      results[resultIndex] = returned[index]!;
    }
  }
  return results.map((result, index) => {
    if (result === undefined) {
      throw new Error(`quality run has no result at index ${index}`);
    }
    return result;
  });
}
