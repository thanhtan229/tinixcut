import { RefObject, useLayoutEffect, useState } from "react";
import { anchoredTimelineScroll, clampTimelineZoom, DEFAULT_TIMELINE_ZOOM, fitTimelineRange, stepTimelineZoom, TimelineViewRange } from "../components/timelineNavigation";

export interface TimelineNavigationHandle { zoomIn: () => void; zoomOut: () => void; locateFrame: (frame: number) => void }
type FitMode = "manual" | "all" | "selection" | "inout";

export function useTimelineNavigation(container: RefObject<HTMLDivElement | null>, all: TimelineViewRange | null, selection: TimelineViewRange | null, playhead: number, projectId?: string, inout: TimelineViewRange | null = null) {
  const [view, setView] = useState({ zoom: DEFAULT_TIMELINE_ZOOM, scrollLeft: 0, width: 0, mode: "manual" as FitMode });
  useLayoutEffect(() => {
    const element = container.current;
    if (!element) return;
    const measure = () => setView(v => v.width === element.clientWidth ? v : { ...v, width: element.clientWidth });
    measure();
    const observer = new ResizeObserver(measure); observer.observe(element);
    return () => observer.disconnect();
  }, [container]);
  useLayoutEffect(() => {
    setView(v => ({ ...v, zoom: DEFAULT_TIMELINE_ZOOM, scrollLeft: 0, mode: "manual" }));
  }, [projectId]);
  const fittedRange = view.mode === "selection" ? selection : view.mode === "inout" ? inout : all;
  useLayoutEffect(() => {
    if (view.mode === "manual" || view.width <= 0) return;
    if (view.mode === "selection" && !selection) { setView(v => ({ ...v, mode: "manual" })); return; }
    if (view.mode === "inout" && !inout) { setView(v => ({ ...v, mode: "manual" })); return; }
    const fit = fitTimelineRange(fittedRange, view.width);
    setView(v => v.zoom === fit.zoom && v.scrollLeft === fit.scrollLeft ? v : { ...v, ...fit });
  }, [view.mode, view.width, fittedRange?.start, fittedRange?.end, selection]);
  useLayoutEffect(() => {
    if (container.current) container.current.scrollLeft = view.scrollLeft;
  }, [container, view.scrollLeft, view.zoom, view.width]);

  const manualZoom = (requested: number | ((old: number) => number)) => setView(v => {
    const zoom = clampTimelineZoom(typeof requested === "function" ? requested(v.zoom) : requested);
    const scrollLeft = anchoredTimelineScroll(v.zoom, zoom, container.current?.scrollLeft ?? v.scrollLeft, v.width, playhead);
    return { ...v, zoom, scrollLeft, mode: "manual" };
  });
  return {
    locateFrame: (frame: number) => setView(v=>({...v,mode:"manual",scrollLeft:Math.max(0,frame*v.zoom-Math.max(0,v.width-160)/2)})),
    ...view,
    zoomIn: () => manualZoom(z => stepTimelineZoom(z, 1)),
    zoomOut: () => manualZoom(z => stepTimelineZoom(z, -1)),
    setZoom: (zoom: number) => manualZoom(zoom),
    fitAll: () => setView(v => ({ ...v, ...fitTimelineRange(all, v.width), mode: "all" })),
    fitSelection: () => { if (selection) setView(v => ({ ...v, ...fitTimelineRange(selection, v.width), mode: "selection" })); },
    fitInOut: () => { if (inout) setView(v => ({ ...v, ...fitTimelineRange(inout, v.width), mode: "inout" })); },
    onScroll: (scrollLeft: number) => setView(v => Math.abs(v.scrollLeft - scrollLeft) < 0.5 ? v : { ...v, scrollLeft, mode: "manual" }),
  };
}
