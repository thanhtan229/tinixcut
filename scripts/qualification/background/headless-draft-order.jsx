import React,{useState} from 'react';
import {createRoot} from 'react-dom/client';
import {mockIPC} from '@tauri-apps/api/mocks';
import {EditPlanDraftDialog} from '../../../src/components/EditPlanDraftDialog';
import raw from './caption-merge.tcproj?raw';
mockIPC((command,args)=>window.draftInvoke(command,args));
function Harness(){const [offline,setOffline]=useState({});const [assets,setAssets]=useState(['a','b','c']);const [revision,setRevision]=useState(0);const project=JSON.parse(raw);project.revision=revision;project.assets=assets.map(id=>({asset_id:id,name:({a:'Misty river',b:'Flowing stream',c:'Grasshopper'})[id],media_kind:'Video',path:id,duration:{value:300,rate:project.settings.frame_rate},file_size:1}));return <><button onClick={()=>setOffline({b:{last_known_path:'b',permission_required:false}})}>Offline b</button><button onClick={()=>setOffline({})}>Online all</button><button onClick={()=>setOffline(current=>({...current}))}>Refresh media</button><button onClick={()=>setAssets(['a','b'])}>Remove c</button><button onClick={()=>setRevision(value=>value+1)}>Change revision</button><EditPlanDraftDialog isOpen project={project} offlineMedia={offline} onClose={()=>{}} onApply={async receipt=>window.captureDraft(receipt)}/></>};
createRoot(document.getElementById('root')).render(<Harness/>);
