export function singleDroppedPath(paths: string[]): string | null {
  return paths.length === 1 ? paths[0] : null;
}
