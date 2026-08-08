export function buildRealisticProjectFixture(corpus) {
  const epoch = "parity:12-realistic-project";
  const documents = corpus.documents.map((document) => ({
    ...document,
    documentVersion: 1,
    includes: projectIncludes(document.content),
  }));
  const byId = new Map(documents.map((document) => [document.fileId, document]));
  const queries = corpus.targets.map((target) => {
    const document = byId.get(target.fileId);
    if (!document) throw new Error(`${target.id}: missing document ${target.fileId}`);
    const start = document.content.indexOf(target.needle);
    if (start < 0) throw new Error(`${target.id}: missing needle ${target.needle}`);
    const offset =
      start +
      (target.needleOffset ?? Math.max(1, Math.floor(target.needle.length / 2)));
    return {
      protocolVersion: 1,
      epoch,
      inventoryVersion: 1,
      documentVersion: document.documentVersion,
      analysisGeneration: 1,
      query: { kind: target.kind, fileId: target.fileId, offset },
    };
  });
  return {
    expectations: corpus.targets,
    fixture: {
      snapshot: {
        protocolVersion: 1,
        epoch,
        inventoryVersion: 1,
        projectId: "v0.12-realistic-project",
        mainFileId: "main",
        documents,
      },
      queries,
    },
  };
}

export function assertRealisticProjectResults(results, expectations) {
  if (results.length !== expectations.length) {
    throw new Error(
      `realistic project result count ${results.length} differs from ${expectations.length}`,
    );
  }
  for (const [index, expectation] of expectations.entries()) {
    const value = results[index]?.value;
    if (expectation.expectedPatterns) {
      const recognitions =
        value?.kind === "formulaRecognitions"
          ? value.recognitions
          : value?.kind === "inspection"
            ? value.inspection.recognitions
            : undefined;
      if (!recognitions) {
        throw new Error(`${expectation.id}: missing formula recognition result`);
      }
      const actual = recognitions.map((recognition) => recognition.patternId);
      if (!sameValues(actual, expectation.expectedPatterns)) {
        throw new Error(
          `${expectation.id}: expected [${expectation.expectedPatterns.join(", ")}], got [${actual.join(", ")}]`,
        );
      }
      continue;
    }
    if (expectation.expectedFileIds) {
      if (value?.kind !== "locations") {
        throw new Error(`${expectation.id}: missing location result`);
      }
      const actual = value.locations.map((location) => location.fileId);
      if (!sameValues(actual, expectation.expectedFileIds)) {
        throw new Error(
          `${expectation.id}: expected files [${expectation.expectedFileIds.join(", ")}], got [${actual.join(", ")}]`,
        );
      }
    }
  }
  return { results: results.length };
}

function projectIncludes(content) {
  return [...content.matchAll(/\\input\{([^{}]+)\}/g)].map((match) => ({
    path: match[1],
    sourceRange: {
      startOffset: match.index + "\\input{".length,
      endOffset: match.index + "\\input{".length + match[1].length,
    },
  }));
}

function sameValues(actual, expected) {
  const sortedExpected = [...expected].sort();
  return (
    actual.length === sortedExpected.length &&
    [...actual]
      .sort()
      .every((value, index) => value === sortedExpected[index])
  );
}
