import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {mockIPC} from '@tauri-apps/api/mocks';
import {SourceViewerDialog} from '../../../src/components/SourceViewerDialog';
import {MediaPanel} from '../../../src/components/MediaPanel';
mockIPC(()=>{});
window.__TAURI_INTERNALS__.convertFileSrc=()=>'/video.webm';
const params=new URLSearchParams(location.search);
const asset={asset_id:'test',name:'CC0 grasshopper',path:'video.webm',media_kind:params.has('image')?'Image':'Video',...(params.has('missing')||params.has('image')?{}:{duration:params.has('invalid')?{value:300,rate:{numerator:0,denominator:1}}:params.has('actual')?{value:19907,rate:{numerator:1000,denominator:1}}:{value:300,rate:{numerator:30,denominator:1}}})};
function Harness(){const [assets,setAssets]=useState([asset]);const [viewerId,setViewerId]=useState(asset.asset_id);window.updateSourceDuration=()=>setAssets(a=>a.map(x=>x.asset_id===asset.asset_id?{...x,duration:{value:300,rate:{numerator:30,denominator:1}}}:x));window.removeSource=()=>setAssets([]);window.captureSourceRange=null;return <><MediaPanel assets={assets} onAddSampleAsset={()=>{}} onViewAsset={a=>setViewerId(a.asset_id)}/><button onClick={()=>window.updateSourceDuration()}>Relink source</button><button onClick={()=>window.removeSource()}>Remove source</button><SourceViewerDialog asset={viewerId?assets.find(a=>a.asset_id===viewerId)??null:null} frameRate={{numerator:30000,denominator:1001}} onClose={()=>setViewerId(null)} onAddRange={(a,start,end)=>{window.captureSourceRange={id:a.asset_id,start,end};}}/></>;}
createRoot(document.getElementById('root')).render(<Harness/>);
