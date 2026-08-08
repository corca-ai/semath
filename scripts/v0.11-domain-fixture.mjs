export function buildDomainPackFixture(corpus) {
  const epoch = "parity:11-domain-packs";
  const queries = [];
  const expectations = [];
  const catalogs = [];

  for (const [index, entry] of corpus.cases.entries()) {
    for (const [variantIndex, variant] of recognitionVariants(entry).entries()) {
      const batch = index * 2 + (variantIndex < 3 ? 0 : 1);
      const catalog = catalogs[batch] ?? {
        content: "",
        fileId: `catalog-${batch}`,
      };
      catalogs[batch] = catalog;
      const start = catalog.content.length;
      catalog.content += `${variant.content}\n`;
      const offset = cursorInsideSegment(variant.content, start);
      if (variant.expected) {
        for (const kind of [
          "formulaRecognition",
          "formulaCompletion",
          "formulaRewrite",
        ]) {
          queries.push(envelope(epoch, catalog.fileId, offset, kind));
          expectations.push({
            entry,
            expected: true,
            kind,
            variant: variant.id,
          });
        }
      } else {
        queries.push(
          envelope(epoch, catalog.fileId, offset, "formulaRecognition"),
        );
        expectations.push({
          entry,
          expected: false,
          kind: "formulaRecognition",
          variant: variant.id,
        });
      }
    }
  }

  for (const [index, entry] of (corpus.collisions ?? []).entries()) {
    const catalog = {
      content: `${entry.content}\n`,
      fileId: `collision-${index}`,
    };
    catalogs.push(catalog);
    queries.push(
      envelope(
        epoch,
        catalog.fileId,
        cursorInsideSegment(entry.content, 0),
        "formulaRecognition",
      ),
    );
    expectations.push({
      entry,
      expected: true,
      kind: "formulaRecognition",
      variant: `collision-${entry.id}`,
    });
  }

  return {
    expectations,
    fixture: {
      snapshot: {
        protocolVersion: 1,
        epoch,
        inventoryVersion: 1,
        projectId: "v0.11-domain-packs",
        mainFileId: catalogs[0]?.fileId ?? null,
        documents: catalogs.map((catalog) =>
          document(
            catalog.fileId,
            `packs/${catalog.fileId}.md`,
            catalog.content,
          ),
        ),
      },
      queries,
    },
  };
}

export function assertDomainPackResults(results, expectations) {
  if (results.length !== expectations.length) {
    throw new Error(
      `domain pack result count ${results.length} differs from ${expectations.length}`,
    );
  }
  const recognized = new Set();
  for (const [index, expectation] of expectations.entries()) {
    const value = results[index]?.value;
    if (expectation.kind === "formulaRecognition") {
      if (value?.kind !== "formulaRecognitions") {
        throw new Error(`${expectation.entry.id}: missing recognition result`);
      }
      const actual = value.recognitions.map(
        (recognition) => recognition.patternId,
      );
      const expected = expectation.expected
        ? (expectation.entry.expectedPatterns ?? [
            expectation.entry.expectedPattern,
          ])
        : [];
      if (!sameValues(actual, expected)) {
        throw new Error(
          `${expectation.entry.id}/${expectation.variant}: expected recognition [${expected.join(", ")}], got [${actual.join(", ")}]`,
        );
      }
      for (const pattern of expected) recognized.add(pattern);
      continue;
    }
    const items =
      expectation.kind === "formulaCompletion"
        ? value?.completions
        : value?.rewrites;
    if (!Array.isArray(items) || items.length !== 0) {
      throw new Error(
        `${expectation.entry.id}/${expectation.variant}: recognition-only entry exposed ${expectation.kind}`,
      );
    }
  }
  return { recognized: recognized.size, results: results.length };
}

export function recognitionVariants(entry) {
  const formula = mathBody(entry.content);
  const truncated = formula.slice(0, Math.max(1, formula.length - 1));
  const positives = [
    { id: "positive-inline", content: `$${formula}$`, expected: true },
    { id: "positive-display", content: `\\[${formula}\\]`, expected: true },
    { id: "positive-padded", content: `$  ${formula}  $`, expected: true },
    {
      id: "positive-grouped",
      content: `$\\left(${formula}\\right)$`,
      expected: true,
    },
    {
      id: "positive-braced",
      content: `$ {${formula}} $`,
      expected: true,
    },
  ];
  if (!formula.includes("=")) {
    positives.push({
      id: "positive-assignment",
      content: `$q=${formula}$`,
      expected: true,
    });
  }
  return [
    ...positives,
    { id: "negative-unfinished", content: `$${truncated}{$`, expected: false },
    {
      id: "negative-function-argument",
      content: `$g\\left(${formula}\\right)$`,
      expected: false,
    },
    {
      id: "negative-subscript",
      content: `$z_{${formula}}$`,
      expected: false,
    },
    {
      id: "negative-superscript",
      content: `$z^{${formula}}$`,
      expected: false,
    },
    {
      id: "negative-adjacent-expression",
      content: `$z+\\left(${formula}\\right)$`,
      expected: false,
    },
  ];
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

function sameValues(actual, expected) {
  return (
    actual.length === expected.length &&
    [...actual].sort().every((value, index) => value === [...expected].sort()[index])
  );
}

function document(fileId, path, content) {
  return {
    fileId,
    path,
    language: "markdown",
    content,
    documentVersion: 1,
  };
}

function envelope(epoch, fileId, offset, kind) {
  return {
    protocolVersion: 1,
    epoch,
    inventoryVersion: 1,
    documentVersion: 1,
    analysisGeneration: 1,
    query: { kind, fileId, offset },
  };
}

function cursorInsideSegment(content, segmentStart) {
  const openLength = content.startsWith("\\[") ? 2 : 1;
  const closeLength = content.endsWith("\\]") ? 2 : 1;
  const end = content.length - closeLength;
  return (
    segmentStart +
    openLength +
    Math.floor((Math.max(end, openLength + 1) - openLength) / 2)
  );
}
