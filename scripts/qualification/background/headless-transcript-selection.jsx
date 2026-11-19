import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {mockIPC} from '@tauri-apps/api/mocks';
import {AutoCaptionDialog} from '../../../src/components/AutoCaptionDialog';
import {buildTranscriptCutOperations} from '../../../src/components/transcriptCut';

mockIPC(() => ({legacyBrowserSettingsMigrated:true,transcriptionLanguage:'auto'}));
const rate={numerator:30,denominator:1};
const time=value=>({value,rate});
const segments=[{text:'Xin chào',sourceStart:time(0),sourceDuration:time(30)},{text:'Cảm ơn',sourceStart:time(60),sourceDuration:time(30)}];
const report={jobId:'test',state:'completed',progress:1,message:'Ready',transcript:segments};
const reportSource={projectId:'project',assetId:'source',assetName:'voice.wav',sourcePath:'voice.wav'};
const assets=[{asset_id:'source',name:'voice.wav',path:'voice.wav',media_kind:'Audio',file_size:1,duration:time(90)},{asset_id:'other',name:'other.wav',path:'other.wav',media_kind:'Audio',file_size:1}];
const otherSource={projectId:'project',assetId:'other',assetName:'other.wav',sourcePath:'other.wav'};
const otherReport={jobId:'other-job',state:'completed',progress:1,message:'Ready',transcript:[{text:'Bonjour',sourceStart:time(0),sourceDuration:time(30)},{text:'Merci',sourceStart:time(60),sourceDuration:time(30)}]};
window.draft=null;
function Harness(){const [other,setOther]=useState(false);return <><button onClick={()=>setOther(true)}>Load second transcript</button><AutoCaptionDialog isOpen selectedClipName="voice" projectId="project" reportSource={other?otherSource:reportSource} availableTranscriptAssets={assets} report={other?otherReport:report} onClose={()=>{}} onStart={async()=>{}} onCancel={async()=>{}} onSeekTranscript={()=>{}} onOpenSourceTranscript={()=>{}} onInsertTranscriptRange={()=>{}} onBuildTranscriptCut={async(assetId,ranges)=>{window.draft={assetId,ranges,operations:buildTranscriptCutOperations(assets.find(a=>a.asset_id===assetId),ranges,rate,60,42)};}} frameRate={rate}/></>}
createRoot(document.getElementById('root')).render(<Harness/>);
