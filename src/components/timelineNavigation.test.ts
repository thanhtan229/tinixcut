import { describe, expect, it } from "vitest";
import { adjacentMarkerFrame, adjacentClipBoundary, anchoredTimelineScroll, fitTimelineRange, stepTimelineZoom, timelineRulerTicks, timelineViewRange, nextMarkerLabel, markerTimeLabel, TIMELINE_HEADER_WIDTH } from "./timelineNavigation";
import { Project } from "../types/project";

const project = { settings: { frame_rate: { numerator: 30, denominator: 1 } }, tracks: [
  { locked: true, items: [{ item_id: "a", timeline_range: { start: { value: 2, rate: { numerator: 1, denominator: 1 } }, duration: { value: 3, rate: { numerator: 1, denominator: 1 } } } }] },
  { items: [{ item_id: "b", timeline_range: { start: { value: 18000, rate: { numerator: 30, denominator: 1 } }, duration: { value: 60, rate: { numerator: 30, denominator: 1 } } } }] },
] } as Project;

describe("timeline view geometry", () => {
  it("navigates distinct clip boundaries across locked tracks and timebases", () => {
    const before=JSON.stringify(project);
    expect(adjacentClipBoundary(project,0,1)).toBe(60);
    expect(adjacentClipBoundary(project,60,1)).toBe(150);
    expect(adjacentClipBoundary(project,150,1)).toBe(18000);
    expect(adjacentClipBoundary(project,18000,-1)).toBe(150);
    expect(adjacentClipBoundary(project,18060,1)).toBeNull();
    expect(adjacentClipBoundary(project,60,-1)).toBeNull();
    expect(adjacentClipBoundary(project,150,1,new Set(["a"]))).toBeNull();
    expect(adjacentClipBoundary(project,0,1,new Set(["b"]))).toBe(18000);
    expect(adjacentClipBoundary(project,60,1,new Set(["a","b"]))).toBe(150);
    expect(adjacentClipBoundary(project,0,1,new Set(["missing"]))).toBeNull();
    expect(adjacentClipBoundary(project,0,1,new Set())).toBeNull();
    expect(adjacentClipBoundary(null,0,1)).toBeNull();
    expect(JSON.stringify(project)).toBe(before);
  });
  it("labels markers with elapsed time and the exact frame at integer and fractional rates", () => {
    expect(markerTimeLabel(8700, { numerator: 30, denominator: 1 })).toBe("04:50 (8700f)");
    expect(markerTimeLabel(30, { numerator: 30000, denominator: 1001 })).toBe("00:01.001 (30f)");
  });
  it("uses actual frame grids across locked tracks and ignores stale selected IDs without mutation", () => {
    const before = JSON.stringify(project);
    expect(timelineViewRange(project)).toEqual({ start: 0, end: 18060 });
    expect(timelineViewRange(project, new Set(["a", "missing"]))).toEqual({ start: 60, end: 150 });
    expect(timelineViewRange(project, new Set(["a", "b"]))).toEqual({ start: 60, end: 18060 });
    expect(timelineViewRange(project, new Set(["missing"]))).toBeNull();
    expect(timelineViewRange(null)).toBeNull();
    expect(JSON.stringify(project)).toBe(before);
  });
  it.each([1800, 18000, 54000])("fits a %i-frame edit in narrow and wide viewports", end => {
    for (const width of [360, 720, 1440]) {
      const view = fitTimelineRange({ start: 0, end }, width);
      expect(view.zoom).toBeGreaterThan(0);
      expect(end * view.zoom + TIMELINE_HEADER_WIDTH - view.scrollLeft).toBeLessThan(width - 8);
      expect(view.scrollLeft).toBe(0);
    }
  });
  it("fits late selections and bounds empty/single-frame views", () => {
    const view = fitTimelineRange({ start: 50000, end: 51000 }, 720);
    expect(50000 * view.zoom + 160 - view.scrollLeft).toBeGreaterThanOrEqual(176);
    expect(51000 * view.zoom + 160 - view.scrollLeft).toBeLessThanOrEqual(704);
    expect(fitTimelineRange(null, 720)).toEqual({ zoom: 3, scrollLeft: 0 });
    expect(fitTimelineRange({ start: 0, end: 1 }, 720).zoom).toBe(10);
    expect(fitTimelineRange({ start: 0, end: 30 }, 0).zoom).toBeGreaterThan(0);
  });
  it("retains a visible playhead over repeated zoom cycles", () => {
    let zoom = 0.03, scroll = 100;
    const frame = 10000, x = frame * zoom - scroll;
    for (const direction of [1, 1, 1, -1, -1, -1] as const) {
      const next = stepTimelineZoom(zoom, direction);
      scroll = anchoredTimelineScroll(zoom, next, scroll, 1000, frame); zoom = next;
      expect(Math.abs(frame * zoom - scroll - x)).toBeLessThan(1);
    }
  });
  it("anchors the viewport center when the playhead is offscreen", () => {
    expect(anchoredTimelineScroll(1, 2, 500, 960, 0)).toBe(1400);
    expect(anchoredTimelineScroll(2, 0.01, 0, 960, 10)).toBe(0);
  });
});

describe("visible adaptive ruler", () => {
  it.each([25, 30, 60])("places the one-second label at frame %i", fps => {
    const ticks = timelineRulerTicks({ numerator: fps, denominator: 1 }, 4, 0, 1280, 10000);
    expect(ticks.find(t => t.f === fps)?.label).toBe(`00:01 (${fps}f)`);
  });
  it("derives fractional-rate labels from absolute frame times", () => {
    const ticks = timelineRulerTicks({ numerator: 30000, denominator: 1001 }, 4, 0, 1280, 10000);
    expect(ticks.find(t => t.f === 30)?.label).toBe("00:01.001 (30f)");
    expect(ticks.find(t => t.f === 60)?.label).toBe("00:02.002 (60f)");
  });
  it("bounds ticks for 30-minute overviews and scrolled detail views", () => {
    for (const width of [360, 720, 1440]) for (const [zoom, scroll] of [[0.006, 0], [3, 150000]]) {
      const ticks = timelineRulerTicks({ numerator: 30, denominator: 1 }, zoom, scroll, width, 54000);
      expect(ticks.length).toBeLessThan(width / 8 + 100);
      expect(ticks.filter(t => t.label).length).toBeLessThan(width / 70 + 5);
      expect(ticks.every(t => Number.isInteger(t.f))).toBe(true);
      expect(new Set(ticks.map(t => t.f)).size).toBe(ticks.length);
    }
  });
});


describe("marker navigation",()=>{
 it("chooses an unused default label after deletion or rename",()=>{
  expect(nextMarkerLabel([])).toBe("Marker 1");
  expect(nextMarkerLabel([{label:"Marker 2"},{label:" marker 1 "},{label:"Custom"}])).toBe("Marker 3");
  expect(nextMarkerLabel([{label:"Marker 1"},{label:"Marker 3"}])).toBe("Marker 2");
 });
 it("uses nearest strictly earlier or later marker on the project frame grid without wrapping or mutation",()=>{
  const marked={...project,markers:[{marker_id:"a",label:"A",position:{value:2,rate:{numerator:1,denominator:1}}},{marker_id:"b",label:"B",position:{value:15,rate:{numerator:30,denominator:1}}},{marker_id:"c",label:"C",position:{value:60,rate:{numerator:30,denominator:1}}}]};
  const before=JSON.stringify(marked);
  expect(adjacentMarkerFrame(marked,15,1)).toBe(60);
  expect(adjacentMarkerFrame(marked,60,-1)).toBe(15);
  expect(adjacentMarkerFrame(marked,60,1)).toBeNull();
  expect(adjacentMarkerFrame(marked,15,-1)).toBeNull();
  expect(adjacentMarkerFrame(null,0,1)).toBeNull();
  expect(JSON.stringify(marked)).toBe(before);
 });
});
