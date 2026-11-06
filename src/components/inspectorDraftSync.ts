export function shouldResetInspectorDrafts(
  previousRevision: number | undefined,
  revision: number,
  previousItemId: string | undefined,
  itemId: string | undefined
): boolean {
  return previousRevision !== revision || previousItemId !== itemId;
}
