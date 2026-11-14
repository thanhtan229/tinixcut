import { rationalTimeToSeconds, TimelineItem } from "../types/project";

/** A reversed preview file generated for one clip's source range (disposable cache). */
export interface ReverseProxySource {
  /** Identifies the clip's asset, source range and path the file was built for. */
  key: string;
  path: string;
}

/**
 * Changes whenever the clip's source range or media changes, so an edit never keeps showing an
 * outdated reversed file. Speed is not part of the key: the proxy covers the whole source range.
 */
export function reverseProxyKey(item: TimelineItem, assetPath: string | null | undefined): string {
  const range = item.source_range;
  return [
    item.item_id,
    item.asset_id ?? "",
    assetPath ?? "",
    range.start.value,
    range.duration.value,
    range.start.rate.numerator,
    range.start.rate.denominator,
  ].join(":");
}

/**
 * Time to show in the reversed file. The file starts at the end of the clip's source range and
 * runs backwards, so at timeline offset `o` it shows `o x speed`, which is the forward source
 * time minus the range start. This is the same source instant the export shows.
 */
export function reverseProxySeconds(forwardSourceSeconds: number, item: TimelineItem): number {
  const start = rationalTimeToSeconds(item.source_range.start);
  const duration = rationalTimeToSeconds(item.source_range.duration);
  return Math.min(Math.max(0, forwardSourceSeconds - start), duration);
}

/** Reversed video clips that are on screen and need a reversed file. */
export function reversedVideoItems(
  items: readonly TimelineItem[],
  activeItemIds: ReadonlySet<string>,
): TimelineItem[] {
  return items.filter((item) => item.kind === "Video" && item.reverse && activeItemIds.has(item.item_id));
}
