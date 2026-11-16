import assert from 'node:assert/strict';
import { build } from 'vite';
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFileSync, existsSync, mkdirSync } from 'node:fs';
import { resolve, join, sep } from 'node:path';

const site = resolve('artifacts/background/draft-stale/site');
mkdirSync(site, { recursive: true });
await build({ configFile: false, root: process.cwd(), logLevel: 'error', build: { outDir: site, rollupOptions: { input: resolve('scripts/qualification/background/headless-draft-order.html') } } });
const server = createServer((req, res) => {
  const path = resolve(site, '.' + new URL(req.url, 'http://localhost').pathname);
  if (!path.startsWith(site + sep) || !existsSync(path)) return res.writeHead(404).end();
  res.setHeader('Content-Type', path.endsWith('.html') ? 'text/html' : 'text/javascript');
  res.end(readFileSync(path));
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const browser = await chromium.launch({ headless: true, args: ['--mute-audio'] });
try {
  const receipt = { provider: 'test', plan: { projectId: 'test', baseRevision: 0, operations: [], assumptions: [], warnings: [], referencedMediaIds: [] }, validation: { valid: true, issues: [], diff: null } };
  const page = await browser.newPage();
  let release;
  await page.exposeFunction('draftInvoke', () => new Promise((resolve, reject) => { release = (fail = false) => fail ? reject(new Error('old planner failure')) : resolve(receipt); }));
  const start = async () => {
    release = undefined;
    await page.getByRole('button', { name: 'Generate preview' }).click();
    await page.waitForFunction(() => document.querySelector('button[type=submit]')?.textContent === 'Validating…');
    for (let i = 0; !release && i < 500; i++) await new Promise(resolve => setTimeout(resolve, 10));
    assert.ok(release, 'planner request was not received');
  };
  const finish = async () => { release(); await page.getByText('Validating…').waitFor({ state: 'detached' }); };
  const url = `http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-draft-order.html`;
  for (const change of ['Change revision', 'Offline b', 'Remove c']) {
    await page.goto(url);
    await start();
    await page.getByRole('button', { name: change }).click();
    await finish();
    assert.equal(await page.getByRole('region', { name: 'Draft review' }).count(), 0, change);
    assert.match(await page.getByRole('alert').textContent(), /changed.*generate/i);
  }
  await page.goto(url);
  await start();
  await page.getByRole('button', { name: 'Offline b' }).click();
  await page.getByRole('button', { name: 'Online all' }).click();
  await finish();
  assert.equal(await page.getByRole('region', { name: 'Draft review' }).count(), 0, 'offline then online');
  await page.goto(url);
  await start();
  await page.getByRole('button', { name: 'Change revision' }).click();
  release(true);
  await page.getByText('Validating…').waitFor({ state: 'detached' });
  assert.match(await page.getByRole('alert').textContent(), /changed.*generate/i);
  await start();
  release();
  await page.getByRole('region', { name: 'Draft review' }).waitFor();
  await page.goto(url);
  await start();
  await page.getByRole('button', { name: 'Refresh media' }).click();
  release();
  await page.getByRole('region', { name: 'Draft review' }).waitFor();
  await page.getByRole('button', { name: 'Change revision' }).click();
  assert.equal(await page.getByRole('region', { name: 'Draft review' }).count(), 0, 'review invalidated');
  console.log('PASS: stale draft responses rejected; unchanged media refresh accepted');
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
