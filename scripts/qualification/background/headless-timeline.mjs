import assert from 'node:assert/strict';
import { createReadStream,existsSync,mkdirSync,writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { resolve,join,sep } from 'node:path';
import { build } from 'vite';
import { chromium } from 'playwright';
const directory=resolve('artifacts/background/timeline-navigation',new Date().toISOString().replaceAll(':','-'));mkdirSync(directory,{recursive:true});
const site=join(directory,'site');let browser,server;
const report={startedAt:new Date().toISOString(),scope:'Production React Timeline and shortcut hook in headless Chromium; asset-free fixtures, no native/app/picker/audio claim',steps:[],errors:[]};
try{
 await build({configFile:false,root:process.cwd(),logLevel:'error',build:{outDir:site,rollupOptions:{input:resolve('scripts/qualification/background/headless-timeline.html')}}});
 server=createServer((req,res)=>{const path=resolve(site,'.'+new URL(req.url,'http://localhost').pathname);if(!path.startsWith(site+sep)||!existsSync(path)){res.writeHead(404).end();return;}res.setHeader('Content-Type',path.endsWith('.html')?'text/html':path.endsWith('.css')?'text/css':'text/javascript');createReadStream(path).pipe(res);});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));browser=await chromium.launch({headless:true,args:['--mute-audio']});
 const page=await browser.newPage({viewport:{width:1280,height:720}});page.on('pageerror',e=>report.errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-timeline.html`);
 const state=()=>page.evaluate(()=>window.timelineTest.snapshot());
 const viewport=page.locator('.timeline-scroll-viewport');const zoom=()=>page.getByLabel('Timeline zoom').inputValue().then(Number);
 const geometry=()=>page.evaluate(()=>{
  const v=document.querySelector('.timeline-scroll-viewport'),r=v.getBoundingClientRect();
  return {width:v.clientWidth,scroll:v.scrollLeft,left:r.left,right:r.right,ticks:document.querySelectorAll('.timeline-ruler svg g').length,clips:[...document.querySelectorAll('[data-timeline-clip]')].map(e=>{const b=e.getBoundingClientRect();return {label:e.getAttribute('aria-label'),left:b.left,right:b.right};})};
 });
 const settled=()=>page.waitForTimeout(100);
 const marker=page.getByRole('button',{name:'Marker One at frame 30',exact:true});
 page.once('dialog',dialog=>dialog.accept('Revised marker'));
 await marker.focus();await marker.press('F2');await marker.press('Delete');await marker.press('Backspace');
 assert.deepEqual(await page.evaluate(()=>window.markerActions),[['rename','m1','Revised marker'],['delete','m1'],['delete','m1']]);
 assert.equal((await state()).mutations,0,'marker keys must not trigger clip edit handlers');
 report.steps.push({case:'focused marker keyboard rename/delete routes to marker callbacks'});
 const initial=await state();await page.locator('body').click({position:{x:2,y:700}});await page.keyboard.press('i');await settled();assert.deepEqual((await state()).marks,{markIn:0,markOut:null});assert.equal(await page.getByRole('button',{name:'Go to In',exact:true}).isEnabled(),true);assert.equal(await page.getByRole('button',{name:'Go to Out',exact:true}).isDisabled(),true);await page.getByRole('button',{name:'Late playhead'}).click();await page.locator('body').click({position:{x:2,y:700}});await page.keyboard.press('o');await settled();assert.deepEqual((await state()).marks,{markIn:0,markOut:53800});await page.getByRole('button',{name:'Go to In',exact:true}).click();await settled();assert.equal((await state()).playhead,0);await page.locator('body').click({position:{x:2,y:700}});await page.keyboard.press('Alt+i');await settled();assert.deepEqual((await state()).marks,{markIn:null,markOut:53800});assert.equal(await page.getByRole('button',{name:'Go to In',exact:true}).isDisabled(),true);assert.equal(await page.getByRole('button',{name:'Go to Out',exact:true}).isEnabled(),true);assert.equal((await state()).project,initial.project);assert.equal((await state()).mutations,initial.mutations);await page.getByRole('button',{name:'Clear test marks'}).click();await page.getByRole('button',{name:'Reset test playhead'}).click();report.steps.push({case:'I/O and clear with independent boundary navigation, no project edit'});
 assert.equal(await page.getByRole('button',{name:'Fit In/Out',exact:true}).isDisabled(),true);assert.equal(await page.getByRole('button',{name:'Go to In',exact:true}).isDisabled(),true);assert.equal(await page.getByRole('button',{name:'Go to Out',exact:true}).isDisabled(),true);await page.getByRole('button',{name:'Set test marks'}).click();const beforeBoundary=await state();await page.getByRole('button',{name:'Go to In',exact:true}).click();await settled();assert.equal((await state()).playhead,30000);await page.getByRole('button',{name:'Go to Out',exact:true}).click();await settled();assert.equal((await state()).playhead,36000);assert.equal((await state()).project,beforeBoundary.project);assert.equal((await state()).mutations,beforeBoundary.mutations);const fitMarks=page.getByRole('button',{name:'Fit In/Out',exact:true});await fitMarks.click();await settled();assert.equal(await fitMarks.getAttribute('aria-pressed'),'true');let marked=await geometry();let z=Math.exp(await zoom());assert.ok(30000*z-marked.scroll>=0&&36000*z-marked.scroll<=marked.width-160);await page.setViewportSize({width:720,height:720});await settled();marked=await geometry();z=Math.exp(await zoom());assert.ok(30000*z-marked.scroll>=0&&36000*z-marked.scroll<=marked.width-160);await page.getByRole('button',{name:'Clear test marks'}).click();await settled();assert.equal(await fitMarks.isDisabled(),true);assert.equal(await fitMarks.getAttribute('aria-pressed'),'false');report.steps.push({case:'Fit In/Out follows resize and clears safely'});await page.getByRole('button',{name:'Reset test playhead'}).click();
 await page.locator('body').click({position:{x:2,y:700}});await page.keyboard.press('Alt+.');await settled();assert.equal((await state()).playhead,30);await page.keyboard.press('Alt+.');await settled();assert.equal((await state()).playhead,90);await page.keyboard.press('Alt+,');await settled();assert.equal((await state()).playhead,30);await page.getByLabel('Typing guard').focus();await page.keyboard.press('Alt+.');assert.equal((await state()).playhead,30);report.steps.push({case:'marker shortcuts navigate and suppress text input'});
 const markerBaseline=await state();await page.getByRole('button',{name:'Marker ▶',exact:true}).click();await settled();assert.equal((await state()).playhead,90);assert.equal(await page.getByRole('button',{name:'Marker ▶',exact:true}).isDisabled(),true);await page.getByRole('button',{name:'◀ Marker',exact:true}).click();await settled();assert.equal((await state()).playhead,30);assert.equal(await page.getByRole('button',{name:'◀ Marker',exact:true}).isDisabled(),true);assert.equal((await state()).project,markerBaseline.project);assert.equal((await state()).mutations,markerBaseline.mutations);report.steps.push({case:'sample marker button navigation and edge guards preserve project'});await page.locator('[data-timeline-clip]').first().focus();await page.keyboard.press('Alt+.');await settled();assert.equal((await state()).playhead,90);report.steps.push({case:'marker shortcut with focused timeline clip'});
 await page.getByRole('button',{name:'Duplicate markers'}).click();const markerList=page.getByLabel('Go to marker');
 assert.deepEqual(await markerList.locator('option').allTextContents(),['Go to marker…','Same — frame 30','Same — frame 90']);
 const listBaseline=await state();await markerList.selectOption('m1');await settled();assert.equal((await state()).playhead,30);assert.equal(await markerList.inputValue(),'');
 await markerList.selectOption('m2');await settled();assert.equal((await state()).playhead,90);await markerList.selectOption('m1');await settled();assert.equal((await state()).playhead,30);
 assert.equal((await state()).project,listBaseline.project);assert.deepEqual((await state()).selected,listBaseline.selected);assert.equal((await state()).mutations,listBaseline.mutations);
 report.steps.push({case:'direct marker navigation sorts duplicate labels and preserves project and selection'});
 await page.getByRole('button',{name:'Fit all',exact:true}).click();await settled();const before=await state();
 for(const width of [360,720,1280]){
  await page.setViewportSize({width,height:720});await settled();const g=await geometry();
  assert.ok(g.clips.every(c=>c.left>=g.left+159&&c.right<=g.right-8),JSON.stringify({width,first:g.clips[0],last:g.clips.at(-1),right:g.right}));assert.ok(g.ticks<width/8+100);
  report.steps.push({case:'30-minute/300-clip fit and resize',width,ticks:g.ticks});
 }
 assert.deepEqual(await state(),before);
 await page.getByRole('button',{name:'Select late clips',exact:true}).click();let selected=await state();
 await page.getByRole('button',{name:'Go to selection start',exact:true}).click();await settled();
 assert.equal((await state()).playhead,53640);
 await page.getByRole('button',{name:'Go to selection last frame',exact:true}).click();await settled();
 assert.equal((await state()).playhead,53999);
 assert.equal((await state()).project,selected.project);assert.deepEqual((await state()).selected,selected.selected);
 await page.getByRole('button',{name:'Reset test playhead'}).click();await settled();
 selected=await state();
 await page.getByRole('button',{name:'Fit selection',exact:true}).click();await settled();let g=await geometry();
 assert.ok(g.clips.filter(c=>/Caption 29[89],/.test(c.label)).every(c=>c.left>=g.left+159&&c.right<g.right-8));assert.ok(g.scroll>0);assert.deepEqual(await state(),selected);
 await page.setViewportSize({width:720,height:720});await settled();g=await geometry();
 assert.ok(g.clips.filter(c=>/Caption 29[89],/.test(c.label)).every(c=>c.left>=g.left+159&&c.right<g.right-8));
 await page.getByRole('button',{name:'Late playhead',exact:true}).click();await settled();const beforeZoom=await state();
 const anchor=async()=>{const g=await geometry();return 160+53800*Math.exp(await zoom())-g.scroll;};const x=await anchor();
 await page.locator('body').click({position:{x:2,y:700}});
 for(const key of ['=','=','-','-']){await page.keyboard.press(key);await settled();assert.ok(Math.abs(await anchor()-x)<1.1);}
 const slider=page.getByLabel('Timeline zoom');const oldSlider=await zoom();await slider.press('ArrowRight');await settled();assert.notEqual(await zoom(),oldSlider);assert.ok(Math.abs(await anchor()-x)<1.1);
 assert.deepEqual(await state(),beforeZoom);
 const manualZoom=await zoom();await page.setViewportSize({width:1100,height:720});await settled();assert.equal(await zoom(),manualZoom);
 const input=page.getByLabel('Typing guard');await input.fill('');await input.press('-');assert.equal(await zoom(),manualZoom);
 report.steps.push({case:'late multi-track selection, keyboard anchor, manual resize and typing guard'});
 await page.getByRole('button',{name:'Stale selection',exact:true}).click();assert.equal(await page.getByRole('button',{name:'Fit selection',exact:true}).isDisabled(),true);
 assert.equal(await page.getByRole('button',{name:'Go to selection start',exact:true}).isDisabled(),true);
 assert.equal(await page.getByRole('button',{name:'Go to selection last frame',exact:true}).isDisabled(),true);
 await page.getByRole('button',{name:'Ten minutes',exact:true}).click();await page.getByRole('button',{name:'Fit all',exact:true}).click();await settled();g=await geometry();assert.ok(g.clips.every(c=>c.right<=g.right-8));
 await page.screenshot({path:join(directory,'ten-minute-fit.png')});
 await page.getByRole('button',{name:'Single frame',exact:true}).click();await page.getByRole('button',{name:'Fit selection',exact:true}).click();await settled();g=await geometry();assert.ok(g.clips[0].left>=g.left+159&&g.clips[0].right<g.right-8);assert.ok(Math.abs(Math.exp(await zoom())-10)<1e-12);
 await page.getByRole('button',{name:'Go to selection last frame',exact:true}).click();await settled();assert.equal((await state()).playhead,0);
 await page.getByRole('button',{name:'Empty project',exact:true}).click();await page.getByRole('button',{name:'Fit all',exact:true}).click();await settled();assert.equal((await geometry()).scroll,0);assert.ok(Math.abs(Math.exp(await zoom())-3)<1e-12);
 assert.equal(await markerList.isDisabled(),true);
 assert.equal((await state()).mutations,0);assert.equal(report.errors.length,0,JSON.stringify(report.errors));
 report.steps.push({case:'10-minute fit, reduced timebase clip, stale selection, single-frame/empty project, zero edit calls and zero page errors'});report.status='passed';console.log('PASS: headless timeline navigation and shortcut integration');
}catch(error){report.status='failed';report.error=String(error);console.error(error);process.exitCode=1;}
finally{if(browser)await browser.close();if(server?.listening)await new Promise(r=>server.close(r));report.finishedAt=new Date().toISOString();writeFileSync(join(directory,'report.json'),JSON.stringify(report,null,2));writeFileSync('artifacts/background/timeline-navigation-latest.json',JSON.stringify({...report,directory},null,2));}
