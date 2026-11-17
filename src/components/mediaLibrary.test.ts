import { describe, expect, it } from "vitest";
import { Asset, Project } from "../types/project";
import { filterAndSortMediaAssets, sourceRangeSecondsToFrames, mediaUsageCounts, nextMediaOccurrence, selectedMediaOccurrence } from "./mediaLibrary";

describe("mediaUsageCounts", () => {
  it("counts occurrences across tracks by identity and recomputes after history changes", () => {
    const tracks = [{items:[{asset_id:"a"},{asset_id:"a"},{asset_id:null}]},{items:[{asset_id:"b"},{asset_id:"a"}]}] as Project["tracks"];
    expect({...mediaUsageCounts(tracks)}).toEqual({a:3,b:1});
    const prior = tracks.slice(0,1);
    expect({...mediaUsageCounts(prior)}).toEqual({a:2});
    expect({...mediaUsageCounts(tracks)}).toEqual({a:3,b:1});
    expect({...mediaUsageCounts([])}).toEqual({});
  });
});

const assets: Asset[] = [
  { asset_id: "video-z", name: "Zebra.mp4", path: "z", media_kind: "Video", file_size: 900 },
  { asset_id: "audio-a", name: "Ambience.wav", path: "a", media_kind: "Audio", file_size: 120 },
  { asset_id: "image-b", name: "Beach.jpg", path: "b", media_kind: "Image", file_size: 300 },
];

describe("filterAndSortMediaAssets", () => {
  it("sorts durations across timebases and keeps unknown or invalid values last in both directions", () => {
    const timed: Asset[] = [
      {...assets[0], asset_id:"short", duration:{value:30,rate:{numerator:30,denominator:1}}},
      {...assets[1], asset_id:"long", duration:{value:30,rate:{numerator:30000,denominator:1001}}},
      assets[2],
      {...assets[0], asset_id:"invalid", name:"Invalid", duration:{value:10,rate:{numerator:0,denominator:1}}},
    ];
    const before=structuredClone(timed);
    const sorted=(sort:"duration-asc"|"duration-desc")=>filterAndSortMediaAssets(timed,new Set(),"","all","all",sort).map(a=>a.asset_id);
    expect(sorted("duration-asc")).toEqual(["short","long","image-b","invalid"]);
    expect(sorted("duration-desc")).toEqual(["long","short","image-b","invalid"]);
    expect(filterAndSortMediaAssets(timed,new Set(["long"]),"","all","online","duration-desc").map(a=>a.asset_id)).toEqual(["short","image-b","invalid"]);
    expect(timed).toEqual(before);
  });

  it("breaks equal-duration ties deterministically by name then asset identity", () => {
    const a={...assets[0],asset_id:"b",name:"Same",duration:{value:60,rate:{numerator:30,denominator:1}}};
    const b={...a,asset_id:"a",duration:{value:50,rate:{numerator:25,denominator:1}}};
    expect(filterAndSortMediaAssets([a,b],new Set(),"","Video","all","duration-desc").map(a=>a.asset_id)).toEqual(["a","b"]);
  });
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


describe("nextMediaOccurrence",()=>{
  it("orders by timeline time, skips other sources, wraps and leaves project unchanged",()=>{
    const rate={numerator:30,denominator:1};
    const project={settings:{frame_rate:rate},tracks:[{items:[{item_id:"late",asset_id:"a",timeline_range:{start:{value:90,rate}}},{item_id:"other",asset_id:"b",timeline_range:{start:{value:0,rate}}}]},{items:[{item_id:"early",asset_id:"a",timeline_range:{start:{value:10,rate}}}]}]} as Project;
    project.tracks[0].name="Voice"; project.tracks[1].name="B-roll";
    const before=JSON.stringify(project);
    expect(selectedMediaOccurrence(project,"late")).toEqual({assetId:"a",index:2,total:2,trackName:"Voice"});
    expect(selectedMediaOccurrence(project,"deleted")).toBeNull();
    expect(nextMediaOccurrence(project,"a",null)).toEqual({itemId:"early",trackName:"B-roll",frame:10,index:0,total:2});
    expect(nextMediaOccurrence(project,"a","early")?.itemId).toBe("late");
    expect(nextMediaOccurrence(project,"a","late")?.itemId).toBe("early");
    expect(nextMediaOccurrence(project,"missing",null)).toBeNull();
    expect(nextMediaOccurrence(project,"a","deleted")?.itemId).toBe("early");
    expect(nextMediaOccurrence(project,"a","early",-1)?.itemId).toBe("late");
    expect(nextMediaOccurrence(project,"a","late",-1)?.itemId).toBe("early");
    expect(nextMediaOccurrence(project,"a",null,-1)?.itemId).toBe("late");
    expect(nextMediaOccurrence(project,"a","deleted",-1)?.itemId).toBe("late");
    expect(nextMediaOccurrence(project,"b","other",-1)?.itemId).toBe("other");
    expect(nextMediaOccurrence(project,"missing",null,-1)).toBeNull();
    expect(JSON.stringify(project)).toBe(before);
  });
});
