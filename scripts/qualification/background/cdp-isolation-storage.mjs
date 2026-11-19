import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, existsSync, realpathSync } from 'node:fs';
import { join, resolve, relative, isAbsolute } from 'node:path';
import { chromium } from 'playwright';

const root = process.env.TINIXCUT_QUALIFICATION_DATA_ROOT;
assert.ok(root && isAbsolute(root), 'qualification data root is required');
const expectedRoot = realpathSync(root);
const cacheFile = join(expectedRoot, 'cache', 'thumbnails', 'qualification-sentinel.txt');
writeFileSync(cacheFile, 'qualification cache fixture');
const draft = JSON.parse(readFileSync('artifacts/animated-text-templates/phase-35/draft_vietnamese_v1.json', 'utf8'));
const timer = setTimeout(() => process.exit(124), 30000);
const browser = await chromium.connectOverCDP('http://127.0.0.1:9445', { timeout: 3000 });
try {
  const page = browser.contexts().flatMap(context => context.pages()).find(candidate => /tauri|localhost/.test(candidate.url()));
  assert.ok(page, 'qualification page not found');
  const invoke = (command, args = {}) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  const saved = await invoke('template_draft_save', { draft, overwrite: false });
  const catalogRoot = resolve(expectedRoot, 'catalog');
  const savedPath = realpathSync(saved.savedPath);
  const child = relative(catalogRoot, savedPath);
  assert.ok(child && !child.startsWith('..') && !isAbsolute(child), 'template was not saved in qualification catalog');
  await invoke('media_cache_clear');
  assert.equal(existsSync(cacheFile), false, 'qualification cache was not cleared');
  console.log('PASS: template saved under isolated catalog; isolated cache cleared');
} finally {
  clearTimeout(timer);
  await browser.close();
}
