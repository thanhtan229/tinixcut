import { chromium } from 'playwright';
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
const path=process.env.TINIXCUT_CDP_PROJECT+'.source-shortcuts.wav';
execFileSync(resolve('src-tauri/resources/bin/ffmpeg.exe'),['-v','error','-y','-f','lavfi','-i','anullsrc=r=8000:cl=mono','-t','1','-c:a','pcm_s16le',path],{windowsHide:true,timeout:10000});
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});const report={scope:'Source frame shortcuts in isolated WebView',steps:[]};const timer=setTimeout(()=>process.exit(124),60000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});const get=()=>invoke('project_get_current');
 const asset=await invoke('media_import',{filePath:path});await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 await page.getByRole('button',{name:'Preview',exact:true}).click();const dialog=page.getByRole('dialog',{name:'Source viewer'});await dialog.waitFor();await page.waitForFunction(()=>document.querySelector('audio')?.readyState>=1);
 const frame=async n=>page.waitForFunction(n=>Math.abs(document.querySelector('audio').currentTime-n/30)<0.002,n,{timeout:5000});
 const before=await get();await dialog.focus();await page.keyboard.press('ArrowRight');await frame(1);await page.keyboard.press('ArrowRight');await frame(2);await page.keyboard.press('ArrowLeft');await frame(1);
 await page.getByLabel('Source In seconds').focus();await page.keyboard.press('ArrowRight');await frame(1);
 await page.evaluate(()=>{window.arrowGuard=null;document.querySelector('audio').addEventListener('keydown',e=>{if(e.key==='ArrowRight')queueMicrotask(()=>window.arrowGuard=e.defaultPrevented);},{once:true});});
 await page.locator('audio').focus();await page.keyboard.press('ArrowRight');await page.waitForFunction(()=>window.arrowGuard!==null);assert.equal(await page.evaluate(()=>window.arrowGuard),false,'media control owns its arrow key');
 await dialog.getByRole('button',{name:'Go to source start'}).click();await frame(0);await dialog.focus();await page.keyboard.press('ArrowRight');await frame(1);await page.keyboard.press('ArrowRight');await frame(2);await page.keyboard.press('i');
 for(let i=0;i<6;i++)await page.keyboard.press('ArrowRight');await frame(8);await page.keyboard.press('o');assert.ok(Math.abs(Number(await page.getByLabel('Source In seconds').inputValue())-2/30)<0.002);assert.ok(Math.abs(Number(await page.getByLabel('Source Out seconds').inputValue())-8/30)<0.002);assert.deepEqual(await get(),before);
 await dialog.getByRole('button',{name:'Add range to timeline'}).click();
 const poll=async pred=>{for(let i=0;i<80;i++){const p=await get();if(pred(p))return p;await new Promise(r=>setTimeout(r,100));}throw Error('source range state timeout');};
 const find=p=>p.tracks.flatMap(t=>t.items).find(i=>i.asset_id===asset.asset_id);const applied=await poll(p=>!!find(p));assert.equal(find(applied).source_range.start.value,2);assert.equal(find(applied).timeline_range.duration.value,6);
 await page.getByRole('button',{name:/Undo/}).first().click();const undone=await poll(p=>!find(p));assert.deepEqual(undone.tracks,before.tracks);await page.getByRole('button',{name:/Redo/}).first().click();const redone=await poll(p=>!!find(p));assert.deepEqual(redone.tracks,applied.tracks);
 await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>!find(p));
 for(const [sourceIn,sourceOut,start,end] of [['0.0166','0.0501',0,2],['0.0167','0.0501',1,2]]) {
  await page.getByRole('button',{name:'Preview',exact:true}).click();await dialog.waitFor();
  await page.getByLabel('Source In seconds').fill(sourceIn);await page.getByLabel('Source Out seconds').fill(start===0?'0.01665':'0.017');
  assert.equal(await dialog.getByRole('button',{name:'Add range to timeline'}).isDisabled(),true);assert.equal(await dialog.getByLabel('Frame-aligned source range').count(),0);
  await page.getByLabel('Source Out seconds').fill(sourceOut);assert.equal(await dialog.getByLabel('Frame-aligned source range').textContent(),`Source frames ${start}–${end} (end exclusive)`);
  const base=await get();await dialog.getByRole('button',{name:'Add range to timeline'}).click();const added=await poll(p=>!!find(p));assert.equal(find(added).source_range.start.value,start);assert.equal(find(added).timeline_range.duration.value,end-start);
  await page.getByRole('button',{name:/Undo/}).first().click();const back=await poll(p=>!find(p));assert.deepEqual(back.tracks,base.tracks);
  await page.getByRole('button',{name:/Redo/}).first().click();const again=await poll(p=>!!find(p));assert.deepEqual(again.tracks,added.tracks);
  await page.getByRole('button',{name:/Undo/}).first().click();await poll(p=>!find(p));
 }
 report.steps.push('Dialog arrows, input and media guards, keyboard In/Out, add range and Undo/Redo via Command Service');report.status='passed';console.log('PASS: source frame shortcuts and range history');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-source-shortcuts.json',JSON.stringify(report,null,2));await browser.close();}
