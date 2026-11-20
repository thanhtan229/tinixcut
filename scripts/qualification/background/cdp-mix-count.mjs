import {chromium} from 'playwright';
import {execFileSync} from 'node:child_process';
import {writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
const path=process.env.TINIXCUT_CDP_PROJECT+'.mix-count.wav';
execFileSync(resolve('src-tauri/resources/bin/ffmpeg.exe'),['-v','error','-y','-f','lavfi','-i','anullsrc=r=8000:cl=mono','-t','6','-c:a','pcm_s16le',path],{windowsHide:true,timeout:30000});
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});
const report={scope:'Audio mix selection counts in isolated WebView',steps:[]};const timer=setTimeout(()=>process.exit(124),60000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const get=()=>invoke('project_get_current');const asset=await invoke('media_import',{filePath:path});const seed=await get();const t=value=>({value,rate:seed.settings.frame_rate});
 const ops=[];for(const [track,id,name,start,duration] of [['a','voice','Voice clip',30,60],['b','music1','Same music',0,180],['c','music2','Same music',0,180],['d','voice2','Second voice',120,30]]){ops.push({type:'TrackCreate',payload:{track_id:track,name:track,kind:'Audio'}},{type:'ClipCreate',payload:{track_id:track,item_id:id,name,kind:'Audio',asset_id:asset.asset_id,timeline_start:t(start),duration:t(duration),source_start:t(0)}});}
 await invoke('project_execute_command',{envelope:{schema_version:seed.schema_version,project_id:seed.project_id,base_revision:seed.revision,transaction_id:'mix_count_seed',operations:ops.map((operation,i)=>({operation_id:`mix_count_${i}`,operation}))}});
 await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 await page.getByRole('button',{name:'Audio mix',exact:true}).click();const dialog=page.getByRole('dialog',{name:'Review audio mix plan'});await dialog.waitFor();
 const voice=dialog.getByRole('group',{name:'Voice clips'}),music=dialog.getByRole('group',{name:'Music clips'});
 await voice.getByText('0 voice clips selected',{exact:true}).waitFor();await music.getByText('4 music clips selected',{exact:true}).waitFor();
 const voiceRole=voice.getByRole('checkbox',{name:/Voice clip/}),musicRole=music.getByRole('checkbox',{name:/Voice clip/});await voiceRole.check();await voice.getByText('1 voice clip selected',{exact:true}).waitFor();await music.getByText('3 music clips selected',{exact:true}).waitFor();assert.equal(await musicRole.isChecked(),false);
 await musicRole.check();await voice.getByText('0 voice clips selected',{exact:true}).waitFor();await music.getByText('4 music clips selected',{exact:true}).waitFor();await voiceRole.check();
 await voice.getByRole('checkbox',{name:/Second voice/}).check();await voice.getByText('2 voice clips selected',{exact:true}).waitFor();await music.getByRole('checkbox',{name:/Same music · c/}).uncheck();await music.getByText('1 music clip selected',{exact:true}).waitFor();
 await dialog.getByRole('checkbox',{name:'Normalize music from source audio levels'}).uncheck();const baseline=await get();await dialog.getByRole('button',{name:'Build review plan'}).click();await dialog.getByRole('button',{name:'Apply reviewed mix'}).or(dialog.getByRole('alert')).first().waitFor();if(await dialog.getByRole('alert').count())throw Error(await dialog.getByRole('alert').innerText());assert.deepEqual(await get(),baseline);
 const voiceList=dialog.getByLabel('Voice clips used for ducking');assert.deepEqual(await voiceList.locator('li').allTextContents(),['Voice clip · a · 1.00–3.00 s','Second voice · d · 4.00–5.00 s']);
 const musicList=dialog.getByLabel('Music clips in mix plan');assert.deepEqual(await musicList.locator('li').allTextContents(),['Same music · b · 0.00–6.00 s']);await dialog.getByRole('button',{name:'Change settings'}).click();await music.getByRole('checkbox',{name:/Same music · c/}).check();await dialog.getByRole('button',{name:'Build review plan'}).click();await dialog.getByRole('button',{name:'Apply reviewed mix'}).waitFor();assert.deepEqual(await musicList.locator('li').allTextContents(),['Same music · b · 0.00–6.00 s','Same music · c · 0.00–6.00 s']);assert.deepEqual(await get(),baseline);
 await dialog.getByRole('button',{name:'Apply reviewed mix'}).click();await dialog.waitFor({state:'detached'});
 const poll=async predicate=>{for(let n=0;n<100;n++){const x=await get();if(predicate(x))return x;await new Promise(r=>setTimeout(r,100));}throw Error('mix project state timeout');};const item=(p,id)=>p.tracks.flatMap(t=>t.items).find(i=>i.item_id===id);
 const applied=await poll(p=>item(p,'music1').keyframes.length>0);assert.deepEqual(item(applied,'music2').keyframes.map(k=>k.offset.value),[0,23,30,150,173]);assert.equal(item(applied,'voice').keyframes.length,0);assert.equal(item(applied,'voice2').keyframes.length,0);assert.deepEqual(item(applied,'music1').keyframes.map(k=>k.offset.value),[0,23,30,150,173]);
 await page.getByRole('button',{name:/Undo/}).first().click();const undone=await poll(p=>item(p,'music1').keyframes.length===0);assert.deepEqual(undone.tracks,baseline.tracks);
 await page.getByRole('button',{name:/Redo/}).first().click();const redone=await poll(p=>item(p,'music1').keyframes.length>0);assert.deepEqual(redone.tracks,applied.tracks);
 report.steps.push('Four tracks: duplicate music names distinguished by track; subset and both reviewed, ducking offsets and Apply/Undo/Redo without playback');report.status='passed';console.log('PASS: audio mix counts and history');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-mix-count.json',JSON.stringify(report,null,2));await browser.close();}
