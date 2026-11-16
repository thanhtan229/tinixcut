import { chromium } from 'playwright';
import { writeFileSync, readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const path=process.env.TINIXCUT_CDP_PROJECT;
const report={startedAt:new Date().toISOString(),scope:'Qualification WebView branch replacement on saved/unsaved projects; not installed recovery UI',cases:[]};
const browser=await chromium.connectOverCDP('http://127.0.0.1:9444');
try{
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const get=()=>invoke('project_get_current');const wait=async predicate=>{for(let n=0;n<100;n++){const p=await get();if(predicate(p))return p;await new Promise(r=>setTimeout(r,100));}throw new Error('History/recovery state timeout');};
 report.health=await invoke('app_health_check');assert.equal(report.health.status,'healthy');
 for(const saved of [false,true]){
  await invoke('project_new',{name:`Branch ${saved?'saved':'unsaved'}`});
  const file=path+`.${saved?'saved':'unsaved'}.tcproj`;if(saved)await invoke('project_save',{path:file});await page.reload();
  const later=page.getByRole('button',{name:'Later',exact:true});if(await later.count())await later.click();
  const revisions=[];
  await page.getByRole('button',{name:/\+ Track/}).click();let p=await wait(p=>p.tracks.length===1);revisions.push(p.revision);
  await page.getByRole('button',{name:/Undo/}).first().click();p=await wait(p=>p.tracks.length===0);revisions.push(p.revision);
  await page.getByRole('button',{name:/Redo/}).first().click();p=await wait(p=>p.tracks.length===1);revisions.push(p.revision);
  await page.getByRole('button',{name:/Undo/}).first().click();p=await wait(p=>p.tracks.length===0);revisions.push(p.revision);
  await page.getByRole('button',{name:/\+ Track/}).click();p=await wait(p=>p.tracks.length===1);revisions.push(p.revision);
  const branch=p;assert.equal(await invoke('project_redo'),null);assert.deepEqual((await get()).tracks,branch.tracks);
  assert.ok(revisions.every((v,i)=>i===0||v>revisions[i-1]));
  if(saved){
   await invoke('project_save_recovery_snapshot',{path:file,expectedRevision:p.revision});
   const recovery=JSON.parse(readFileSync(file.replace(/\.tcproj$/,'.tcproj.recovery'),'utf8'));assert.deepEqual(recovery.tracks,p.tracks);
  }else{
   const receipt=await invoke('project_save_unsaved_recovery',{expectedProjectId:p.project_id,expectedRevision:p.revision});report.unsavedRecovery=receipt;
  }
  await invoke('project_save',{path:file});const reopened=await invoke('project_open',{path:file});assert.deepEqual(reopened.tracks,branch.tracks);
  report.cases.push({saved,revisions,redoCleared:true,tracks:reopened.tracks.length});
 }
 report.status='passed';console.log('PASS: qualification saved/unsaved history branch, recovery snapshot and reopen');
}catch(error){report.status='failed';report.error=String(error);throw error;}finally{report.finishedAt=new Date().toISOString();writeFileSync('artifacts/background/cdp-history-branch.json',JSON.stringify(report,null,2));await browser.close();}
