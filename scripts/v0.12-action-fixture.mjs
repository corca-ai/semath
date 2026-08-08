export function buildActionPatternFixture(corpus) {
  const epoch = "parity:12-action-calibration";
  const documents = [];
  const expectations = [];
  const queries = [];

  for (const [caseIndex, entry] of corpus.cases.entries()) {
    let content = entry.preamble;
    const formula = mathBody(entry.surfaces[0]);
    const positiveSurfaces = [
      ...entry.surfaces,
      `$\\left(${formula}\\right)$`,
      `$ {${formula}} $`,
    ];
    if (!formula.includes("=")) positiveSurfaces.push(`$q=${formula}$`);
    const variants = [
      ...positiveSurfaces.map((surface, index) => ({
        expected: true,
        id: `positive-${index + 1}`,
        surface,
      })),
      ...negativeVariants(entry.surfaces[0]),
    ];
    for (const variant of variants) {
      const start = content.length;
      content += `${variant.surface}\n`;
      queries.push(
        envelope(
          epoch,
          `action-${caseIndex}`,
          cursorInsideSegment(variant.surface, start),
        ),
      );
      expectations.push({ entry, variant });
    }
    documents.push({
      content,
      documentVersion: 1,
      fileId: `action-${caseIndex}`,
      language: "markdown",
      path: `calibration/action-${caseIndex}.md`,
    });
  }

  return {
    expectations,
    fixture: {
      snapshot: {
        protocolVersion: 1,
        epoch,
        inventoryVersion: 1,
        projectId: "v0.12-action-calibration",
        mainFileId: documents[0]?.fileId ?? null,
        documents,
      },
      queries,
    },
  };
}

export function assertActionPatternResults(results, expectations) {
  if (results.length !== expectations.length) {
    throw new Error(
      `action calibration result count ${results.length} differs from ${expectations.length}`,
    );
  }
  const recognized = new Set();
  for (const [index, expectation] of expectations.entries()) {
    const value = results[index]?.value;
    if (value?.kind !== "formulaRecognitions") {
      throw new Error(
        `${expectation.entry.id}/${expectation.variant.id}: missing recognition result`,
      );
    }
    const actual = value.recognitions.map(
      (recognition) => recognition.patternId,
    );
    const expected = expectation.variant.expected
      ? [expectation.entry.expectedPattern]
      : [];
    if (!sameValues(actual, expected)) {
      throw new Error(
        `${expectation.entry.id}/${expectation.variant.id}: expected [${expected.join(", ")}], got [${actual.join(", ")}]`,
      );
    }
    for (const pattern of expected) recognized.add(pattern);
  }
  return { recognized: recognized.size, results: results.length };
}

function negativeVariants(surface) {
  const formula = mathBody(surface);
  const truncated = formula.slice(0, Math.max(1, formula.length - 1));
  return [
    { expected: false, id: "negative-unfinished", surface: `$${truncated}{$` },
    {
      expected: false,
      id: "negative-function-argument",
      surface: `$g\\left(${formula}\\right)$`,
    },
    {
      expected: false,
      id: "negative-subscript",
      surface: `$z_{${formula}}$`,
    },
    {
      expected: false,
      id: "negative-superscript",
      surface: `$z^{${formula}}$`,
    },
    {
      expected: false,
      id: "negative-adjacent-expression",
      surface: `$z+\\left(${formula}\\right)$`,
    },
  ];
}

function envelope(epoch, fileId, offset) {
  return {
    protocolVersion: 1,
    epoch,
    inventoryVersion: 1,
    documentVersion: 1,
    analysisGeneration: 1,
    query: { kind: "formulaRecognition", fileId, offset },
  };
}

function mathBody(content) {
  if (content.startsWith("$") && content.endsWith("$")) {
    return content.slice(1, -1);
  }
  if (content.startsWith("\\[") && content.endsWith("\\]")) {
    return content.slice(2, -2);
  }
  throw new Error(`unsupported math fixture delimiter: ${content}`);
}

function cursorInsideSegment(content, segmentStart) {
  const openLength = content.startsWith("\\[") ? 2 : 1;
  const closeLength = content.endsWith("\\]") ? 2 : 1;
  const end = content.length - closeLength;
  return segmentStart + openLength + Math.floor((end - openLength) / 2);
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
