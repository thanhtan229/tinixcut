import { chromium } from 'playwright';
import { writeFileSync, readFileSync, copyFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const path = process.env.TINIXCUT_CDP_PROJECT;
const report = { startedAt: new Date().toISOString(), scope: 'Real isolated WebView caption edits/transcript; scoped native subtitle file commands, native dialogs excluded', steps: [] };
const browser = await chromium.connectOverCDP('http://127.0.0.1:9444');
const timer = setTimeout(() => process.exit(124), 180000);
try {
 const page = browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const originalSettings=await invoke('app_settings_get');report.savedSettings=originalSettings;
 const get=()=>invoke('project_get_current'); const delay=ms=>new Promise(r=>setTimeout(r,ms));
 const poll=async predicate=>{for(let n=0;n<100;n++){const p=await get();if(predicate(p))return p;await delay(100);}throw new Error('Caption native state timeout');};
 report.health=await invoke('app_health_check'); assert.equal(report.health.status,'healthy');
 let p=await get();const time=value=>({value,rate:p.settings.frame_rate});
 const operations=[{type:'TrackCreate',payload:{track_id:'captions',name:'Captions',kind:'Video'}}];
 for(const [id,start,text] of [['a',0,'Xin chào'],['b',30,'Việt Nam'],['c',60,'Dòng ba']])operations.push({type:'ClipCreate',payload:{track_id:'captions',item_id:id,name:id,kind:'Text',timeline_start:time(start),duration:time(30)}},{type:'ClipTextSet',payload:{item_id:id,text}});
 const execute=async ops=>{const p=await get();return invoke('project_execute_command',{envelope:{schema_version:p.schema_version,project_id:p.project_id,base_revision:p.revision,transaction_id:`caption_${p.revision}`,operations:ops.map((operation,i)=>({operation_id:`op_${p.revision}_${i}`,operation}))}});};
 await execute(operations);await invoke('project_save',{path});await page.reload();
 const deferRecovery=async()=>{const later=page.getByRole('button',{name:'Later',exact:true});
  try{await later.waitFor({timeout:2500});}catch(error){if(error.name!=='TimeoutError')throw error;}
  if(await later.isVisible())await later.click();};
 const open=async()=>{await deferRecovery();await page.getByRole('button',{name:/Caption editor/}).click();};
 const close=()=>page.getByRole('button',{name:'Close caption editor'}).click();
 await open();
 await page.getByRole('textbox',{name:'Caption text 0.00–1.00s'}).fill('Xin chào\nViệt Nam — thử nghiệm');
 await page.getByRole('button',{name:'Apply edits',exact:true}).click();await poll(p=>p.tracks[0].items.find(i=>i.item_id==='a').text_content.includes('thử nghiệm'));
 // Reorder to an empty range, preserving item identity and avoiding an implicit swap.
 await page.getByRole('spinbutton',{name:'Start frame a',exact:true}).fill('90');
 await page.getByRole('button',{name:'Apply edits',exact:true}).click();await poll(p=>p.tracks[0].items.find(i=>i.item_id==='a').timeline_range.start.value===90);
 assert.deepEqual(await page.locator('textarea[aria-label^="Caption text"]').allTextContents(),['Việt Nam','Dòng ba','Xin chào\nViệt Nam — thử nghiệm']);
 await close();await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>p.tracks[0].items.find(i=>i.item_id==='a').timeline_range.start.value===0);
 await page.getByRole('button',{name:/Redo/}).first().click();await poll(p=>p.tracks[0].items.find(i=>i.item_id==='a').timeline_range.start.value===90);
 await page.getByLabel('Go to frame',{exact:true}).fill('105');await page.getByRole('button',{name:'Go',exact:true}).click();await open();
 await page.getByRole('button',{name:'Split here',exact:true}).filter({visible:true}).last().click();
 p=await poll(p=>p.tracks[0].items.length===4);
 const tail=p.tracks[0].items.find(i=>i.item_id!=='a'&&i.timeline_range.start.value===105);
 await page.getByRole('checkbox',{name:'Select caption a to merge'}).check();await page.getByRole('checkbox',{name:`Select caption ${tail.item_id} to merge`}).check();
 await page.getByRole('button',{name:'Merge adjacent',exact:true}).click();p=await poll(p=>p.tracks[0].items.length===3);
 assert.equal(p.tracks[0].items.find(i=>i.item_id==='a').timeline_range.duration.value,30);
 await close();await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>p.tracks[0].items.length===4);
 await page.getByRole('button',{name:/Redo/}).first().click();p=await poll(p=>p.tracks[0].items.length===3);
 await invoke('project_save',{path});assert.deepEqual((await invoke('project_open',{path})).tracks,p.tracks);await page.reload();await open();
 assert.equal(await page.locator('textarea[aria-label^="Caption text"]').count(),3);await close();
 report.steps.push('UI multiline edit, reorder into empty range, split/merge and Undo/Redo; native save/reopen/UI readback');
 const normalize=items=>items.map(i=>({text:i.text_content,start:i.timeline_range.start.value*i.timeline_range.start.rate.denominator/i.timeline_range.start.rate.numerator,duration:i.timeline_range.duration.value*i.timeline_range.duration.rate.denominator/i.timeline_range.duration.rate.numerator})).sort((a,b)=>a.start-b.start);
 const expected=normalize(p.tracks[0].items);
 for(const extension of ['srt','vtt']){
  const output=path+'.'+extension;await invoke('captions_export',{outputPath:output});assert.ok(readFileSync(output,'utf8').includes('Việt Nam'));
  await invoke('captions_import',{filePath:output});let imported=await get();assert.deepEqual(normalize(imported.tracks.at(-1).items),expected);
  await page.reload();await open();assert.equal(await page.locator('textarea[aria-label^="Caption text"]').count(),6);await close();
  await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>p.tracks.length===1);
 }
 report.steps.push('Scoped native SRT/WebVTT export/import preserves Vietnamese multiline timing/text; real UI shows imported rows and Undo removes them');
 const wav=path+'.speech.wav';copyFileSync(resolve('artifacts/phase26-whisper/test-input-jfk.wav'),wav);
 const sha=file=>createHash('sha256').update(readFileSync(file)).digest('hex');
 report.transcriptionInputs={sourceSha256:sha(wav),cliSha256:sha(resolve('artifacts/phase26-whisper/whisper.cpp-v1.9.2-x64/Release/whisper-cli.exe')),modelSha256:sha(resolve('artifacts/phase26-whisper/models/ggml-base.bin'))};
 const asset=await invoke('media_import',{filePath:wav});
 await execute([{type:'TrackCreate',payload:{track_id:'speech',name:'Speech',kind:'Audio'}},{type:'ClipCreate',payload:{track_id:'speech',item_id:'speech',name:'Speech',kind:'Audio',asset_id:asset.asset_id,timeline_start:time(0),duration:time(300),source_start:time(0)}}]);
 await invoke('project_save',{path});await page.reload();
 await invoke('app_settings_save',{settings:{...originalSettings,whisperCliPath:resolve('artifacts/phase26-whisper/whisper.cpp-v1.9.2-x64/Release/whisper-cli.exe'),whisperModelPath:resolve('artifacts/phase26-whisper/models/ggml-base.bin'),legacyBrowserSettingsMigrated:true,transcriptionLanguage:'en'}});
  await deferRecovery();await page.getByRole('group',{name:/^Speech, Audio clip/}).press('Enter');await page.getByRole('button',{name:/Auto captions/}).click();
 const dialog=page.getByRole('dialog',{name:'Auto captions',exact:true});
 for(let n=0;n<80;n++){if((await dialog.getByPlaceholder('Path to whisper-cli.exe').inputValue()) && (await dialog.getByPlaceholder('Path to ggml model .bin').inputValue()))break;await delay(100);}
 report.transcriptConfiguration={cli:await dialog.getByPlaceholder('Path to whisper-cli.exe').inputValue(),model:await dialog.getByPlaceholder('Path to ggml model .bin').inputValue(),dialog:await dialog.innerText()};
 const beforeAnalysis=await get();
 await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).click();
 await dialog.getByRole('button',{name:'Cancel job',exact:true}).waitFor();
 await dialog.getByRole('button',{name:'Cancel job',exact:true}).click();
 await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).waitFor();
 assert.deepEqual((await get()).tracks,beforeAnalysis.tracks);
 report.cancelText=await dialog.innerText();assert.match(report.cancelText,/cancel/i);
 report.steps.push('Real Whisper analysis start/cancel via UI preserves project tracks');
 await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).click();
 await dialog.getByRole('region',{name:'Source transcript',exact:true}).waitFor({timeout:45000});
 const segments=dialog.getByRole('checkbox',{name:/Select transcript segment/});await segments.first().waitFor();
 report.transcriptSegments=await segments.count();assert.ok(report.transcriptSegments>0);
 // Retry from the UI while the completed source transcript is cached. This
 // exercises both uncached and cached worker paths without a project mutation.
 await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).click();
 await dialog.getByRole('checkbox',{name:/Select transcript segment/}).first().waitFor({timeout:90000});
 await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).waitFor();
 for(let n=0;n<400 && await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).isDisabled();n++)await delay(100);
 assert.equal(await dialog.getByRole('button',{name:'Analyze transcript',exact:true}).isDisabled(),false);
 assert.match(await dialog.innerText(),/Analyzed .*source transcript segment/);
 assert.deepEqual((await get()).tracks,beforeAnalysis.tracks);
 report.steps.push('Repeated source-transcript analysis completes without changing project tracks (matching cache identity)');
 const transcriptUi=(await dialog.getByRole('region',{name:'Source transcript'}).innerText());
 // Compare the real UI transcript with the independently completed installed
 // native transcript; both use the same recorded source/model/CLI inputs.
 const candidate=JSON.parse(readFileSync('artifacts/candidates/latest-candidate.json','utf8'));
 const installed=JSON.parse(readFileSync(resolve(candidate.directory,'installed-acceptance-latest.json'),'utf8'));
 assert.equal(installed.status,'passed');assert.equal(installed.binarySha256,candidate.expectedInstalledBinary.sha256);
 assert.deepEqual(installed.transcriptionInputs,report.transcriptionInputs);
 const installedTranscript=installed.transcript;
 const segment=installedTranscript.result.segments[0];assert.ok(transcriptUi.includes(segment.text));
 const selected={sourceStart:segment.start,sourceDuration:segment.duration,text:segment.text};
 const previewBefore=await get();
 await dialog.getByRole('button',{name:'Close',exact:true}).first().click();await dialog.waitFor({state:'hidden'});
 await page.getByRole('button',{name:'Preview',exact:true}).click();
 await page.getByLabel('Source In seconds').waitFor();
 const sourceDialog=page.locator('.source-viewer-dialog');await sourceDialog.getByRole('button',{name:'Cancel',exact:true}).click();
 await sourceDialog.waitFor({state:'hidden'});assert.deepEqual((await get()).tracks,previewBefore.tracks);
 await page.getByRole('button',{name:/Auto captions/}).click();
 await segments.first().check();await dialog.getByRole('button',{name:'Create rough cut track',exact:true}).click();
 p=await poll(p=>p.tracks.length>beforeAnalysis.tracks.length);
 report.roughCut=p.tracks.at(-1).items.map(i=>({assetId:i.asset_id,source:i.source_range,timeline:i.timeline_range}));assert.ok(report.roughCut.every(i=>i.assetId===asset.asset_id));
 const seconds=t=>t.value*t.rate.denominator/t.rate.numerator;
 const actual=report.roughCut[0].source;const expectedStart=Math.round(seconds(selected.sourceStart)*30)/30;const expectedEnd=Math.round((seconds(selected.sourceStart)+seconds(selected.sourceDuration))*30)/30;
 assert.equal(seconds(actual.start),expectedStart);assert.equal(seconds(actual.duration),expectedEnd-expectedStart);
 report.selectedTranscript=selected;report.transcriptUi=transcriptUi;
 await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
 await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>p.tracks.length===beforeAnalysis.tracks.length);
 await page.getByRole('button',{name:/Redo/}).first().click();p=await poll(p=>p.tracks.length>beforeAnalysis.tracks.length);
 await invoke('project_save',{path});assert.deepEqual((await invoke('project_open',{path})).tracks,p.tracks);
 report.steps.push('Real Whisper source transcript -> UI selected-range rough cut -> Undo/Redo -> native save/reopen');
 await invoke('app_settings_save',{settings:originalSettings});
 report.status='passed';console.log('PASS: extended real WebView caption/subtitle/transcript workflow');
}catch(error){report.status='failed';report.error=String(error);throw error;}finally{
 const settings=report.savedSettings;delete report.savedSettings;report.finishedAt=new Date().toISOString();
 writeFileSync('artifacts/background/cdp-captions.json',JSON.stringify(report,null,2));
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 try{
  if(settings&&page){
   let restoreTimer;
   try{await Promise.race([page.evaluate(settings=>window.__TAURI_INTERNALS__.invoke('app_settings_save',{settings}),settings),new Promise((_,reject)=>{restoreTimer=setTimeout(()=>reject(new Error('Settings restore IPC timed out')),5000);})]);}
   finally{clearTimeout(restoreTimer);}
   report.settingsRestored=true;
  }
 }catch(error){report.status='failed';report.settingsRestoreError=String(error);throw error;}
 finally{writeFileSync('artifacts/background/cdp-captions.json',JSON.stringify(report,null,2));clearTimeout(timer);await browser.close();}
}
