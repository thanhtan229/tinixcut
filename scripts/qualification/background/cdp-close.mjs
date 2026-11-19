import { chromium } from 'playwright';
import assert from 'node:assert/strict';
const timer = setTimeout(() => process.exit(124), 10000);
const browser = await chromium.connectOverCDP('http://127.0.0.1:9445', { timeout: 3000 });
try {
  const page = browser.contexts().flatMap(c => c.pages()).find(p => /tauri|localhost/.test(p.url()));
  assert.ok(page);
  const title = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('plugin:window|title', { label: 'main' }));
  assert.equal(title, 'TinixCut Qualification');
  const browserSession = await browser.newBrowserCDPSession();
  const closed = page.waitForEvent('close', { timeout: 5000 });
  const request = page.evaluate(() => window.__TAURI_INTERNALS__.invoke('plugin:window|close', { label: 'main' }));
  const result = await Promise.allSettled([closed, request]);
  if (result[0].status !== 'fulfilled') throw result[0].reason;
  if (browser.isConnected()) {
    const disconnected = new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Browser did not disconnect after close')), 3000);
      browser.once('disconnected', () => { clearTimeout(timeout); resolve(); });
    });
    const browserClose = browserSession.send('Browser.close');
    const shutdown = await Promise.allSettled([disconnected, browserClose]);
    if (shutdown[0].status !== 'fulfilled') throw shutdown[0].reason;
  }
  console.log('PASS: qualification window closed through Tauri lifecycle API');
} finally { clearTimeout(timer); await browser.close(); }
