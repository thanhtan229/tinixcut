import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import { mockIPC } from '@tauri-apps/api/mocks';
import { SilenceCutDialog } from '../../../src/components/SilenceCutDialog';
import { MediaPanel } from '../../../src/components/MediaPanel';
import { buildSilenceCutOperations } from '../../../src/components/silenceCuts';
import '../../../src/App.css';
mockIPC((command,args)=>window.silenceTestInvoke(command,args));
// Browser-only replacement for Tauri's asset URL bridge, served by the harness.
window.__TAURI_INTERNALS__.convertFileSrc=()=>new URL('/fixture.wav',location.href).href;
const asset={asset_id:'source',name:'Synthetic voice.wav',path:'fixture.wav',media_kind:'Audio',duration:{value:90,rate:{numerator:30,denominator:1}}};
function Harness(){const [revision,setRevision]=useState(1);const [open,setOpen]=useState(false);window.silenceTestChangeProject=()=>setRevision(r=>r+1);return <>
 <button onClick={()=>setRevision(r=>r+1)}>Change project</button>
 <MediaPanel onLocateAsset={(_,direction)=>{window.mediaLocateDirection=direction;}} usageCounts={{source:revision===1?2:0}} assets={[asset]} onAddSampleAsset={()=>{throw new Error('No native picker in this test');}} onSuggestSilenceCuts={()=>setOpen(true)}/>
 {open&&<SilenceCutDialog asset={asset} projectId="test" revision={revision} rate={{numerator:30,denominator:1}} onClose={()=>{setOpen(false);window.silenceTestClosed();}}
 onApply={async(report,selected)=>{const verified=await window.silenceTestInvoke('media_silence_validate',{jobId:report.jobId});await window.silenceTestApply(buildSilenceCutOperations(asset,verified.analysis,selected,{numerator:30,denominator:1},60,42));}}/>}
 </>;}
createRoot(document.getElementById('root')).render(<Harness/>);
