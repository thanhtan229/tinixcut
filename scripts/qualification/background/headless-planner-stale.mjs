import assert from 'node:assert/strict';
import { build } from 'vite';
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFileSync, existsSync, mkdirSync } from 'node:fs';
import { resolve, join, sep } from 'node:path';

const site = resolve('artifacts/background/planner-stale/site');
mkdirSync(site, { recursive: true });
await build({ configFile: false, root: process.cwd(), logLevel: 'error', build: { outDir: site, rollupOptions: { input: resolve('scripts/qualification/background/headless-planner-stale.html') } } });
const server = createServer((req, res) => {
  const path = resolve(site, '.' + new URL(req.url, 'http://localhost').pathname);
  if (!path.startsWith(site + sep) || !existsSync(path)) return res.writeHead(404).end();
  res.setHeader('Content-Type', path.endsWith('.html') ? 'text/html' : 'text/javascript');
  res.end(readFileSync(path));
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const browser = await chromium.launch({ headless: true, args: ['--mute-audio'] });
try {
  const page = await browser.newPage();
  const receipt = { provider: 'test', plan: { projectId: 'p', baseRevision: 1, operations: [], assumptions: [], warnings: [], referencedMediaIds: [] }, validation: { valid: true, issues: [], diff: null } };
  const suggestion = { provider: 'test', model: 'test', orderedSceneIds: ['s1'], rationale: 'test', selectedDurationMicros: 3000000, targetDurationSeconds: null, openingTitleSuggestion: null, constraints: {} };
  let pending;
  await page.exposeFunction('plannerInvoke', (command, args) => new Promise((resolve, reject) => { pending = { command, args, resolve, reject }; }));
  const cases = [
    { mode: 'trim', submit: 'Preview trim', busy: 'Validating…', review: 'Trim plan review' },
    { mode: 'reorder', submit: 'Preview order', busy: 'Validating…', review: 'Reorder plan review' },
    { mode: 'scene', submit: 'Tạo preview', busy: 'Đang xác thực…', review: 'Xem lại draft từ cảnh' },
    { mode: 'scene', submit: 'Đề xuất chọn cảnh', busy: 'Đang xử lý…', review: 'Đề xuất chọn cảnh bằng AI' },
    { mode: 'mix', submit: 'Build review plan', busy: 'Analyzing…', review: 'Change settings' },
  ];
  for (const test of cases) for (const change of ['Change revision', 'Offline a']) {
    await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-planner-stale.html?mode=${test.mode}`);
    if (test.mode === 'reorder') await page.getByRole('button', {name:'Move b up'}).click();
    if (test.mode === 'mix') {
      await page.getByText('a · Video · 0.00–3.00 s',{exact:true}).waitFor();
      await page.getByText('b · Video · 3.00–6.00 s',{exact:true}).waitFor();
      await page.getByText('music · Music · 0.00–3.00 s',{exact:true}).waitFor();
      await page.getByRole('checkbox', {name:'Normalize music from source audio levels'}).uncheck();
    }
    pending = undefined;
    await page.getByRole('button', {name:test.submit, exact:true}).click();
    for (let i = 0; !pending && i < 500; i++) await new Promise(resolve => setTimeout(resolve, 10));
    assert.ok(pending, `${test.mode}: no request`);
    await page.getByRole('button', {name:change}).click();
    const response = test.submit === 'Đề xuất chọn cảnh' ? suggestion : test.mode === 'mix' ? {projectId:'p',baseRevision:1,operationsEvaluated:pending.args.envelope.operations.length,changes:[],warnings:[]} : receipt;
    pending.resolve(response);
    await page.getByRole('button', {name:test.busy, exact:true}).waitFor({state:'detached'});
    const review = test.mode === 'mix' ? page.getByRole('button',{name:test.review}) : page.getByRole('region',{name:test.review});
    assert.equal(await review.count(), 0, `${test.mode} ${test.submit}: stale response after ${change}`);
    assert.ok(await page.getByRole('alert').count(), `${test.mode}: missing stale notice`);
    assert.equal(await page.evaluate(() => window.applied), 0);
  }
  for (const test of cases) {
    await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-planner-stale.html?mode=${test.mode}`);
    if (test.mode === 'reorder') await page.getByRole('button', {name:'Move b up'}).click();
    if (test.mode === 'mix') await page.getByRole('checkbox', {name:'Normalize music from source audio levels'}).uncheck();
    pending = undefined;
    await page.getByRole('button', {name:test.submit, exact:true}).click();
    for (let i = 0; !pending && i < 500; i++) await new Promise(resolve => setTimeout(resolve, 10));
    assert.ok(pending, `${test.mode}: no request`);
    await page.getByRole('button', {name:'Refresh media'}).click();
    const response = test.submit === 'Đề xuất chọn cảnh' ? suggestion : test.mode === 'mix' ? {projectId:'p',baseRevision:1,operationsEvaluated:pending.args.envelope.operations.length,changes:[],warnings:[]} : receipt;
    pending.resolve(response);
    const review = test.mode === 'mix' ? page.getByRole('button',{name:test.review}) : page.getByRole('region',{name:test.review});
    await review.waitFor();
    await page.getByRole('button', {name:'Refresh media'}).click();
    assert.equal(await review.count(), 1, `${test.mode}: unchanged media refresh keeps review`);
    await page.getByRole('button', {name:'Offline a'}).click();
    assert.equal(await review.count(), 0, `${test.mode}: existing review invalidated`);
  }
  await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-planner-stale.html?mode=mix`);
  pending = undefined;
  await page.getByRole('button', {name:'Build review plan'}).click();
  for (let i = 0; !pending && i < 500; i++) await new Promise(resolve => setTimeout(resolve, 10));
  assert.equal(pending?.command, 'media_analyze_audio_levels');
  await page.getByRole('button', {name:'Offline a'}).click();
  pending.resolve({meanDbfs:-20,peakDbfs:-10});
  await page.getByRole('button', {name:'Analyzing…'}).waitFor({state:'detached'});
  assert.equal(pending.command, 'media_analyze_audio_levels', 'stale analysis must not proceed to dry-run');
  assert.ok(await page.getByRole('alert').count());
  await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-planner-stale.html?mode=trim`);
  pending = undefined;
  await page.getByRole('button', {name:'Preview trim'}).click();
  for (let i = 0; !pending && i < 500; i++) await new Promise(resolve => setTimeout(resolve, 10));
  await page.getByRole('button', {name:'Offline a'}).click();
  await page.getByRole('button', {name:'Online all'}).click();
  pending.reject(new Error('old failure'));
  await page.getByRole('button', {name:'Validating…'}).waitFor({state:'detached'});
  assert.match(await page.getByRole('alert').textContent(), /changed.*preview/i);
  assert.equal(await page.getByRole('region', {name:'Trim plan review'}).count(), 0);
  console.log('PASS: trim, reorder, scene preview/suggestion and mix reject delayed stale responses');
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
