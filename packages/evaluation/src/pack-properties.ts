export const PROPERTY_FAMILIES = [
  "positive",
  "refusal",
  "scope",
  "mutation",
  "macro-project",
  "cursor",
  "attachment",
] as const;

export type PropertyFamily = (typeof PROPERTY_FAMILIES)[number];

export interface PropertyLawDeclaration {
  readonly canonicalRelation: string;
  readonly id: string;
  readonly representations?: readonly string[];
  readonly roles: readonly { readonly id: string }[];
}

export interface PropertyPackDeclaration {
  readonly laws: readonly PropertyLawDeclaration[];
  readonly packId: string;
}

export type PropertyOracle =
  | { readonly kind: "decision-equal" }
  | { readonly kind: "must-refuse" }
  | { readonly kind: "must-conflict" }
  | { readonly kind: "cursor-equal" };

export interface PackPropertyCell {
  readonly family: PropertyFamily;
  readonly id: string;
  readonly lawId: string;
  readonly oracle: PropertyOracle;
  readonly packId: string;
  readonly semanticForm: string;
  readonly transform: string;
  readonly variant: number;
}

const FAMILY_TRANSFORMS: Readonly<
  Record<PropertyFamily, readonly { oracle: PropertyOracle; transform: string }[]>
> = {
  positive: [
    { oracle: { kind: "decision-equal" }, transform: "alpha-renaming" },
    { oracle: { kind: "decision-equal" }, transform: "safe-grouping" },
    { oracle: { kind: "decision-equal" }, transform: "neutral-prose" },
  ],
  refusal: [
    { oracle: { kind: "must-refuse" }, transform: "missing-role-evidence" },
    { oracle: { kind: "must-conflict" }, transform: "conflicting-role-evidence" },
  ],
  scope: [
    { oracle: { kind: "decision-equal" }, transform: "declaration-before-use" },
    { oracle: { kind: "must-refuse" }, transform: "declaration-after-use" },
    { oracle: { kind: "must-refuse" }, transform: "disconnected-evidence" },
  ],
  mutation: [
    { oracle: { kind: "must-refuse" }, transform: "operator-mutation" },
    { oracle: { kind: "must-refuse" }, transform: "role-swap" },
    { oracle: { kind: "must-refuse" }, transform: "extra-term" },
  ],
  "macro-project": [
    { oracle: { kind: "decision-equal" }, transform: "transparent-macro" },
    { oracle: { kind: "decision-equal" }, transform: "included-evidence" },
    { oracle: { kind: "decision-equal" }, transform: "unrelated-pack" },
  ],
  cursor: [
    { oracle: { kind: "cursor-equal" }, transform: "occurrence-edges" },
    { oracle: { kind: "cursor-equal" }, transform: "decorated-components" },
    { oracle: { kind: "cursor-equal" }, transform: "application-edge" },
  ],
  attachment: [
    { oracle: { kind: "decision-equal" }, transform: "prose-before-formula" },
    { oracle: { kind: "decision-equal" }, transform: "formula-before-where-clause" },
    { oracle: { kind: "decision-equal" }, transform: "formula-before-neighbor-sentence" },
    { oracle: { kind: "must-refuse" }, transform: "attachment-retraction" },
    { oracle: { kind: "must-refuse" }, transform: "sibling-section-attachment" },
    { oracle: { kind: "must-refuse" }, transform: "cited-or-hedged-attachment" },
  ],
};

/**
 * Plans semantic properties from reviewed pack declarations without consulting
 * the production matcher or any runtime recognition output.
 */
export function planPackPropertyCells(
  packs: readonly PropertyPackDeclaration[],
  seed: number,
): readonly PackPropertyCell[] {
  if (!Number.isSafeInteger(seed)) throw new Error("property seed must be an integer");
  const cells: PackPropertyCell[] = [];
  for (const pack of [...packs].sort((left, right) =>
    left.packId.localeCompare(right.packId),
  )) {
    for (const law of [...pack.laws].sort((left, right) =>
      left.id.localeCompare(right.id),
    )) {
      const forms = [law.canonicalRelation, ...(law.representations ?? [])].filter(validSemanticForm);
      if (!forms.length) {
        throw new Error(`${pack.packId}/${law.id}: no renderable semantic form`);
      }
      for (const [familyIndex, family] of PROPERTY_FAMILIES.entries()) {
        const variants = FAMILY_TRANSFORMS[family];
        const selectedVariants = family === "attachment"
          ? variants.map((_, index) => index)
          : [stableIndex(seed, `${pack.packId}/${law.id}/${family}`, variants.length)];
        for (const variant of selectedVariants) {
          const selected = variants[variant]!;
          const semanticForm = forms[(variant + familyIndex) % forms.length]!;
          cells.push({
            family,
            id: `${pack.packId}/${law.id}/${family}/${selected.transform}`,
            lawId: law.id,
            oracle: selected.oracle,
            packId: pack.packId,
            semanticForm,
            transform: selected.transform,
            variant,
          });
        }
      }
    }
  }
  assertPropertyPlan(cells, packs);
  return cells;
}

export function assertPropertyPlan(
  cells: readonly PackPropertyCell[],
  packs: readonly PropertyPackDeclaration[],
): void {
  const ids = new Set<string>();
  for (const cell of cells) {
    if (ids.has(cell.id)) throw new Error(`duplicate property cell ${cell.id}`);
    ids.add(cell.id);
    if (!validSemanticForm(cell.semanticForm)) {
      throw new Error(`${cell.id}: invalid semantic form`);
    }
  }
  for (const pack of packs) {
    for (const law of pack.laws) {
      const owned = cells.filter(
        (cell) => cell.packId === pack.packId && cell.lawId === law.id,
      );
      for (const family of PROPERTY_FAMILIES) {
        if (!owned.some((cell) => cell.family === family)) {
          throw new Error(`${pack.packId}/${law.id}: missing ${family} property`);
        }
      }
    }
  }
}

export function shrinkPropertyFailure(
  cell: PackPropertyCell,
  stillFails: (candidate: PackPropertyCell) => boolean,
): PackPropertyCell {
  const candidates = [
    { ...cell, semanticForm: cell.semanticForm.trim() },
    { ...cell, semanticForm: stripOuterGroups(cell.semanticForm.trim()) },
  ];
  let smallest = cell;
  for (const candidate of candidates) {
    if (candidate.semanticForm.length <= smallest.semanticForm.length && stillFails(candidate)) {
      smallest = candidate;
    }
  }
  return smallest;
}

function stableIndex(seed: number, identity: string, size: number): number {
  let hash = seed >>> 0;
  for (const character of identity) {
    hash = Math.imul(hash ^ character.codePointAt(0)!, 16_777_619) >>> 0;
  }
  return hash % size;
}

function validSemanticForm(form: string): boolean {
  if (!form.trim() || /[\u0000-\u001f]/u.test(form)) return false;
  let depth = 0;
  for (const character of form) {
    if (character === "{") depth += 1;
    if (character === "}") depth -= 1;
    if (depth < 0) return false;
  }
  return depth === 0;
}

function stripOuterGroups(form: string): string {
  return form.startsWith("{") && form.endsWith("}")
    ? form.slice(1, -1).trim()
    : form;
}
