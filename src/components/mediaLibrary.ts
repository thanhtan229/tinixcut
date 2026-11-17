import { Asset, FrameRate, Project, rationalTimeToFrames } from "../types/project";

export function mediaOccurrences(project: Project, assetId: string) {
  return project.tracks.flatMap(track => track.items.map(item=>({item,trackName:track.name})))
    .filter(({item}) => item.asset_id === assetId)
    .map(({item,trackName}) => ({ itemId: item.item_id, trackName, frame: rationalTimeToFrames(item.timeline_range.start, project.settings.frame_rate) }))
    .sort((a,b)=>a.frame-b.frame || a.itemId.localeCompare(b.itemId));
}

export function nextMediaOccurrence(project: Project, assetId: string, currentItemId: string | null, direction: 1 | -1 = 1) {
  const items = mediaOccurrences(project, assetId);
  const index = items.findIndex(item=>item.itemId===currentItemId);
  if (!items.length) return null;
  const target = index < 0 ? (direction === 1 ? 0 : items.length-1) : (index+direction+items.length)%items.length;
  return { ...items[target], index: target, total: items.length };
}

export function selectedMediaOccurrence(project: Project, currentItemId: string | null) {
  const item = project.tracks.flatMap(track=>track.items).find(item=>item.item_id===currentItemId);
  if (!item?.asset_id) return null;
  const items=mediaOccurrences(project,item.asset_id);
  const index=items.findIndex(occurrence=>occurrence.itemId===currentItemId);
  return {assetId:item.asset_id,index:index+1,total:items.length,trackName:items[index].trackName};
}

export function mediaUsageCounts(tracks: Project["tracks"]): Record<string, number> {
  const counts: Record<string, number> = Object.create(null);
  for (const track of tracks) for (const item of track.items) {
    if (item.asset_id) counts[item.asset_id] = (counts[item.asset_id] ?? 0) + 1;
  }
  return counts;
}

export type MediaLibraryKindFilter = "all" | Asset["media_kind"];
export type MediaLibraryStatusFilter = "all" | "online" | "offline";
export type MediaLibrarySort = "name" | "kind" | "size" | "duration-asc" | "duration-desc";

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
    if (sort === "duration-asc" || sort === "duration-desc") {
      const duration = (asset: Asset) => {
        const time = asset.duration;
        return time && Number.isSafeInteger(time.value) && time.value > 0 && Number.isSafeInteger(time.rate.numerator) && time.rate.numerator > 0 && Number.isSafeInteger(time.rate.denominator) && time.rate.denominator > 0 ? time : null;
      };
      const a = duration(left), b = duration(right);
      if (!a || !b) return a ? -1 : b ? 1 : left.name.localeCompare(right.name) || left.asset_id.localeCompare(right.asset_id);
      const x = BigInt(a.value) * BigInt(a.rate.denominator) * BigInt(b.rate.numerator);
      const y = BigInt(b.value) * BigInt(b.rate.denominator) * BigInt(a.rate.numerator);
      const order = x < y ? -1 : x > y ? 1 : 0;
      return order * (sort === "duration-asc" ? 1 : -1) || left.name.localeCompare(right.name) || left.asset_id.localeCompare(right.asset_id);
    }
    if (sort === "size") return right.file_size - left.file_size || left.name.localeCompare(right.name);
    if (sort === "kind") return left.media_kind.localeCompare(right.media_kind) || left.name.localeCompare(right.name);
    return left.name.localeCompare(right.name);
  });
}
