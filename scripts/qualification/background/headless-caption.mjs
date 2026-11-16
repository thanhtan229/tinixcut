import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { once } from "node:events";
import { createReadStream, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
import { createInterface } from "node:readline";
import { chromium } from "playwright";
import { build } from "vite";

const root = process.cwd();
const fixture = JSON.parse(readFileSync(resolve(root, "scripts/qualification/background/caption-merge.tcproj"), "utf8"));
const binary = process.env.TINIXCUT_TEST_BINARY ?? resolve(root, "src-tauri/target/debug/tinixcut.exe");
const dir = mkdtempSync(join(tmpdir(), "tinixcut-headless-caption-"));
let server;
let browser;
let completed = 0;

async function applyMcp(projectPath, envelope) {
  const child = spawn(binary, ["mcp"], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  const lines = createInterface({ input: child.stdout });
  const responses = lines[Symbol.asyncIterator]();
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", chunk => { stderr += chunk; });
  try {
    const request = { jsonrpc: "2.0", id: 1, method: "tools/call", params: {
      _meta: { "io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {} },
      name: "tinixcut_apply", arguments: { projectPath, envelope },
    } };
    child.stdin.write(JSON.stringify(request) + "\n");
    let timer;
    const next = await Promise.race([
      responses.next(),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`MCP timeout: ${stderr}`)), 30000); }),
    ]).finally(() => clearTimeout(timer));
    assert.equal(next.done, false, `MCP exited before responding: ${stderr}`);
    const response = JSON.parse(next.value);
    assert.equal(response.id, 1);
    assert.equal(response.result?.isError, false, JSON.stringify(response));
    assert.equal(response.result.structuredContent.newRevision, fixture.revision + 1);
  } finally {
    const exited = child.exitCode !== null ? Promise.resolve() : once(child, "exit");
    child.kill();
    lines.close();
    await exited;
  }
}

try {
  const site = join(dir, "site");
  await build({ configFile: false, root, logLevel: "error", build: { outDir: site, emptyOutDir: true,
    rollupOptions: { input: resolve(root, "scripts/qualification/background/headless-caption.html") } } });
  server = createServer((req, res) => {
    const path = resolve(site, `.${decodeURIComponent(new URL(req.url, "http://127.0.0.1").pathname)}`);
    if (!path.startsWith(site + sep) || !existsSync(path)) { res.writeHead(404).end(); return; }
    const mime = path.endsWith(".html") ? "text/html" : path.endsWith(".js") ? "text/javascript" : "application/octet-stream";
    res.setHeader("Content-Type", mime);
    createReadStream(path).pipe(res);
  });
  await new Promise(resolveListen => server.listen(0, "127.0.0.1", resolveListen));
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  browser = await chromium.launch({ headless: true, args: ["--mute-audio"] });
  const context = await browser.newContext();
  await context.exposeFunction("applyQualificationMerge", async operations => {
    assert.equal(completed, 0, "The merge must be submitted only once");
    assert.deepEqual(operations.map(item => item.operation.type), ["ClipTextSet", "ClipDelete", "ClipTrim"]);
    const cliPath = join(dir, "cli.tcproj");
    const mcpPath = join(dir, "mcp.tcproj");
    const envelopePath = join(dir, "envelope.json");
    writeFileSync(cliPath, JSON.stringify(fixture));
    writeFileSync(mcpPath, JSON.stringify(fixture));
    const envelope = { schema_version: fixture.schema_version, project_id: fixture.project_id, base_revision: fixture.revision,
      client_id: "headless-caption", client_epoch: 1, sequence_number: 1, dry_run: false,
      transaction_id: "tx_headless_caption_merge", operations };
    writeFileSync(envelopePath, JSON.stringify(envelope));
    execFileSync(binary, ["apply", cliPath, envelopePath], { encoding: "utf8", timeout: 30000, windowsHide: true });
    await applyMcp(mcpPath, envelope);
    const cli = JSON.parse(readFileSync(cliPath, "utf8"));
    const mcp = JSON.parse(readFileSync(mcpPath, "utf8"));
    assert.equal(cli.revision, fixture.revision + 1);
    assert.equal(mcp.revision, cli.revision);
    assert.deepEqual(mcp.tracks, cli.tracks);
    assert.equal(cli.tracks[0].items.length, 1);
    assert.equal(cli.tracks[0].items[0].text_content, "Xin chao\nViet Nam");
    assert.equal(cli.tracks[0].items[0].timeline_range.duration.value, 60);
    completed++;
    return cli;
  });
  let scopedDrafts=null;
  await context.exposeFunction('captureCaptionDrafts',(text,timing)=>{scopedDrafts={text,timing};});
  const page = await context.newPage();
  const url = `http://127.0.0.1:${address.port}/scripts/qualification/background/headless-caption.html`;
  await page.goto(url+'?scenario=long');
  const timingFilter = page.getByLabel('Filter caption timing');
  assert.equal(await page.locator('article').count(), 202, 'long fixture exposes 202 rows before filtering');
  await timingFilter.selectOption('notices');
  assert.equal(await page.locator('article').count(), 2);
  assert.equal(await page.getByText('Gap before: 5 frames').count(), 1);
  assert.equal(await page.getByText('Overlap with earlier captions: 5 frames').count(), 1);
  await timingFilter.selectOption('gap');
  assert.equal(await page.locator('article').count(), 1);
  assert.equal(await page.getByLabel('Start frame long-120').count(), 1);
  await timingFilter.selectOption('overlap');
  assert.equal(await page.getByLabel('Start frame long-170').count(), 1);
  await page.getByLabel('Search captions').fill('120');
  assert.match(await page.getByText('No captions match the current filters.').textContent(), /current filters/);
  await page.getByLabel('Search captions').clear();
  await page.getByLabel('Duration frames long-170').fill('30');
  const overlapStart = page.getByLabel('Start frame long-170');
  await overlapStart.fill('5105');
  assert.equal(await overlapStart.count(), 1, 'focused row remains while its notice disappears');
  await timingFilter.focus();
  assert.equal(await overlapStart.count(), 0, 'resolved row leaves filtered view on blur');
  await timingFilter.selectOption('notices');
  assert.equal(await page.locator('article').count(), 1);
  await page.getByLabel('Start frame long-120').fill('');
  assert.match(await page.getByRole('status').textContent(), /Select All timing to find invalid entries/);
  await timingFilter.focus();
  assert.equal(await page.locator('article').count(), 0);
  await timingFilter.selectOption('all');
  await page.getByLabel('Start frame long-120').fill('3605');
  await timingFilter.selectOption('notices');
  assert.equal(await page.locator('article').count(), 1);
  await page.getByLabel('Filter caption track').selectOption('other-track');
  assert.equal(await page.locator('article').count(), 0, 'timing filter respects track scope');
  await timingFilter.selectOption('all');
  assert.equal(await page.locator('article').count(), 2);
  await page.goto(url+'?scenario=tracks');
  const trackFilter=page.getByLabel('Filter caption track');
  await trackFilter.selectOption('other-track');
  const start = page.getByLabel('Start frame other-caption_right');
  await start.fill('35');
  assert.equal(await page.getByText('Gap before: 5 frames').count(), 1);
  assert.equal(await page.getByRole('button',{name:'Apply edits'}).isEnabled(), true, 'a gap is advisory');
  await page.getByLabel('Search captions').fill('Viet');
  assert.equal(await page.getByText('Gap before: 5 frames').count(), 1, 'compare to caption hidden by search');
  await page.getByLabel('Search captions').clear();
  await start.fill('25');
  assert.equal(await page.getByText('Overlap with earlier captions: 5 frames').count(), 1);
  await start.fill('');
  assert.match(await page.getByRole('status').textContent(), /Timing notices unavailable for Other language/);
  assert.equal(await page.getByText(/Overlap with earlier captions/).count(), 0);
  assert.equal(await page.getByRole('button',{name:'Apply edits'}).isDisabled(), true);
  await trackFilter.selectOption(fixture.tracks[0].track_id);
  assert.equal(await page.getByText(/Timing notices unavailable/).count(), 0, 'invalid timing stays scoped to its track');
  await page.goto(url+'?scenario=tracks');
  await trackFilter.selectOption('other-track');
  assert.equal(await page.getByRole('textbox',{name:/Caption text/}).count(),2);
  await page.getByLabel('Replace text',{exact:true}).fill('Xin');await page.getByLabel('Replace with',{exact:true}).fill('Hello');await page.getByRole('button',{name:'Replace in track',exact:true}).click();
  assert.equal(await page.getByRole('textbox',{name:'Caption text 0.00–1.00s'}).inputValue(),'Hello chao');
  await trackFilter.selectOption(fixture.tracks[0].track_id);
  assert.equal(await page.getByRole('textbox',{name:'Caption text 0.00–1.00s'}).inputValue(),'Xin chao');
  await trackFilter.selectOption('other-track');
  assert.equal(await page.getByRole('textbox',{name:'Caption text 0.00–1.00s'}).inputValue(),'Hello chao');
  await page.getByLabel('Search captions').fill('chao');
  assert.equal(await page.getByRole('textbox',{name:/Caption text/}).count(),1);
  await page.getByRole('button',{name:'Apply edits',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('button')!==null);
  for(let i=0;i<50&&!scopedDrafts;i++)await new Promise(r=>setTimeout(r,20));
  assert.deepEqual(scopedDrafts,{text:[{item_id:'other-caption_left',text:'Hello chao'}],timing:[]});
  writeFileSync('artifacts/background/caption-scoped-drafts.json',JSON.stringify(scopedDrafts,null,2));
  await page.goto(url);
  const merge = page.getByRole("button", { name: "Merge adjacent" });
  const left = page.getByRole("checkbox", { name: "Select caption caption_left to merge" });
  const right = page.getByRole("checkbox", { name: "Select caption caption_right to merge" });
  await left.waitFor();
  assert.equal(await merge.isDisabled(), true);
  await left.check();
  assert.equal(await merge.isDisabled(), true);
  await right.check();
  assert.equal(await merge.isEnabled(), true);
  const caption = page.getByRole("textbox", { name: "Caption text 0.00–1.00s" });
  await caption.fill("Edited draft");
  assert.equal(await merge.isDisabled(), true, "Dirty text must block merge");
  await page.getByRole("button", { name: "Discard edits" }).click();
  assert.equal(await merge.isEnabled(), true);
  await page.getByRole("spinbutton", { name: "Duration frames caption_left" }).fill("29");
  assert.equal(await merge.isDisabled(), true, "Dirty timing must block merge");
  await page.getByRole("button", { name: "Discard edits" }).click();
  assert.equal(await merge.isEnabled(), true);
  await merge.click();
  await page.getByRole("textbox", { name: "Caption text 0.00–2.00s" }).waitFor();
  assert.equal(await page.getByRole("textbox", { name: "Caption text 0.00–2.00s" }).inputValue(), "Xin chao\nViet Nam");
  assert.equal(await page.getByRole("checkbox").count(), 1);
  assert.equal(completed, 1);
  await page.goto(`${url}?scenario=gap`);
  await left.check();
  await right.check();
  assert.equal(await merge.isDisabled(), true, "Captions with a gap must not be merged");
  await page.goto(url);
  await right.check();
  await left.check();
  assert.equal(await merge.isDisabled(), true, "Captions selected in reverse order must not be merged");
  await page.goto(`${url}?scenario=locked`);
  await left.waitFor();
  assert.equal(await left.isDisabled(), true, "Locked captions must not be selectable");
  assert.equal(await merge.isDisabled(), true);
  assert.equal(completed, 1);
  console.log("PASS: headless selection, dirty-draft, gap and locked-track guards; merge and CLI/MCP saved-result parity");
} finally {
  if (browser) await browser.close();
  if (server?.listening) await new Promise(resolveClose => server.close(resolveClose));
  rmSync(dir, { recursive: true, force: true });
}
