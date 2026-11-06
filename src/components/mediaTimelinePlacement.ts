import { Asset, Project, Track } from "../types/project";

export interface MediaTimelinePlacement {
  track: Track | null;
  startFrame: number;
  insertBeforeIndex: number | null;
}

export function getMediaTimelinePlacement(project: Project, asset: Asset): MediaTimelinePlacement {
  const isAudio = asset.media_kind === "Audio";
  const trackKind = isAudio ? "Audio" : "Video";
  const mediaItemKind = isAudio ? "Audio" : asset.media_kind;
  const compatibleTracks = project.tracks.filter((track) => track.kind === trackKind && !track.muted && !track.locked);

  const mediaTrack = compatibleTracks.find((track) =>
    track.items.some((item) => item.kind === mediaItemKind)
  );
  const emptyTrack = compatibleTracks.find((track) => track.items.length === 0);
  const track = mediaTrack ?? emptyTrack ?? null;
  const startFrame = track
    ? track.items.reduce((endFrame, item) => {
        const itemStart = item.timeline_range.start.value;
        const itemEnd = itemStart + item.timeline_range.duration.value;
        return Math.max(endFrame, itemEnd);
      }, 0)
    : 0;

  // Visual tracks are composed bottom-to-top. A newly added video track must sit
  // below text-only tracks so that captions remain visible over the footage.
  const insertBeforeIndex = !isAudio && !track
    ? project.tracks.findIndex((candidate) =>
        candidate.kind === "Video" && candidate.items.some((item) => item.kind === "Text")
      )
    : null;

  return {
    track,
    startFrame,
    insertBeforeIndex: insertBeforeIndex !== null && insertBeforeIndex >= 0 ? insertBeforeIndex : null,
  };
}
