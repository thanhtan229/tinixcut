import { chromium } from 'playwright';
import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const path = process.env.TINIXCUT_CDP_PROJECT;
if (!path) throw new Error('Missing scoped temporary project path');
const recoveryPath = path.replace(/\.tcproj$/, '.tcproj.recovery');
const meta = path + '.test.json';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9444', { timeout: 10000 });
try {
  const page = browser.contexts().flatMap(c => c.pages()).find(p => /tauri|localhost/.test(p.url()));
  const invoke = (command, args = {}) => page.evaluate(({command,args}) => window.__TAURI_INTERNALS__.invoke(command,args), {command,args});
  const poll = async fn => { for(let i=0;i<60;i++){if(await fn())return;await new Promise(r=>setTimeout(r,250));} throw new Error('Native state polling timed out'); };
  if(process.argv[2] === 'prepare') {
    const project = await invoke('project_new',{name:'CDP Saved Recovery'});
    await invoke('project_save',{path});
    await page.reload();
    const later=page.getByRole('button',{name:'Later',exact:true}); if(await later.count())await later.click();
    await page.getByRole('button',{name:/\+ Track/}).click();
    await poll(async()=> { const s=await invoke('project_check_recovery',{path}); return Boolean(s.NewerRecoveryAvailable || s.RecoveryDiffersFromProject); });
    const current=await invoke('project_get_current');
    assert.equal(current.tracks.length,1);
    writeFileSync(meta,JSON.stringify({projectId:project.project_id,saved:readFileSync(path).toString('base64'),recovery:readFileSync(recoveryPath).toString('base64')}));
    console.log('PASS: saved project has real UI edit and autosaved recovery before restart');
  } else {
    const data=JSON.parse(readFileSync(meta,'utf8'));
    const status=await invoke('project_check_recovery',{path});
    const details=status.NewerRecoveryAvailable ?? status.RecoveryDiffersFromProject;
    assert.ok(details,'Recovery must survive restart');
    const args={path,expectedProjectRevision:details.project_revision,expectedRecoveryRevision:details.recovery_revision,expectedRecoveryFingerprint:details.recovery_fingerprint};
    await assert.rejects(()=>invoke('project_restore_recovery',{...args,expectedRecoveryFingerprint:'stale'}));
    assert.equal(readFileSync(path).toString('base64'),data.saved);
    assert.equal(readFileSync(recoveryPath).toString('base64'),data.recovery);
    const receipt=await invoke('project_restore_recovery',args);
    assert.equal(receipt.project.project_id,data.projectId);
    assert.equal(receipt.project.tracks.length,1);
    assert.equal(readFileSync(path).toString('base64'),data.saved);
    await page.reload();
    await page.getByRole('button',{name:/Save/,exact:false}).first().click();
    await poll(async()=>JSON.parse(readFileSync(path,'utf8')).tracks.length===1);
    const reopened=await invoke('project_open',{path});
    assert.equal(reopened.tracks.length,1);
    writeFileSync('artifacts/background/cdp-saved-recovery.json',JSON.stringify({status:'passed',scope:'real UI edit/autosave; process restart; native recovery check/stale fingerprint rejection/restore; UI Save; command reopen; picker/prompt unqualified',revision:reopened.revision},null,2));
    console.log('PASS: saved recovery survives restart; stale fingerprint preserves files; native restore/UI Save/reopen match');
  }
} finally {await browser.close();}
