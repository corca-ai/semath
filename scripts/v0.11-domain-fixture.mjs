export function buildDomainPackFixture(corpus) {
  const epoch = "parity:11-domain-packs";
  const queries = [];
  const expectations = [];
  const catalogs = [];

  for (const [index, entry] of corpus.cases.entries()) {
    const batch = Math.floor(index / 4);
    const catalog = catalogs[batch] ?? {
      content: "",
      fileId: `catalog-${batch}`,
    };
    catalogs[batch] = catalog;
    const positive = entry.content;
    const unfinished = `${entry.content.slice(0, -1)}{$`;
    const positiveStart = catalog.content.length;
    catalog.content += `${positive}\n`;
    const unfinishedStart = catalog.content.length;
    catalog.content += `${unfinished}\n`;
    const offset = cursorInsideSegment(positive, positiveStart);
    const unfinishedOffset = cursorInsideSegment(unfinished, unfinishedStart);
    for (const kind of [
      "formulaRecognition",
      "formulaCompletion",
      "formulaRewrite",
    ]) {
      queries.push(envelope(epoch, catalog.fileId, offset, kind));
      expectations.push({ entry, kind, variant: "positive" });
    }
    queries.push(
      envelope(epoch, catalog.fileId, unfinishedOffset, "formulaRecognition"),
    );
    expectations.push({
      entry,
      kind: "formulaRecognition",
      variant: "unfinished",
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
      const matched = value.recognitions.some(
        (recognition) =>
          recognition.patternId === expectation.entry.expectedPattern,
      );
      if (expectation.variant === "positive" && !matched) {
        throw new Error(
          `${expectation.entry.id}: expected ${expectation.entry.expectedPattern}`,
        );
      }
      if (expectation.variant === "unfinished" && matched) {
        throw new Error(
          `${expectation.entry.id}: recognized an unfinished expression`,
        );
      }
      if (matched) recognized.add(expectation.entry.expectedPattern);
      continue;
    }
    const items =
      expectation.kind === "formulaCompletion"
        ? value?.completions
        : value?.rewrites;
    if (!Array.isArray(items) || items.length !== 0) {
      throw new Error(
        `${expectation.entry.id}: recognition-only entry exposed ${expectation.kind}`,
      );
    }
  }
  return { recognized: recognized.size, results: results.length };
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
  const start = content.indexOf("$") + 1;
  const end = content.lastIndexOf("$");
  return segmentStart + start + Math.floor((Math.max(end, start + 1) - start) / 2);
}
