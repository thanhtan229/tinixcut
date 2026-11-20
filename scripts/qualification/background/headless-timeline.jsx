import React, { useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { adjacentMarkerFrame, adjacentClipBoundary } from '../../../src/components/timelineNavigation';
import { setMark, exportRangeFor } from '../../../src/hooks/inOutRange';
import { Timeline } from '../../../src/components/Timeline';
import { useTimelineShortcuts } from '../../../src/hooks/useTimelineShortcuts';
import '../../../src/App.css';
import raw from './caption-merge.tcproj?raw';

function fixture(minutes, count) {
 const p=JSON.parse(raw);p.project_id=`navigation-${minutes}-${count}`;p.assets=[];
 p.markers=[{marker_id:'m1',label:'One',position:{value:30,rate:p.settings.frame_rate}},{marker_id:'m2',label:'Two',position:{value:90,rate:p.settings.frame_rate}}];
 p.tracks=[{...p.tracks[0],track_id:'one',locked:true,items:[]},{...p.tracks[0],track_id:'two',items:[]}];
 for(let i=0;i<count;i++){
  const duration=minutes*60*30/count;
  p.tracks[i%2].items.push({...JSON.parse(raw).tracks[0].items[0],item_id:`c${i}`,name:`Caption ${i}`,timeline_range:{start:{value:i*duration,rate:p.settings.frame_rate},duration:{value:duration,rate:p.settings.frame_rate}}});
 }
 if(count>0){
  // A reduced source timebase must still draw on the project's frame grid.
  const item=p.tracks[0].items[0];item.timeline_range.duration={value:item.timeline_range.duration.value/30,rate:{numerator:1,denominator:1}};
 }
 return p;
}
let mutations=0;
window.markerActions=[];
const rejectEdit=()=>{mutations++;throw new Error('Navigation must not edit the project');};
function Harness(){
 const [marks,setMarks]=useState({markIn:null,markOut:null});const [project,setProject]=useState(()=>fixture(30,300));const [selected,setSelected]=useState(new Set());const [playhead,setPlayhead]=useState(0);const [scope,setScope]=useState('all');const [markerQuery,setMarkerQuery]=useState('');const ref=useRef(null);
 useTimelineShortcuts({enabled:true,project,selectedItemIds:selected,primarySelectedItemId:[...selected][0]??null,playheadFrame:playhead,onMarkIn:()=>setMarks(m=>setMark(m,"in",playhead)),onMarkOut:()=>setMarks(m=>setMark(m,"out",playhead)),onClearIn:()=>setMarks(m=>({...m,markIn:null})),onClearOut:()=>setMarks(m=>({...m,markOut:null})),onMarkerNavigation:direction=>{const frame=adjacentMarkerFrame(project,playhead,direction,markerQuery);if(frame!==null){setPlayhead(frame);ref.current?.locateFrame(frame);}},onClipBoundaryNavigation:direction=>{const frame=adjacentClipBoundary(project,playhead,direction,scope==='selected_tracks'?selected:undefined);if(frame!==null){setPlayhead(frame);ref.current?.locateFrame(frame);}},onZoomIn:()=>ref.current?.zoomIn(),onZoomOut:()=>ref.current?.zoomOut()});
 window.timelineTest={snapshot:()=>({project:JSON.stringify(project),selected:[...selected],playhead,marks,mutations,exportRange:exportRangeFor(marks,54000)})};
 return <><div><button onClick={()=>{setProject(fixture(10,100));setSelected(new Set());}}>Ten minutes</button>
 <button onClick={()=>{setProject(fixture(30,300));setSelected(new Set());}}>Thirty minutes</button>
 <button onClick={()=>setSelected(new Set(['c298','c299','stale']))}>Select late clips</button>
 <button onClick={()=>setSelected(new Set(['stale']))}>Stale selection</button>
 <button onClick={()=>{const p=fixture(0,0);p.markers=[];setProject(p);setSelected(new Set());}}>Empty project</button>
 <button onClick={()=>{const p=fixture(30,300);p.markers=[{...p.markers[1],label:'Same'},{...p.markers[0],label:'Same'}];setProject(p);}}>Duplicate markers</button>
 <button onClick={()=>{const p=fixture(1,1);p.project_id='single-frame';p.tracks[0].items[0].timeline_range.duration={value:1,rate:p.settings.frame_rate};setProject(p);setSelected(new Set(['c0']));}}>Single frame</button>
 <button onClick={()=>{const p=fixture(1,4);for(const [item,start,duration] of [[p.tracks[0].items[0],0,30],[p.tracks[0].items[1],90,30],[p.tracks[1].items[0],45,15],[p.tracks[1].items[1],150,30]]){item.timeline_range={start:{value:start,rate:p.settings.frame_rate},duration:{value:duration,rate:p.settings.frame_rate}};}setProject(p);setSelected(new Set(['c0']));setPlayhead(30);setScope('all');}}>Boundary scope fixture</button>
 <button onClick={()=>{setProject(p=>({...p,settings:{...p.settings,frame_rate:{numerator:30000,denominator:1001}}}));setMarks({markIn:30000,markOut:36000});}}>Fractional marks</button><button onClick={()=>setMarks({markIn:53900,markOut:54100})}>Beyond timeline marks</button><button onClick={()=>setMarks({markIn:54100,markOut:54200})}>Outside timeline marks</button><button onClick={()=>setMarks({markIn:30,markOut:null})}>Partial marks</button><button onClick={()=>setPlayhead(53800)}>Late playhead</button><button onClick={()=>setPlayhead(0)}>Reset test playhead</button>
 <button onClick={()=>setMarks({markIn:30000,markOut:36000})}>Set test marks</button><button onClick={()=>setMarks({markIn:null,markOut:null})}>Clear test marks</button>
 <input aria-label="Typing guard" /></div>
 <Timeline project={project} exportTimelineFrames={54000} markerQuery={markerQuery} onMarkerQueryChange={setMarkerQuery} markIn={marks.markIn} markOut={marks.markOut} playheadFrame={playhead} selectedItemIds={selected} clipBoundaryScope={scope} onClipBoundaryScopeChange={setScope} navigationRef={ref}
 onSelectItem={(id,ctrl)=>setSelected(s=>ctrl?new Set([...s,id]):new Set(id?[id]:[]))} onClearSelection={()=>setSelected(new Set())}
 onPlayheadChange={setPlayhead} onMoveClip={rejectEdit} onTrimClip={rejectEdit} onSplitClip={rejectEdit} onDeleteClip={rejectEdit} onDuplicateClip={rejectEdit} onRenameMarker={(id,label)=>window.markerActions.push(['rename',id,label])} onDeleteMarker={id=>window.markerActions.push(['delete',id])}/></>;
}
createRoot(document.getElementById('root')).render(<Harness/>);
