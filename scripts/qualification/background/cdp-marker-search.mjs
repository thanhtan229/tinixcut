import { chromium } from 'playwright';
import { writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});
const report={scope:'Marker search counts on isolated WebView',steps:[]};
const timer=setTimeout(()=>process.exit(124),60000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const get=()=>invoke('project_get_current');const seed=await get();
 const operations=Array.from({length:30},(_,i)=>({operation_id:`marker_${i}`,operation:{type:'ProjectMarkerSet',payload:{marker_id:`marker_${i}`,label:`Review ${String(i===28?29:i).padStart(2,'0')}`,position:{value:i*300,rate:seed.settings.frame_rate}}}}));
 await invoke('project_execute_command',{envelope:{schema_version:seed.schema_version,project_id:seed.project_id,base_revision:seed.revision,transaction_id:'search_seed',operations}});
 await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 const search=page.getByLabel('Find marker');const picker=page.getByLabel('Go to marker');const status=page.getByRole('status');
 await search.fill(' REVIEW 29 ');await status.waitFor();assert.equal(await status.textContent(),'2 of 30 markers');assert.equal(await picker.locator('option').count(),3);
 const next=page.getByRole('button',{name:'Marker ▶',exact:true});const previous=page.getByRole('button',{name:'◀ Marker',exact:true});
 const expectFrame=async frame=>page.waitForFunction(frame=>document.querySelector('#preview-frame-seek')?.value===String(frame),frame,{timeout:5000});
 const press=async chord=>{await page.evaluate(()=>document.activeElement?.blur());await page.keyboard.press(chord);};
 const navProject=await get();await next.click();await expectFrame(8400);await press('Alt+.');await expectFrame(8700);await previous.click();await expectFrame(8400);await press('Alt+,');await expectFrame(8400);assert.deepEqual(await get(),navProject);
 const before=await get();await picker.selectOption('marker_29');await page.waitForFunction(()=>document.querySelector('#preview-frame-seek')?.value==='8700');assert.deepEqual(await get(),before);
 await page.getByRole('button',{name:'◆ Marker',exact:true}).click();await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='2 of 31 markers');
 const added=(await get()).markers.find(m=>m.label==='Marker 1');assert.ok(added);
 const marker=page.getByRole('button',{name:/^Marker Marker 1 at frame /});await marker.focus();page.once('dialog',d=>d.accept('Review 29'));await page.keyboard.press('F2');await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='3 of 31 markers');assert.equal(await picker.locator('option').count(),4);
 const renamed=page.getByRole('button',{name:/^Marker Review 29 at frame 8700/});assert.equal(await renamed.count(),2);
 await renamed.last().focus();await page.keyboard.press('Delete');await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='2 of 30 markers');
 const oldMatch=page.getByRole('button',{name:'Marker Review 29 at frame 8400',exact:true});await oldMatch.focus();page.once('dialog',d=>d.accept('Changed'));await page.keyboard.press('F2');await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='1 of 30 markers');
 const afterRename=await get();await page.getByLabel('Go to frame',{exact:true}).fill('0');await page.getByRole('button',{name:'Go',exact:true}).click();await expectFrame(0);await next.click();await expectFrame(8700);assert.equal(await previous.isDisabled(),true);await press('Alt+,');await expectFrame(8700);assert.deepEqual(await get(),afterRename);
 await page.getByRole('button',{name:/Undo/}).first().click();await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='2 of 30 markers');
 await page.getByRole('button',{name:/Undo/}).first().click();await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='3 of 31 markers');
 await page.getByRole('button',{name:/Redo/}).first().click();await page.waitForFunction(()=>document.querySelector('[role="status"]')?.textContent==='2 of 30 markers');
 await search.fill('unmatched');assert.equal(await status.textContent(),'0 of 30 markers');assert.equal(await picker.isDisabled(),true);
 assert.equal(await next.isDisabled(),true);assert.equal(await previous.isDisabled(),true);const noMatchFrame=await page.locator('#preview-frame-seek').inputValue();await search.focus();await page.keyboard.press('Alt+.');assert.equal(await page.locator('#preview-frame-seek').inputValue(),noMatchFrame);
 const unchanged=await get();const frame=await page.locator('#preview-frame-seek').inputValue();await page.getByRole('button',{name:'Clear marker search'}).click();assert.equal(await search.inputValue(),'');assert.equal(await status.count(),0);assert.equal(await picker.locator('option').count(),31);assert.equal(await page.locator('#preview-frame-seek').inputValue(),frame);assert.deepEqual(await get(),unchanged);
 report.steps.push('30 markers, duplicate names, active-filter add/rename/delete and Undo/Redo, no results, clear and navigation preserve project');report.status='passed';console.log('PASS: marker search counts and clear');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-marker-search.json',JSON.stringify(report,null,2));await browser.close();}
