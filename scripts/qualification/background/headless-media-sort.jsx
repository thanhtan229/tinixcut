import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {MediaPanel} from '../../../src/components/MediaPanel';
import {nextMediaOccurrence,selectedMediaOccurrence,mediaUsageCounts} from '../../../src/components/mediaLibrary';
const rate={numerator:30,denominator:1};
const assets=Array.from({length:120},(_,i)=>({asset_id:`a${i}`,name:`Source ${String(i).padStart(3,'0')}`,path:`fixture-${i}`,media_kind:['Video','Audio','Image'][i%3],file_size:1,...(i%3===2?{}:{duration:{value:(i+1)*30,rate}})}));
if(new URLSearchParams(location.search).has('duplicates')){assets[0].name=assets[1].name='interview.mp4';assets[0].path='C:\\Day One\\interview.mp4';assets[1].path='D:\\Day Two\\interview.mp4';}
const project={settings:{frame_rate:rate},tracks:[{name:'Main',items:[{item_id:'one',asset_id:'a30',timeline_range:{start:{value:0,rate}}},{item_id:'two',asset_id:'a30',timeline_range:{start:{value:90,rate}}}]}]};
window.fixtureProject=()=>JSON.stringify(project);
function App(){const [selected,setSelected]=useState(null);const [offline,setOffline]=useState(false);const [usage,setUsage]=useState(2);window.setFixtureOffline=setOffline;window.setFixtureUsage=setUsage;const tracks=[{...project.tracks[0],items:project.tracks[0].items.slice(0,usage)}];return <MediaPanel assets={assets} offlineMedia={offline?{a30:{last_known_path:'fixture-30',permission_required:false}}:{}} usageCounts={mediaUsageCounts(tracks)} locatedOccurrence={selectedMediaOccurrence(project,selected)} onLocateAsset={(asset,direction)=>{const next=nextMediaOccurrence(project,asset.asset_id,selected,direction);setSelected(next?.itemId??null);window.lastLocate=next;}} onAddSampleAsset={()=>{}}/>}
createRoot(document.getElementById('root')).render(<App/>);
