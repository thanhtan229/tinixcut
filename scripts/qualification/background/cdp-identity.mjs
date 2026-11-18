import { chromium } from 'playwright';
import assert from 'node:assert/strict';
const expected = process.env.TINIXCUT_CDP_WINDOW_TITLE || 'TinixCut Qualification';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9444', { timeout: 3000 });
try {
  const page = browser.contexts().flatMap(context => context.pages()).find(candidate => /tauri|localhost/.test(candidate.url()));
  assert.ok(page, 'qualification page not found');
  const title = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('plugin:window|title', { label: 'main' }));
  assert.equal(title, expected, 'qualification title mismatch');
} finally { await browser.close(); }
