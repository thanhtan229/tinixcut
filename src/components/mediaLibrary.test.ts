import { describe, expect, it } from "vitest";
import { Asset } from "../types/project";
import { filterAndSortMediaAssets, sourceRangeSecondsToFrames } from "./mediaLibrary";

const assets: Asset[] = [
  { asset_id: "video-z", name: "Zebra.mp4", path: "z", media_kind: "Video", file_size: 900 },
  { asset_id: "audio-a", name: "Ambience.wav", path: "a", media_kind: "Audio", file_size: 120 },
  { asset_id: "image-b", name: "Beach.jpg", path: "b", media_kind: "Image", file_size: 300 },
];

describe("filterAndSortMediaAssets", () => {
  it("searches names without changing the source asset array", () => {
    const result = filterAndSortMediaAssets(assets, new Set(), "  BEACH ", "all", "all", "name");
    expect(result.map((asset) => asset.asset_id)).toEqual(["image-b"]);
    expect(assets.map((asset) => asset.asset_id)).toEqual(["video-z", "audio-a", "image-b"]);
  });

  it("combines type and online status filters and sorts by size", () => {
    const result = filterAndSortMediaAssets(assets, new Set(["audio-a"]), "", "Video", "online", "size");
    expect(result.map((asset) => asset.asset_id)).toEqual(["video-z"]);
  });

  it("shows offline media in their explicit status filter", () => {
    const result = filterAndSortMediaAssets(assets, new Set(["audio-a", "video-z"]), "", "all", "offline", "kind");
    expect(result.map((asset) => asset.asset_id)).toEqual(["audio-a", "video-z"]);
  });

  it("maps source In/Out to the project frame grid and rejects invalid ranges", () => {
    expect(sourceRangeSecondsToFrames(1.25, 3.5, { numerator: 30, denominator: 1 })).toEqual({ start: 38, duration: 67 });
    expect(sourceRangeSecondsToFrames(2, 2, { numerator: 30, denominator: 1 })).toBeNull();
    expect(sourceRangeSecondsToFrames(Number.NaN, 3, { numerator: 30, denominator: 1 })).toBeNull();
  });
});
