import { chromium } from 'playwright';
import { writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});
const report={scope:'Clip boundary scope in isolated WebView',steps:[]};
const timer=setTimeout(()=>process.exit(124),60000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const get=()=>invoke('project_get_current');const seed=await get();
 const t=value=>({value,rate:seed.settings.frame_rate});
 const ops=[
  ...['a','b'].map(id=>({operation_id:`track_${id}`,operation:{type:'TrackCreate',payload:{track_id:id,name:`Track ${id}`,kind:'Video'}}})),
  ...[['first','a',1000],['second','a',1060],['other','b',1030]].map(([id,track_id,start])=>({operation_id:`clip_${id}`,operation:{type:'ClipCreate',payload:{track_id,item_id:id,name:`Boundary ${id}`,kind:'Text',timeline_start:t(start),duration:t(60)}}})),
  {operation_id:'lock_b',operation:{type:'TrackLockSet',payload:{track_id:'b',locked:true}}}
 ];
 await invoke('project_execute_command',{envelope:{schema_version:seed.schema_version,project_id:seed.project_id,base_revision:seed.revision,transaction_id:'scope_seed',operations:ops}});
 await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 const clip=id=>page.getByRole('group',{name:new RegExp(`^Boundary ${id}, Text clip`)});
 const go=async n=>{await page.getByLabel('Go to frame',{exact:true}).fill(String(n));await page.getByRole('button',{name:'Go',exact:true}).click();await page.waitForFunction(n=>document.querySelector('#preview-frame-seek')?.value===String(n),n);};
 const next=page.getByRole('button',{name:'Clip boundary ▶',exact:true});const previous=page.getByRole('button',{name:'◀ Clip boundary',exact:true});
 const expectFrame=async n=>page.waitForFunction(n=>document.querySelector('#preview-frame-seek')?.value===String(n),n,{timeout:5000});
 const poll=async n=>{for(let i=0;i<60;i++){const p=await get();if(p.tracks.find(t=>t.track_id==='a').items.find(i=>i.item_id==='second').timeline_range.start.value===n)return p;await new Promise(r=>setTimeout(r,100));}throw Error(`Expected second clip at ${n}`);};
 const scope=page.getByLabel('Clip boundary scope');await scope.waitFor();assert.equal(await scope.inputValue(),'all');
 await clip('second').focus();await clip('second').press('Enter');assert.match(await clip('second').getAttribute('aria-label'),/selected/);
 const initial=await get();await go(1000);await next.click();await expectFrame(1030);
 await scope.selectOption('selected_tracks');await go(1000);await next.click();await expectFrame(1060);
 await clip('second').focus();await page.keyboard.press('Control+Alt+,');await expectFrame(1000);await page.keyboard.press('Control+Alt+.');await expectFrame(1060);assert.deepEqual(await get(),initial);
 await clip('second').press('Shift+ArrowRight');await poll(1070);await go(1060);await clip('second').focus();await page.keyboard.press('Control+Alt+.');await expectFrame(1070);
 await page.getByRole('button',{name:/Undo/}).first().click();await poll(1060);await go(1060);await clip('second').focus();await page.keyboard.press('Control+Alt+.');await expectFrame(1120);
 await page.getByRole('button',{name:/Redo/}).first().click();await poll(1070);await go(1060);await clip('second').focus();await page.keyboard.press('Control+Alt+.');await expectFrame(1070);
 await clip('other').focus();await clip('other').press('Control+Enter');assert.match(await clip('other').getAttribute('aria-label'),/selected/);assert.match(await clip('second').getAttribute('aria-label'),/selected/);
 const before=await get();await go(1000);await next.click();await expectFrame(1030);assert.deepEqual(await get(),before);
 await page.locator('[data-track-lane="true"]').first().evaluate(el=>el.dispatchEvent(new MouseEvent('click',{bubbles:true,clientX:0,clientY:0})));
 await page.getByText('Select a clip to navigate its track',{exact:true}).waitFor();assert.equal(await next.isDisabled(),true);assert.equal(await previous.isDisabled(),true);
 const frame=await page.locator('#preview-frame-seek').inputValue();await page.locator('body').focus();await page.keyboard.press('Control+Alt+.');assert.equal(await page.locator('#preview-frame-seek').inputValue(),frame);
 await scope.selectOption('all');assert.equal(await next.isEnabled(),true);assert.deepEqual(await get(),before);
 report.steps.push('All tracks and selected tracks: buttons and shortcuts, locked track multiselection, no selection, move Undo/Redo and no navigation mutation');report.status='passed';console.log('PASS: clip boundary scopes');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-clip-boundary-scope.json',JSON.stringify(report,null,2));await browser.close();}
