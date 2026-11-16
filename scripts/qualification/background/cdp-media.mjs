import { chromium } from 'playwright';
import { execFileSync } from 'node:child_process';
import { writeFileSync, readFileSync, renameSync, mkdirSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, dirname, join } from 'node:path';
import assert from 'node:assert/strict';
const path=process.env.TINIXCUT_CDP_PROJECT;
const ffmpeg=resolve('src-tauri/resources/bin/ffmpeg.exe');
const original=path+'.original.wav',replacement=path+'.replacement.wav',short=path+'.short.wav',wrong=path+'.wrong.png';
execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','sine=frequency=440:duration=4',original],{windowsHide:true,timeout:10000});
execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','sine=frequency=880:duration=0.1',short],{windowsHide:true,timeout:10000});
execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','color=red:size=16x16','-frames:v','1',wrong],{windowsHide:true,timeout:10000});
const hash=file=>createHash('sha256').update(readFileSync(file)).digest('hex');const sourceHash=hash(original);
const report={startedAt:new Date().toISOString(),scope:'Real WebView missing/relinked/collected readback and Undo/Redo; relink/collect via scoped native commands; native pickers excluded',steps:[]};
const browser=await chromium.connectOverCDP('http://127.0.0.1:9444');const timer=setTimeout(()=>process.exit(124),90000);
try{
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const reload=async()=>{await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});
  try{await later.waitFor({timeout:2500});}catch(error){if(error.name!=='TimeoutError')throw error;}
  if(await later.isVisible())await later.click();};
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const get=()=>invoke('project_get_current');const poll=async predicate=>{for(let n=0;n<80;n++){const p=await get();if(predicate(p))return p;await new Promise(r=>setTimeout(r,100));}throw new Error('Media state timeout');};
 report.health=await invoke('app_health_check');assert.equal(report.health.status,'healthy');
 const asset=await invoke('media_import',{filePath:original});
 const execute=async operations=>{const p=await get();return invoke('project_execute_command',{envelope:{schema_version:p.schema_version,project_id:p.project_id,base_revision:p.revision,transaction_id:`media_${p.revision}`,operations:operations.map((operation,i)=>({operation_id:`media_${p.revision}_${i}`,operation}))}});};
 const time=value=>({value,rate:{numerator:30,denominator:1}});
 await execute([{type:'TrackCreate',payload:{track_id:'audio',name:'Audio',kind:'Audio'}},{type:'ClipCreate',payload:{track_id:'audio',item_id:'audio',name:'Source range',kind:'Audio',asset_id:asset.asset_id,timeline_start:time(0),duration:time(60),source_start:time(30)}}]);
 await invoke('project_save',{path});const originalProjectHash=hash(path);
 // Move only the owned temporary fixture. The project deliberately retains the old path.
 renameSync(original,replacement);assert.equal(hash(replacement),sourceHash);await invoke('project_open',{path});await reload();
 await page.getByText(/Media offline · Last path:/).waitFor();assert.equal((await invoke('media_list_offline_assets')).length,1);
 report.steps.push('Missing temporary source is explicit in real WebView and native offline list');
 for(const badPath of [wrong,short,join(dirname(path),'missing.wav')]){
  const before=await get();let rejection;
  try{await execute([{type:'AssetRelink',payload:{asset_id:asset.asset_id,path:badPath}}]);}catch(error){rejection=String(error);}
  assert.ok(rejection,'Incompatible/short/missing relink must reject');assert.deepEqual(await get(),before);report.rejections??=[];report.rejections.push({file:badPath.split(/[\\/]/).at(-1),error:rejection});
 }
 report.steps.push('Wrong-kind, too-short and nonexistent replacements reject atomically');
 await execute([{type:'AssetRelink',payload:{asset_id:asset.asset_id,path:replacement}}]);await invoke('project_save',{path});await reload();
 await page.getByText(/Media offline · Last path:/).waitFor({state:'detached'});assert.equal((await invoke('media_list_offline_assets')).length,0);
 await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>p.assets[0].path===original);await page.getByText(/Media offline · Last path:/).waitFor();
 await page.getByRole('button',{name:/Redo/}).first().click();await poll(p=>p.assets[0].path===replacement);
 await page.getByText(/Media offline · Last path:/).waitFor({state:'detached'});
 await invoke('project_save',{path});const linked=await get();assert.deepEqual((await invoke('project_open',{path})).tracks,linked.tracks);const linkedHash=hash(path);
 await reload();await page.getByRole('button',{name:'Preview',exact:true}).click();await page.getByLabel('Source In seconds').waitFor();
 report.steps.push('Scoped relink -> UI online/source preview -> UI Undo offline/Redo online -> native save/reopen');
 const destination=join(dirname(path),'collected');mkdirSync(destination);
 const current=await get();const receipt=await invoke('project_collect_media',{destinationDirectory:destination,expectedProjectId:current.project_id,expectedRevision:current.revision});report.collection=receipt;
 assert.equal(hash(path),linkedHash);assert.equal(hash(replacement),sourceHash);assert.ok(!existsSync(original));
 const collected=await invoke('project_open',{path:receipt.projectPath});assert.equal(collected.assets.length,1);assert.equal(hash(collected.assets[0].path),sourceHash);
 const manifestPath=receipt.manifestPath ?? join(receipt.mediaDirectory,'manifest.json');
 report.manifest=JSON.parse(readFileSync(manifestPath,'utf8'));assert.ok(JSON.stringify(report.manifest).toLowerCase().includes(sourceHash));
 assert.deepEqual(collected.tracks,current.tracks);await reload();assert.equal(await page.getByText(/Media offline · Last path:/).count(),0);
 await page.getByRole('button',{name:'Preview',exact:true}).click();await page.getByLabel('Source In seconds').waitFor();
 const inaccessible=replacement+'.unavailable';renameSync(replacement,inaccessible);
 try{
  assert.equal(existsSync(replacement),false);
  const independent=await invoke('project_open',{path:receipt.projectPath});assert.equal((await invoke('media_list_offline_assets')).length,0);
  assert.equal(hash(independent.assets[0].path),sourceHash);await reload();
  await page.getByRole('button',{name:'Preview',exact:true}).click();await page.getByLabel('Source In seconds').waitFor();
  const installedBinary=process.env.TINIXCUT_TEST_BINARY;
  if(installedBinary){
   const candidate=JSON.parse(readFileSync('artifacts/candidates/latest-candidate.json','utf8'));
   assert.equal(hash(installedBinary),candidate.expectedInstalledBinary.sha256);
   execFileSync(installedBinary,['inspect',receipt.projectPath],{windowsHide:true,timeout:15000});
   const output=join(destination,'installed-collected.mp4');
   execFileSync(installedBinary,['render',receipt.projectPath,output],{windowsHide:true,timeout:60000});
   execFileSync(join(dirname(installedBinary),'resources/bin/ffmpeg.exe'),['-v','error','-i',output,'-f','null','-'],{windowsHide:true,timeout:20000});
   report.installedCollected={binarySha256:hash(installedBinary),outputSha256:hash(output),formerSourceAbsent:true,fullDecode:true};
  }
  report.steps.push('Former linked source made inaccessible after collect; collected WebView reopen/source preview independent of original');
 }finally{renameSync(inaccessible,replacement);}
 report.hashes={source:sourceHash,linkedProject:linkedHash,originalProjectBeforeRelink:originalProjectHash,collectedSource:hash(collected.assets[0].path)};
 report.steps.push('Collected copy manifest/hash/native reopen/WebView online preview pass; linked source project and media bytes preserved');
 report.status='passed';console.log('PASS: real WebView offline/relink/history and scoped collect/hash/reopen workflow');
}catch(error){report.status='failed';report.error=String(error);throw error;}finally{clearTimeout(timer);report.finishedAt=new Date().toISOString();writeFileSync('artifacts/background/cdp-media.json',JSON.stringify(report,null,2));await browser.close();}
