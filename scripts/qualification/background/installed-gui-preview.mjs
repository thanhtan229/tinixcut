import { chromium } from 'playwright';
import { readFileSync,writeFileSync,mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const candidate=JSON.parse(readFileSync(process.env.TINIXCUT_CANDIDATE_REPORT??'artifacts/candidates/latest-candidate.json','utf8'));
const binary=join(candidate.installation.directory,'tinixcut.exe');
const sha=file=>createHash('sha256').update(readFileSync(file)).digest('hex');
assert.equal(sha(binary),candidate.expectedInstalledBinary.sha256);
const directory=join(candidate.directory,'gui-preview',new Date().toISOString().replaceAll(':','-'));mkdirSync(directory,{recursive:true});
const report={startedAt:new Date().toISOString(),directory,binarySha256:sha(binary),scope:'Exact installed release with process-local CDP and mute-audio; isolated desktop, asset-free text fixture; no native input, video decode, native save/open or picker qualification',frames:[],consoleErrors:[],pageErrors:[]};
const ffmpeg=join(candidate.installation.directory,'resources/bin/ffmpeg.exe');
const run=(...args)=>execFileSync(ffmpeg,args,{windowsHide:true,timeout:45000,maxBuffer:32*1024*1024});
const projectPath=join(directory,'preview.tcproj'),output=join(directory,'export.mp4');
report.fixture='Asset-free text frame markers committed through installed Command Service; video decode excluded';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9455',{timeout:5000});
let page;
try{
 page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));assert.ok(page,'Installed page missing');
 page.on('console',m=>{if(m.type()==='error')report.consoleErrors.push(m.text());});page.on('pageerror',e=>report.pageErrors.push(String(e)));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 report.windowTitle=await invoke('plugin:window|title',{label:'main'});assert.equal(report.windowTitle,candidate.productName);
 report.health=await invoke('app_health_check');assert.equal(report.health.status,'healthy');
 await page.getByText('Core Healthy',{exact:false}).waitFor({timeout:15000});report.reactHealth=true;
 await page.screenshot({path:join(directory,'health.png')});
 await invoke('project_new',{name:'Silent installed GUI preview'});
 const p=await invoke('project_get_current');const time=value=>({value,rate:{numerator:30,denominator:1}});
 const operations=[{type:'TrackCreate',payload:{track_id:'markers',name:'Silent frame markers',kind:'Video'}}];
 for(const [id,start,duration,text] of [['a',0,30,'FRAME 0 — INSTALLED'],['b',30,45,'FRAME 30 — PREVIEW'],['c',75,225,'FRAME 75 — EXPORT']]){
  operations.push({type:'ClipCreate',payload:{track_id:'markers',item_id:id,name:text,kind:'Text',timeline_start:time(start),duration:time(duration)}},{type:'ClipTextSet',payload:{item_id:id,text}},{type:'ClipTextStyleSet',payload:{item_id:id,style:{font_family:'Inter',font_size:130,font_weight:700,font_color:'#FFFFFF',background_color:'#124875',background_opacity:1,background_corner_radius:0,padding_x:40,padding_y:20,alignment:'Center',line_height:1,anchor:'Center'}}});
 }
 operations.push({type:'ProjectMarkerSet',payload:{marker_id:'sample-marker-1',label:'Opening',position:time(30)}},{type:'ProjectMarkerSet',payload:{marker_id:'sample-marker-2',label:'Ending',position:time(90)}});
 await invoke('project_execute_command',{envelope:{schema_version:p.schema_version,project_id:p.project_id,base_revision:p.revision,transaction_id:'installed-preview',operations:operations.map((operation,i)=>({operation_id:`preview_${i}`,operation}))}});
 // This is an exported test snapshot of native Command Service state. There is
 // no host-side reducer and this does not claim native save/open qualification.
 const snapshot=await invoke('project_get_current');writeFileSync(projectPath,JSON.stringify(snapshot,null,2));await page.reload();
 const later=page.getByRole('button',{name:'Later',exact:true});
 try{await later.waitFor({timeout:2000});}catch(e){if(e.name!=='TimeoutError')throw e;}
 if(await later.isVisible())await later.click();
 const markerProject=await invoke('project_get_current');
 await page.getByLabel('Go to frame',{exact:true}).fill('0');await page.getByRole('button',{name:'Go',exact:true}).click();
 await page.getByRole('button',{name:'Marker ▶',exact:true}).click();await page.waitForFunction(()=>document.querySelector('#preview-frame-seek')?.value==='30');
 await page.locator('[data-timeline-clip]').first().focus();await page.keyboard.press('Alt+.');await page.waitForFunction(()=>document.querySelector('#preview-frame-seek')?.value==='90');
 await page.keyboard.press('Alt+,');await page.waitForFunction(()=>document.querySelector('#preview-frame-seek')?.value==='30');
 await page.getByLabel('Go to frame',{exact:true}).focus();await page.keyboard.press('Alt+.');assert.equal(await page.getByLabel('Go to frame',{exact:true}).inputValue(),'30');await page.getByLabel('Go to frame',{exact:true}).blur();
 assert.deepEqual(await invoke('project_get_current'),markerProject);report.markerWorkflow='Two persisted project markers; button, focused-clip shortcuts and input guard; no project mutation';
 execFileSync(binary,['render',projectPath,output],{windowsHide:true,timeout:60000,maxBuffer:1024*1024});
 run('-v','error','-i',output,'-f','null','-');report.exportSha256=sha(output);
 const exact=page.getByRole('button',{name:'Exact frame',exact:true});
 if(await exact.getAttribute('aria-pressed')==='true')await exact.click();
 report.exactFrameScope='Text-only composition uses shared raster overlays; Exact frame background is unsupported when there are no non-text layers';
 for(const frame of [0,30,75]){
  await page.getByLabel('Go to frame',{exact:true}).fill(String(frame));await page.getByRole('button',{name:'Go',exact:true}).click();
  const viewport=page.locator('.canvas-viewport');
  const expectedText={0:'FRAME 0 — INSTALLED',30:'FRAME 30 — PREVIEW',75:'FRAME 75 — EXPORT'}[frame];
  await viewport.locator(`img[alt="${expectedText}"]`).waitFor({timeout:20000});
  await page.waitForFunction(frame=>document.querySelector('#preview-frame-seek')?.value===String(frame),frame);
  await page.waitForTimeout(1000);
  const screenshot=join(directory,`preview-${frame}.png`),reference=join(directory,`export-${frame}.png`);
  await viewport.screenshot({path:screenshot});
  run('-v','error','-i',output,'-vf',`select=eq(n\\,${frame})`,'-frames:v','1',reference);
  // Exclude viewport border and diagnostic HUD, retaining the entire marker.
  const rgb=file=>run('-v','error','-i',file,'-vf','scale=320:180,crop=288:126:16:27','-f','rawvideo','-pix_fmt','rgb24','-');
  const a=rgb(screenshot),b=rgb(reference);assert.equal(a.length,b.length);
  let sum=0,visible=0;for(let i=0;i<a.length;i++){sum+=Math.abs(a[i]-b[i]);if(a[i]>30)visible++;}
  const entry={frame,expectedText,screenshotExportMae:sum/a.length,visibleFraction:visible/a.length,screenshotSha256:sha(screenshot)};report.frames.push(entry);
  assert.ok(entry.visibleFraction>0.02,'A blank screenshot must fail');assert.ok(entry.screenshotExportMae<3,JSON.stringify(entry));
 }
 assert.equal(new Set(report.frames.map(f=>f.screenshotSha256)).size,3,'Frames must change');
 const navigationBefore=await invoke('project_get_current');
 await page.getByRole('button',{name:'Fit all',exact:true}).click();
 assert.equal(await page.getByRole('button',{name:'Fit all',exact:true}).getAttribute('aria-pressed'),'true');
 const clips=page.locator('[data-timeline-clip]');assert.ok(await clips.count()>=3);
 await clips.first().click();
 const fitSelection=page.getByRole('button',{name:'Fit selection',exact:true});assert.equal(await fitSelection.isEnabled(),true);await fitSelection.click();assert.equal(await fitSelection.getAttribute('aria-pressed'),'true');
 const slider=page.getByLabel('Timeline zoom');const oldZoom=await slider.inputValue();await slider.press('ArrowLeft');assert.notEqual(await slider.inputValue(),oldZoom);
 await page.getByLabel('Go to frame',{exact:true}).fill('150');await page.getByRole('button',{name:'Go',exact:true}).click();
 await page.getByRole('button',{name:'Fit all',exact:true}).click();
 const navMetrics=async()=>page.evaluate(()=>{const v=document.querySelector('.timeline-scroll-viewport');const z=Number(document.querySelector('[aria-label="Timeline zoom"]').value);return {zoom:Math.exp(z),scroll:v.scrollLeft,ticks:[...document.querySelectorAll('.timeline-ruler svg g text')].map(e=>e.textContent)};});
 const navBefore=await navMetrics();await slider.press('ArrowRight');await page.waitForTimeout(150);const navAfter=await navMetrics();
 assert.ok(Math.abs((150*navBefore.zoom-navBefore.scroll)-(150*navAfter.zoom-navAfter.scroll))<2,JSON.stringify({navBefore,navAfter}));
 assert.ok(navBefore.ticks.length>0&&navAfter.ticks.length>0);report.timelineGeometry={before:navBefore,after:navAfter,anchorFrame:150};
 assert.deepEqual(await invoke('project_get_current'),navigationBefore);report.timeline={fitAll:true,fitSelection:true,sliderZoom:true,projectUnchanged:true};
 await page.getByLabel('Go to frame',{exact:true}).fill('0');await page.getByRole('button',{name:'Go',exact:true}).click();
 await page.getByRole('button',{name:'▶ Play',exact:true}).click();
 await page.waitForTimeout(900);
 report.playback={frameAfter:await page.getByLabel('Go to frame',{exact:true}).inputValue()};
 await page.getByRole('button',{name:'⏸ Pause',exact:true}).click();
 assert.ok(Number(report.playback.frameAfter)>5);
 assert.equal(await page.locator('video,audio').count(),0,'Asset-free fixture must not create media elements');
 report.playback.immediateAfterPause=await page.getByLabel('Go to frame',{exact:true}).inputValue();
 await page.getByRole('button',{name:'▶ Play',exact:true}).waitFor({timeout:5000});
 await page.waitForTimeout(250);
 const before=await page.getByLabel('Go to frame',{exact:true}).inputValue();await page.waitForTimeout(700);const after=await page.getByLabel('Go to frame',{exact:true}).inputValue();report.playback.pausedSamples=[before,after];assert.equal(after,before);assert.ok(Number(after)>=Number(report.playback.frameAfter),'Pause must retain the played position');report.playback.pausedFrame=after;
 await page.screenshot({path:join(directory,'playback-paused.png')});
 assert.equal(report.pageErrors.length,0);assert.equal(report.consoleErrors.length,0,JSON.stringify(report.consoleErrors));
 const assetState=await invoke('project_get_current');
 await invoke('project_execute_command',{envelope:{schema_version:assetState.schema_version,project_id:assetState.project_id,base_revision:assetState.revision,transaction_id:'installed-media-usage',operations:[{operation_id:'fixture-asset',operation:{type:'AssetImport',payload:{asset_id:'fixture-source',name:'Usage fixture.wav',path:join(directory,'missing-fixture.wav'),media_kind:'Audio',duration:time(300),file_size:0,dimensions:null}}},{operation_id:'fixture-track',operation:{type:'TrackCreate',payload:{track_id:'fixture-audio',name:'Fixture audio',kind:'Audio'}}},...[0,90].map((start,i)=>({operation_id:`fixture-clip-${i}`,operation:{type:'ClipCreate',payload:{track_id:'fixture-audio',item_id:`fixture-${i}`,name:`Fixture ${i}`,kind:'Audio',asset_id:'fixture-source',timeline_start:time(start),duration:time(30),source_start:time(0)}}}))]}});
 await page.reload();
 try{await later.waitFor({timeout:2000});}catch(e){if(e.name!=='TimeoutError')throw e;} if(await later.isVisible())await later.click();
 await page.getByText('Used in 2 timeline clips',{exact:true}).waitFor();
 await page.getByLabel('Filter media usage').selectOption('unused');await page.getByText('No media matches these filters.',{exact:true}).waitFor();
 await page.getByLabel('Filter media usage').selectOption('used');await page.getByText('Used in 2 timeline clips',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Locate on timeline',exact:true}).click();await page.getByText('Clip 1/2 · Fixture audio',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Locate on timeline',exact:true}).click();await page.getByText('Clip 2/2 · Fixture audio',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Previous use',exact:true}).click();await page.getByText('Clip 1/2 · Fixture audio',{exact:true}).waitFor();report.mediaUsage='Two occurrences; used/unused filters and bidirectional navigation';
 const draftState=await invoke('project_get_current');
 await invoke('project_execute_command',{envelope:{schema_version:draftState.schema_version,project_id:draftState.project_id,base_revision:draftState.revision,transaction_id:'installed-draft-fixture',operations:[{operation_id:'v-asset',operation:{type:'AssetImport',payload:{asset_id:'fixture-video',name:'Offline sample.mp4',path:join(directory,'missing-video.mp4'),media_kind:'Video',duration:time(300),file_size:0,dimensions:null}}},{operation_id:'v-track',operation:{type:'TrackCreate',payload:{track_id:'fixture-video-track',name:'Voice',kind:'Video'}}},{operation_id:'v-clip',operation:{type:'ClipCreate',payload:{track_id:'fixture-video-track',item_id:'fixture-voice',name:'Voice sample',kind:'Video',asset_id:'fixture-video',timeline_start:time(0),duration:time(30),source_start:time(0)}}}]}});
 await page.reload();try{await later.waitFor({timeout:2000});}catch(e){if(e.name!=='TimeoutError')throw e;}if(await later.isVisible())await later.click();
 await page.getByRole('button',{name:'Audio mix',exact:true}).click();const mix=page.getByRole('dialog',{name:'Review audio mix plan'});await mix.getByRole('checkbox',{name:'Normalize music from source audio levels'}).uncheck();await mix.getByRole('button',{name:'Build review plan'}).click();await mix.getByRole('button',{name:'Change settings'}).waitFor();await mix.getByRole('button',{name:'Change settings'}).click();assert.equal(await mix.getByRole('button',{name:'Apply reviewed mix'}).count(),0);await mix.getByRole('button',{name:'Cancel',exact:true}).click();report.installedMix='Offline fixture timing; real plan dry-run, revise; no level analysis or Apply';
 await page.getByRole('button',{name:'✨ Draft',exact:true}).click();const draft=page.getByRole('dialog',{name:'Create video draft'});await draft.getByText(/Offline sample.mp4.*offline/i).waitFor();assert.equal(await draft.getByRole('button',{name:'Generate preview'}).isDisabled(),true);await draft.getByRole('button',{name:'Cancel',exact:true}).click();report.installedDraft='Offline source explicitly disabled; no planner preview';
 assert.equal(sha(binary),report.binarySha256);report.status='passed';console.log('PASS: exact installed GUI health, paused text frame/export pixels and silent transport');
}catch(error){report.status='failed';report.error=String(error);console.error(error);process.exitCode=1;
 try{if(page){report.failureDom=await page.locator('body').innerText();report.exactFrameNotes=await page.locator('[title]').evaluateAll(es=>es.filter(e=>e.textContent==='Exact frame: live preview').map(e=>e.title));await page.screenshot({path:join(directory,'failure.png')});}}catch(captureError){report.captureError=String(captureError);}
}
finally{report.finishedAt=new Date().toISOString();writeFileSync(join(directory,'report.json'),JSON.stringify(report,null,2));writeFileSync(join(candidate.directory,'gui-preview-latest.json'),JSON.stringify(report,null,2));await browser.close();}
