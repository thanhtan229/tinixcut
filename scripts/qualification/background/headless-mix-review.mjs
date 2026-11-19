import assert from 'node:assert/strict';
import {build} from 'vite';import {chromium} from 'playwright';import {createServer} from 'node:http';import {readFileSync,existsSync,mkdirSync,writeFileSync} from 'node:fs';import {resolve,join,sep} from 'node:path';
const dir=resolve('artifacts/background/mix-review');mkdirSync(dir,{recursive:true});const site=join(dir,'site');await build({configFile:false,root:process.cwd(),logLevel:'error',build:{outDir:site,rollupOptions:{input:resolve('scripts/qualification/background/headless-mix-review.html')}}});
const server=createServer((req,res)=>{const path=resolve(site,'.'+new URL(req.url,'http://localhost').pathname);if(!path.startsWith(site+sep)||!existsSync(path))return res.writeHead(404).end();res.setHeader('Content-Type',path.endsWith('.html')?'text/html':'text/javascript');res.end(readFileSync(path));});await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await chromium.launch({headless:true,args:['--mute-audio']});let result={status:'failed'};
async function checkBulkSelection(page) {
 for(const [group,label] of [['voice',/voice · Voice/],['music',/music · Music/]]) {
  const scope=page.getByRole('group',{name:group==='voice'?'Voice clips':'Music clips'});
  await page.getByRole('button',{name:`Clear ${group} selection`,exact:true}).click();
  assert.equal(await scope.getByRole('checkbox',{name:label}).isChecked(),false);
  await page.getByRole('button',{name:`Select all ${group} clips`,exact:true}).click();
  const checkbox=scope.getByRole('checkbox',{name:label});
  assert.equal(await checkbox.isChecked(),!(await checkbox.isDisabled()));
 }
}
const bulkPage=await browser.newPage();
try {
 let bulkPlan;
 await bulkPage.exposeFunction('mixInvoke',(cmd,args)=>{if(cmd==='project_dry_run'){bulkPlan=args.envelope;return {projectId:'mix',baseRevision:1,operationsEvaluated:args.envelope.operations.length,changes:[],warnings:[]};}throw new Error(cmd);});
 await bulkPage.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-mix-review.html?bulk=1`);
 for(const group of ['voice','music']) {
  const entries=bulkPage.getByRole('group',{name:group==='voice'?'Voice clips':'Music clips'}).getByRole('checkbox');
  await bulkPage.getByRole('button',{name:`Clear ${group} selection`,exact:true}).click();
  assert.ok((await entries.evaluateAll(es=>es.map(e=>e.checked))).every(x=>!x));
  await bulkPage.getByRole('button',{name:`Select all ${group} clips`,exact:true}).click();
  assert.ok((await entries.evaluateAll(es=>es.map(e=>e.checked||e.disabled||e.closest('label').innerText.includes('assigned to music')))).every(Boolean));
 }
 await bulkPage.getByRole('button',{name:'Clear music selection',exact:true}).click();
 await bulkPage.getByRole('group',{name:'Music clips'}).getByRole('checkbox',{name:/music · Music/}).check();
 await bulkPage.getByRole('checkbox',{name:'Normalize music from source audio levels'}).uncheck();
 await bulkPage.getByRole('button',{name:'Build review plan'}).click();
 await bulkPage.getByRole('button',{name:'Change settings'}).waitFor();
 assert.ok(bulkPlan.operations.length>0);
 assert.ok(bulkPlan.operations.every(({operation})=>operation.payload.item_id==='music'),'plan affects selected music only');
 assert.equal(await bulkPage.getByRole('button',{name:'Select all music clips'}).isDisabled(),true);
} finally {await bulkPage.close();}
const voicePage=await browser.newPage();
try {
 let voicePlan;
 await voicePage.exposeFunction('mixInvoke',(cmd,args)=>{if(cmd==='project_dry_run'){voicePlan=args.envelope;return {projectId:'mix',baseRevision:1,operationsEvaluated:voicePlan.operations.length,changes:[],warnings:[]};}throw new Error(cmd);});
 await voicePage.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-mix-review.html?voiceover=1`);
 const voice=voicePage.getByRole('group',{name:'Voice clips'}),music=voicePage.getByRole('group',{name:'Music clips'});
 const voiceNarration=voice.getByRole('checkbox',{name:/voiceover · Narration/}),musicNarration=music.getByRole('checkbox',{name:/voiceover · Narration/});
 assert.equal(await musicNarration.isChecked(),true);assert.equal(await voiceNarration.isChecked(),false);
 await voicePage.getByRole('button',{name:'Clear voice selection'}).click();
 await voiceNarration.check();assert.equal(await musicNarration.isChecked(),false);
 await voicePage.getByRole('button',{name:'Select all music clips'}).click();assert.equal(await musicNarration.isChecked(),false);
 await musicNarration.check();assert.equal(await voiceNarration.isChecked(),false);
 await voiceNarration.check();assert.equal(await musicNarration.isChecked(),false);
 await voicePage.getByRole('checkbox',{name:'Normalize music from source audio levels'}).uncheck();
 await voicePage.getByRole('button',{name:'Build review plan'}).click();
 await voicePage.getByRole('button',{name:'Change settings'}).waitFor();
 assert.ok(voicePlan.operations.every(({operation})=>operation.payload.item_id==='music'));
 assert.deepEqual(voicePlan.operations.map(({operation})=>operation.payload.keyframe.offset.value),[0,83,90,180,203]);
} finally {await voicePage.close();}
try{const page=await browser.newPage();let applied=0,plans=[];const levels=JSON.parse(readFileSync(join(dir,'real-levels.json'),'utf8'));await page.exposeFunction('mixInvoke',(cmd,args)=>{if(cmd==='media_analyze_audio_levels')return levels;if(cmd==='project_dry_run'){plans.push(args);return {projectId:'mix',baseRevision:1,operationsEvaluated:args.envelope.operations.length,changes:[],warnings:[]};}throw new Error(cmd)});await page.exposeFunction('mixApply',receipt=>{applied++;writeFileSync(join(dir,'real-plan.json'),JSON.stringify(receipt,null,2));});await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-mix-review.html`);await checkBulkSelection(page);await page.getByRole('button',{name:'Build review plan'}).click();await page.getByRole('button',{name:'Change settings'}).waitFor();assert.equal(applied,0);assert.equal(await page.getByRole('button',{name:'Clear music selection',exact:true}).isDisabled(),true);assert.match(await page.locator('.draft-preview').innerText(),/music.*volume keyframes for ducking/i);await page.getByRole('button',{name:'Change settings'}).click();assert.equal(await page.getByRole('button',{name:'Apply reviewed mix'}).count(),0);assert.equal(await page.getByRole('group',{name:'Music clips'}).getByRole('checkbox',{name:/music · Music/}).isChecked(),true);await page.getByLabel('Music duck under voice (dB)').fill('-6');await page.getByRole('button',{name:'Build review plan'}).click();await page.getByRole('button',{name:'Apply reviewed mix'}).click();assert.equal(applied,1);assert.equal(plans.length,2);assert.notDeepEqual(plans[0],plans[1]);assert.ok(plans[0].envelope.operations.every(a=>plans[1].envelope.operations.every(b=>a.operation_id!==b.operation_id)),'each review gets unique operation IDs');const appliedReceipt=JSON.parse(readFileSync(join(dir,'real-plan.json'),'utf8'));assert.deepEqual(appliedReceipt.plan.operations.map(op=>op.operationId),plans[1].envelope.operations.map(op=>op.operation_id),'review and Apply retain identical operation IDs');await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-mix-review.html?locked=1`);const locked=page.getByRole('group',{name:'Music clips'}).getByRole('checkbox',{name:/music · Music.*locked track/});assert.equal(await locked.isDisabled(),true);assert.equal(await locked.isChecked(),false);await checkBulkSelection(page);await page.getByRole('checkbox',{name:'Normalize music from source audio levels'}).uncheck();await page.getByRole('button',{name:'Build review plan'}).click();await page.getByRole('alert').waitFor();assert.equal(applied,1);result={status:'passed',scope:'Real audio mix dialog/plan builder, mocked dry-run/apply IPC; no playback'};console.log('PASS: revise audio mix settings before apply');}finally{await browser.close();await new Promise(r=>server.close(r));writeFileSync(join(dir,'report.json'),JSON.stringify(result,null,2));}
