import {chromium} from 'playwright';
import {execFileSync} from 'node:child_process';
import {writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
const base=process.env.TINIXCUT_CDP_PROJECT, video=base+'.draft.mp4', audio=base+'.draft.wav',ffmpeg=resolve('src-tauri/resources/bin/ffmpeg.exe');
execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','color=c=black:s=64x64:r=30:d=5','-an','-c:v','libx264','-preset','ultrafast',video],{windowsHide:true,timeout:30000});
execFileSync(ffmpeg,['-v','error','-y','-f','lavfi','-i','anullsrc=r=8000:cl=mono','-t','5','-c:a','pcm_s16le',audio],{windowsHide:true,timeout:10000});
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});const report={scope:'Real planner draft audio indication',steps:[]};const timer=setTimeout(()=>process.exit(124),90000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});const get=()=>invoke('project_get_current');
 const v=await invoke('media_import',{filePath:video});const a=await invoke('media_import',{filePath:audio});await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 const poll=async pred=>{for(let i=0;i<100;i++){const p=await get();if(pred(p))return p;await new Promise(r=>setTimeout(r,100));}throw Error('draft project state timeout');};
 for(const withAudio of [false,true]){
  await page.getByRole('button',{name:'✨ Draft',exact:true}).click();const dialog=page.getByRole('dialog',{name:'Create video draft'});await dialog.waitFor();await dialog.getByLabel('Target duration in seconds').fill('3');if(withAudio)await dialog.getByLabel('Draft music bed').selectOption(a.asset_id);
  const before=await get();await dialog.getByRole('button',{name:'Generate preview'}).click();await dialog.getByRole('button',{name:'Apply draft'}).or(dialog.getByRole('alert')).first().waitFor({timeout:20000});if(await dialog.getByRole('alert').count())throw Error(await dialog.getByRole('alert').innerText());
  assert.equal(await dialog.getByText('Includes audio clips.',{exact:true}).count(),withAudio?1:0);assert.deepEqual(await get(),before);
  const audioRows=dialog.getByLabel('Planned audio clips');assert.equal(await audioRows.count(),withAudio?1:0);const reviewedAudio=withAudio?await audioRows.locator('li').allTextContents():[];if(withAudio)assert.equal(reviewedAudio.length,1);
  await dialog.getByRole('button',{name:'Apply draft'}).click();await dialog.waitFor({state:'detached'});const hasVideo=p=>p.tracks.flatMap(t=>t.items).some(i=>i.asset_id===v.asset_id);const hasAudio=p=>p.tracks.flatMap(t=>t.items).some(i=>i.asset_id===a.asset_id);
  const applied=await poll(hasVideo);assert.equal(hasAudio(applied),withAudio);
  if(withAudio){const clip=applied.tracks.flatMap(t=>t.items).find(i=>i.asset_id===a.asset_id);const format=time=>`~${(time.value*time.rate.denominator/time.rate.numerator).toFixed(3)} s (${time.value} frames at ${time.rate.numerator}/${time.rate.denominator} fps)`;assert.equal(reviewedAudio[0],`${clip.name} · timeline start ${format(clip.timeline_range.start)} · duration ${format(clip.timeline_range.duration)} · source start ${format(clip.source_range.start)}`);}
  await page.getByRole('button',{name:/Undo/}).first().click();const undone=await poll(p=>!hasVideo(p));assert.deepEqual(undone.tracks,before.tracks);await page.getByRole('button',{name:/Redo/}).first().click();const redone=await poll(hasVideo);assert.deepEqual(redone.tracks,applied.tracks);await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>!hasVideo(p));
 }
 report.steps.push('Real deterministic planner: no music and selected music, review, Apply/Undo/Redo');report.status='passed';console.log('PASS: real planner audio indication and history');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-draft-audio.json',JSON.stringify(report,null,2));await browser.close();}
