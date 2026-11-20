import {chromium} from 'playwright';
import {execFileSync} from 'node:child_process';
import {writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
const base=process.env.TINIXCUT_CDP_PROJECT,missing=base+'.missing.wav',replacement=base+'.replacement.wav',ffmpeg=resolve('src-tauri/resources/bin/ffmpeg.exe');
execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','anullsrc=r=8000:cl=mono','-t','3','-c:a','pcm_s16le',replacement],{windowsHide:true,timeout:10000});
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});const report={scope:'Missing media duration filter across relink and history',steps:[]};const timer=setTimeout(()=>process.exit(124),60000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});const get=()=>invoke('project_get_current');const p=await get();
 await invoke('project_execute_command',{envelope:{schema_version:p.schema_version,project_id:p.project_id,base_revision:p.revision,transaction_id:'missing_duration_seed',operations:[{operation_id:'missing_duration_asset',operation:{type:'AssetImport',payload:{asset_id:'missing-duration',name:'Missing duration audio',path:missing,media_kind:'Audio',duration:null,file_size:1,dimensions:null}}}]}});
 await invoke('project_save',{path:base});await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 const filter=page.getByLabel('Missing audio/video duration');await filter.check();const results=page.getByLabel('Media results');await page.getByText(/Media offline/).waitFor();assert.equal(await results.textContent(),'1 of 1 sources shown');
 await page.evaluate(value=>{window.__TINIXCUT_QUALIFICATION_PICKER__=async()=>value;},replacement);await page.getByRole('button',{name:'Relink…',exact:true}).click();
 const poll=async pred=>{for(let i=0;i<100;i++){const p=await get();if(pred(p))return p;await new Promise(r=>setTimeout(r,100));}throw Error('metadata state timeout');};
 const relinked=await poll(p=>p.assets.find(a=>a.asset_id==='missing-duration')?.path===replacement);assert.ok(relinked.assets[0].duration?.value>0);await page.getByText('No media matches these filters.').waitFor();assert.equal(await results.textContent(),'0 of 1 sources shown');
 await page.getByRole('button',{name:/Undo/}).first().click();const undone=await poll(p=>p.assets[0].path===missing);assert.equal(undone.assets[0].duration,null);await page.getByText(/Media offline/).waitFor();assert.equal(await results.textContent(),'1 of 1 sources shown');
 await page.getByRole('button',{name:/Redo/}).first().click();await poll(p=>p.assets[0].path===replacement);await page.getByText('No media matches these filters.').waitFor();assert.equal(await results.textContent(),'0 of 1 sources shown');
 report.steps.push('Missing duration appears; UI relink probes duration; Undo restores missing metadata, Redo restores valid metadata and filter results');report.status='passed';console.log('PASS: missing-duration filter relink and history');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-missing-duration.json',JSON.stringify(report,null,2));await browser.close();}
