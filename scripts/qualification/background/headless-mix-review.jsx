import React from 'react';import {createRoot} from 'react-dom/client';import {mockIPC} from '@tauri-apps/api/mocks';import {AudioMixDraftDialog} from '../../../src/components/AudioMixDraftDialog';
const rate={numerator:30,denominator:1};const item=(id,kind,start,duration)=>({item_id:id,asset_id:id,kind,name:id,volume:1,embedded_audio_enabled:true,timeline_range:{start:{value:start,rate},duration:{value:duration,rate}},source_range:{start:{value:0,rate},duration:{value:duration,rate}},keyframes:[]});const project={project_id:'mix',revision:1,schema_version:16,settings:{frame_rate:rate},tracks:[{track_id:'v',name:'Voice',items:[item('voice','Video',90,90)]},{track_id:'m',name:'Music',items:[item('music','Audio',0,360)]}]};
if(new URLSearchParams(location.search).has('locked'))project.tracks[1].locked=true;
if(new URLSearchParams(location.search).has('voiceover'))project.tracks.push({track_id:'narration',name:'Narration',items:[item('voiceover','Audio',90,90)]});
if(new URLSearchParams(location.search).has('bulk')) {
 project.tracks[0].items.push(item('voice2','Video',180,90));
 project.tracks[1].items.push(item('music2','Audio',360,90));
 project.tracks.push({track_id:'locked',name:'Locked',locked:true,items:[item('lockedMusic','Audio',0,90)]});
}
mockIPC((cmd,args)=>window.mixInvoke(cmd,args));createRoot(document.getElementById('root')).render(<AudioMixDraftDialog isOpen project={project} offlineMedia={{}} onClose={()=>{}} onApply={r=>window.mixApply(r)}/>);
