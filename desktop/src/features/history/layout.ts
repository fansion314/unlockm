export const MIN_TITLE_ALBUM_RATIO = 35;
export const MAX_TITLE_ALBUM_RATIO = 75;

export function clampTitleAlbumRatio(value: number): number {
  return Math.min(MAX_TITLE_ALBUM_RATIO, Math.max(MIN_TITLE_ALBUM_RATIO, Math.round(value)));
}

export function visibleItemNumber(page: number, pageSize: number, index: number): number {
  return (page - 1) * pageSize + index + 1;
}

export type PaginationItem = number | "ellipsis";

export function paginationItems(currentPage: number, pageCount: number): PaginationItem[] {
  if (pageCount <= 1) return [];
  const current = Math.min(pageCount, Math.max(1, currentPage));
  const selected = new Set<number>();
  const addRange = (start: number, end: number) => {
    for (let value = Math.max(1, start); value <= Math.min(pageCount, end); value += 1) {
      selected.add(value);
    }
  };
  addRange(1, 2);
  addRange(current - 3, current + 3);
  addRange(pageCount - 1, pageCount);

  const sorted = [...selected].sort((left, right) => left - right);
  const result: PaginationItem[] = [];
  for (const value of sorted) {
    const previous = result.at(-1);
    if (typeof previous === "number") {
      const gap = value - previous;
      if (gap === 2) result.push(previous + 1);
      if (gap > 2) result.push("ellipsis");
    }
    result.push(value);
  }
  return result;
}
