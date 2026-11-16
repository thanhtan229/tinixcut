import { chromium } from 'playwright';
import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const stage = process.argv[2];
const path = process.env.TINIXCUT_CDP_PROJECT;
const meta = path + '.test.json';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9444', { timeout: 10000 });
try {
  const page = browser.contexts().flatMap(c => c.pages()).find(p => /tauri|localhost/.test(p.url()));
  if (!page) throw new Error('Missing qualification page');
  const invoke = (command, args = {}) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  if (stage === 'prepare') {
    const settings = await invoke('app_settings_get');
    const project = await invoke('project_new', { name: 'CDP Recovery ' + Date.now() });
    writeFileSync(meta, JSON.stringify({ settings, projectId: project.project_id, name: project.name }));
    await page.reload();
    const later = page.getByRole('button', { name: 'Later', exact: true });
    if (await later.count()) await later.click();
    await page.getByRole('button', { name: 'Cache', exact: true }).click();
    const toggle = page.getByRole('checkbox', { name: 'Use seek-optimized proxies for video preview' });
    await toggle.click();
    await page.waitForFunction(mode => window.__TAURI_INTERNALS__.invoke('app_settings_get').then(s => s.previewProxyMode !== mode), settings.previewProxyMode);
    await page.getByRole('button', { name: 'Close media cache' }).click();
    await page.getByRole('button', { name: /\+ Track/ }).click();
    let found = false;
    for (let attempt = 0; attempt < 60; attempt++) {
      const rows = await invoke('project_list_unsaved_recoveries');
      if (rows.some(r => r.projectId === project.project_id && r.trackCount === 1)) { found = true; break; }
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    assert.ok(found, 'Autosaved recovery must be listed before restarting');
    writeFileSync('artifacts/background/cdp-recovery-prepare.json', JSON.stringify({localData: await invoke('plugin:path|resolve_directory', {directory:15}), candidates: await invoke('project_list_unsaved_recoveries')}, null, 2));
    console.log('PASS: UI settings persisted; unsaved project autosaved before process restart');
  } else {
    const data = JSON.parse(readFileSync(meta, 'utf8'));
    try {
      const settings = await invoke('app_settings_get');
      assert.notEqual(settings.previewProxyMode, data.settings.previewProxyMode);
      writeFileSync('artifacts/background/cdp-recovery-debug.json', JSON.stringify({localData:await invoke('plugin:path|resolve_directory',{directory:15}), expected: data, candidates: await invoke('project_list_unsaved_recoveries'), dom: await page.locator('body').innerText() }, null, 2));
      const card = page.locator('.dialog-review-summary').filter({ hasText: data.name });
      await card.getByRole('button', { name: 'Restore project', exact: true }).click();
      await page.waitForFunction(id => window.__TAURI_INTERNALS__.invoke('project_get_current').then(p => p.project_id === id && p.tracks.length === 1), data.projectId);
      const restored = await invoke('project_get_current');
      await invoke('project_save', { path });
      const reopened = await invoke('project_open', { path });
      assert.deepEqual(reopened.tracks, restored.tracks);
      assert.equal(reopened.project_id, data.projectId);
      await page.screenshot({ path: 'artifacts/background/cdp-recovery.png' });
      writeFileSync('artifacts/background/cdp-recovery.json', JSON.stringify({ status: 'passed', scope: 'real WebView UI setting, process restart, autosave recovery chooser/restore; command save/reopen', revision: reopened.revision }, null, 2));
      console.log('PASS: setting survives process restart; UI restores autosaved project; save/reopen matches');
    } finally { await invoke('app_settings_save', { settings: data.settings }); }
  }
} finally { await browser.close(); }
