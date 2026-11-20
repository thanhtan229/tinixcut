import { chromium } from 'playwright';
import { writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9445',{timeout:10000});
const report={scope:'Caption pending count on isolated WebView',steps:[]};
const timer=setTimeout(()=>process.exit(124),60000);
try {
 const page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
 const get=()=>invoke('project_get_current');const p=await get();const t=value=>({value,rate:p.settings.frame_rate});
 const ops=[];for(const [track,id,start,text] of [['count_a','ca',0,'Alpha'],['count_a','cb',90,'Beta'],['count_b','cc',0,'Gamma']]){if(!ops.some(o=>o.type==='TrackCreate'&&o.payload.track_id===track))ops.push({type:'TrackCreate',payload:{track_id:track,name:track,kind:'Video'}});ops.push({type:'ClipCreate',payload:{track_id:track,item_id:id,name:id,kind:'Text',timeline_start:t(start),duration:t(30)}},{type:'ClipTextSet',payload:{item_id:id,text}});}
 await invoke('project_execute_command',{envelope:{schema_version:p.schema_version,project_id:p.project_id,base_revision:p.revision,transaction_id:'count_seed',operations:ops.map((operation,i)=>({operation_id:`count_${i}`,operation}))}});
 await page.reload();const later=page.getByRole('button',{name:'Later',exact:true});try{await later.waitFor({timeout:2000});await later.click();}catch(e){if(e.name!=='TimeoutError')throw e;}
 await page.getByRole('button',{name:/Caption editor/}).click();const dialog=page.getByRole('dialog',{name:'Caption editor'});await dialog.waitFor();
 const rows=dialog.locator('article');const row=id=>rows.filter({has:page.getByLabel(`Start frame ${id}`,{exact:true})});
 const total=n=>dialog.getByText(`${n} ${n===1?'caption has':'captions have'} pending edits across all tracks.`,{exact:true});
 await row('ca').locator('textarea').fill('Alpha changed');await row('ca').getByLabel('Start frame ca').fill('10');await total(1).waitFor();
 await row('cc').locator('textarea').fill('Gamma changed');await total(2).waitFor();
 await dialog.getByLabel('Filter caption track').selectOption('count_a');await total(2).waitFor();await dialog.getByText('1 caption has pending edits hidden by filters.').waitFor();
 await dialog.getByLabel('Search captions').fill('no match');await total(2).waitFor();await dialog.getByText('2 captions have pending edits hidden by filters.').waitFor();
 await dialog.getByLabel('Filter caption timing').selectOption('edited');
 const beforeClear=await get();await dialog.getByRole('button',{name:'Clear caption search',exact:true}).click();
 assert.equal(await dialog.getByLabel('Search captions').inputValue(),'');assert.equal(await dialog.getByLabel('Filter caption track').inputValue(),'count_a');assert.equal(await dialog.getByLabel('Filter caption timing').inputValue(),'edited');assert.equal(await row('ca').locator('textarea').inputValue(),'Alpha changed');assert.equal(await row('ca').getByLabel('Start frame ca').inputValue(),'10');await dialog.getByText('1 caption has pending edits hidden by filters.').waitFor();assert.deepEqual(await get(),beforeClear);
 await dialog.getByLabel('Search captions').fill('no match');await dialog.getByText('2 captions have pending edits hidden by filters.').waitFor();
 const beforeShow=await get();await dialog.getByRole('button',{name:'Show all pending edits',exact:true}).click();
 assert.equal(await dialog.getByLabel('Filter caption track').inputValue(),'');assert.equal(await dialog.getByLabel('Search captions').inputValue(),'');assert.equal(await dialog.getByLabel('Filter caption timing').inputValue(),'edited');assert.equal(await rows.count(),2);await total(2).waitFor();
 assert.equal(await row('ca').locator('textarea').inputValue(),'Alpha changed');assert.equal(await row('ca').getByLabel('Start frame ca').inputValue(),'10');assert.equal(await row('cc').locator('textarea').inputValue(),'Gamma changed');assert.deepEqual(await get(),beforeShow);
 await dialog.getByRole('button',{name:'Apply edits',exact:true}).click();
 const poll=async predicate=>{for(let i=0;i<100;i++){const x=await get();if(predicate(x))return x;await new Promise(r=>setTimeout(r,100));}throw Error('project state timeout');};
 const item=(project,id)=>project.tracks.flatMap(track=>track.items).find(x=>x.item_id===id);
 await poll(x=>item(x,'ca').text_content==='Alpha changed'&&item(x,'cc').text_content==='Gamma changed'&&item(x,'ca').timeline_range.start.value===10);
 await total(2).waitFor({state:'hidden'});await dialog.getByRole('button',{name:'Close caption editor'}).click();await page.getByRole('button',{name:/Undo/}).first().click();
 await poll(x=>item(x,'ca').text_content==='Alpha'&&item(x,'cc').text_content==='Gamma'&&item(x,'ca').timeline_range.start.value===0);
 await page.getByRole('button',{name:/Caption editor/}).click();await dialog.waitFor();await dialog.getByLabel('Filter caption timing').selectOption('all');await page.waitForFunction(()=>Array.from(document.querySelectorAll('textarea')).some(e=>e.value==='Alpha'),null,{timeout:5000});assert.equal(await total(1).count(),0);assert.equal(await total(2).count(),0);assert.equal(await row('ca').locator('textarea').inputValue(),'Alpha');assert.equal(await row('cc').locator('textarea').inputValue(),'Gamma');
 await dialog.getByRole('button',{name:'Close caption editor'}).click();await page.getByRole('button',{name:/Redo/}).first().click();
 await poll(x=>item(x,'ca').text_content==='Alpha changed'&&item(x,'cc').text_content==='Gamma changed'&&item(x,'ca').timeline_range.start.value===10);
 await page.getByRole('button',{name:/Caption editor/}).click();await dialog.waitFor();await dialog.getByLabel('Filter caption timing').selectOption('all');await page.waitForFunction(()=>Array.from(document.querySelectorAll('textarea')).some(e=>e.value==='Alpha changed'),null,{timeout:5000});assert.equal(await total(2).count(),0);assert.equal(await row('cc').locator('textarea').inputValue(),'Gamma changed');
 report.steps.push('Two tracks, hidden drafts revealed without mutation; Apply, Undo and Redo through Command Service');report.status='passed';console.log('PASS: show pending edits and Apply/Undo/Redo');
}catch(e){report.status='failed';report.error=String(e);throw e;}finally{clearTimeout(timer);writeFileSync('artifacts/background/cdp-caption-count.json',JSON.stringify(report,null,2));await browser.close();}
