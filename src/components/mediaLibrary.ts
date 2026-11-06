import { Asset, FrameRate } from "../types/project";

export type MediaLibraryKindFilter = "all" | Asset["media_kind"];
export type MediaLibraryStatusFilter = "all" | "online" | "offline";
export type MediaLibrarySort = "name" | "kind" | "size";

export function sourceRangeSecondsToFrames(sourceIn: number, sourceOut: number, rate: FrameRate): { start: number; duration: number } | null {
  if (!Number.isFinite(sourceIn) || !Number.isFinite(sourceOut) || sourceIn < 0 || sourceOut <= sourceIn || rate.numerator <= 0 || rate.denominator <= 0) return null;
  const start = Math.round(sourceIn * rate.numerator / rate.denominator);
  const end = Math.round(sourceOut * rate.numerator / rate.denominator);
  const duration = end - start;
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(duration) || duration < 1) return null;
  return { start, duration };
}

export function filterAndSortMediaAssets(
  assets: Asset[],
  offlineAssetIds: ReadonlySet<string>,
  query: string,
  kind: MediaLibraryKindFilter,
  status: MediaLibraryStatusFilter,
  sort: MediaLibrarySort,
): Asset[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const filtered = assets.filter((asset) => {
    if (kind !== "all" && asset.media_kind !== kind) return false;
    const isOffline = offlineAssetIds.has(asset.asset_id);
    if (status === "offline" && !isOffline) return false;
    if (status === "online" && isOffline) return false;
    return !normalizedQuery || asset.name.toLocaleLowerCase().includes(normalizedQuery);
  });

  return filtered.sort((left, right) => {
    if (sort === "size") return right.file_size - left.file_size || left.name.localeCompare(right.name);
    if (sort === "kind") return left.media_kind.localeCompare(right.media_kind) || left.name.localeCompare(right.name);
    return left.name.localeCompare(right.name);
  });
}
