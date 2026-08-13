export function completeLifecycleUpsertIds(
  directlyChanged: ReadonlySet<string>,
  syntaxInvalidated: readonly string[],
): readonly string[] {
  return [...new Set([...directlyChanged, ...syntaxInvalidated])].sort();
}
