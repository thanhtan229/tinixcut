import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { CaptionEditorDialog } from "../../../src/components/CaptionEditorDialog";
import { buildCaptionMergeOperations, buildCaptionTimingOperations } from "../../../src/components/captionEditor";
import fixture from "./caption-merge.tcproj?raw";

function Harness() {
  const [project, setProject] = useState(() => {
    const initial = JSON.parse(fixture);
    const scenario = new URLSearchParams(window.location.search).get("scenario");
    if (scenario === "gap") initial.tracks[0].items[1].timeline_range.start.value += 1;
    if (scenario === "locked") initial.tracks[0].locked = true;
    if (scenario === "tracks") {const track=structuredClone(initial.tracks[0]);track.track_id="other-track";track.name="Other language";track.items.forEach(item=>{item.item_id="other-"+item.item_id;});initial.tracks.push(track);}
    if (scenario === "long" || scenario === "long-valid") {
      const track = initial.tracks[0];
      const template = track.items[0];
      track.items = Array.from({length: 200}, (_, index) => {
        const entry = structuredClone(template);
        entry.item_id = `long-${index}`;
        entry.text_content = `Caption ${index}`;
        entry.timeline_range.start.value = index * 30 + (index >= 120 ? 5 : 0) - (scenario === "long" && index === 170 ? 5 : 0);
        entry.timeline_range.duration.value = scenario === "long" && index === 170 ? 35 : 30;
        entry.source_range.duration.value = entry.timeline_range.duration.value;
        return entry;
      });
      const other = structuredClone(track);
      other.track_id = "other-track";
      other.name = "Other language";
      other.items = other.items.slice(0, 2).map((entry, index) => ({...entry, item_id:`other-${index}`}));
      initial.tracks.push(other);
    }
    return initial;
  });
  return <CaptionEditorDialog project={project} playheadFrame={0} isOpen={true}
    onClose={() => {}} onSeek={() => {}}
    onApplyEdits={async (text,timing) => { const scenario=new URLSearchParams(location.search).get("scenario"); if(!["tracks", "long", "long-valid"].includes(scenario)) throw new Error("Not implemented in this test harness"); await window.captureCaptionDrafts(text,timing); if(scenario === "long-valid") await window.captureCaptionCommand({project,operations:[...text.map(({item_id,text},i)=>({operation_id:`caption-text-${i}`,operation:{type:"ClipTextSet",payload:{item_id,text}}})),...buildCaptionTimingOperations(project,timing,1)]}); }}
    onSplit={async () => { throw new Error("Not implemented in this test harness"); }}
    onMerge={async (left, right) => {
      const operations = buildCaptionMergeOperations(project, left, right);
      setProject(await window.applyQualificationMerge(operations));
    }} />;
}

createRoot(document.getElementById("root")).render(<Harness />);
