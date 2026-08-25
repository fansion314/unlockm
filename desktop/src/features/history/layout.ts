export const MIN_TITLE_ALBUM_RATIO = 35;
export const MAX_TITLE_ALBUM_RATIO = 75;

export function clampTitleAlbumRatio(value: number): number {
  return Math.min(MAX_TITLE_ALBUM_RATIO, Math.max(MIN_TITLE_ALBUM_RATIO, Math.round(value)));
}

export function visibleItemNumber(page: number, pageSize: number, index: number): number {
  return (page - 1) * pageSize + index + 1;
}
