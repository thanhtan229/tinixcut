import assert from 'node:assert/strict';
import { createReadStream, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve, sep } from 'node:path';
import { chromium } from 'playwright';
import { build } from 'vite';

const root = process.cwd();
const dir = mkdtempSync(join(tmpdir(), 'tinixcut-caption-pending-'));
let server;
let browser;
try {
  const site = join(dir, 'site');
  await build({ configFile: false, root, logLevel: 'error', build: { outDir: site, emptyOutDir: true,
    rollupOptions: { input: resolve(root, 'scripts/qualification/background/headless-caption.html') } } });
  server = createServer((req, res) => {
    const path = resolve(site, `.${decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname)}`);
    if (!path.startsWith(site + sep) || !existsSync(path)) { res.writeHead(404).end(); return; }
    res.setHeader('Content-Type', path.endsWith('.html') ? 'text/html' : path.endsWith('.js') ? 'text/javascript' : 'application/octet-stream');
    createReadStream(path).pipe(res);
  });
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  browser = await chromium.launch({ headless: true, args: ['--mute-audio'] });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}/scripts/qualification/background/headless-caption.html?scenario=pending`);
  const text = page.getByRole('textbox', { name: 'Caption text 0.00–1.00s' });
  await text.fill('First edit');
  await page.getByRole('button', { name: 'Apply edits', exact: true }).click();
  await page.waitForFunction(() => typeof window.completeCaptionApply === 'function');
  assert.equal(await text.getAttribute('readonly'), '');
  assert.equal(await text.isDisabled(), false);
  await text.focus();
  await page.keyboard.type(' lost');
  assert.equal(await text.inputValue(), 'First edit');
  await page.evaluate(() => window.completeCaptionApply(false));
  await page.getByRole('alert').filter({ hasText: 'Apply failed' }).waitFor();
  assert.equal(await text.getAttribute('readonly'), null);
  assert.equal(await text.inputValue(), 'First edit');
  await page.getByRole('button', { name: 'Apply edits', exact: true }).click();
  assert.equal(await text.getAttribute('readonly'), '');
  await page.evaluate(() => window.completeCaptionApply(true));
  await page.waitForFunction(() => !document.querySelector('textarea')?.readOnly);
  assert.equal(await text.inputValue(), 'First edit');
  await text.fill('Second edit');
  assert.equal(await page.getByRole('button', { name: 'Apply edits', exact: true }).isDisabled(), false);
  console.log('PASS: caption drafts are read-only during apply, retained on error, and editable after success');
} finally {
  if (browser) await browser.close();
  if (server) await new Promise(done => server.close(done));
  rmSync(dir, { recursive: true, force: true });
}
