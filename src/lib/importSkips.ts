/**
 * The status line's account of files an import left out: how many, then the
 * first one's name and why, so a lone skipped file explains itself. Each
 * entry is the backend's "file name: reason"; the full paths are in the log.
 */
export function skippedTail(skipped: readonly string[]): string {
  const [first] = skipped;
  return first === undefined ? "" : `; ${skipped.length} skipped (${first})`;
}
