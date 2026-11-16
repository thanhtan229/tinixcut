import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createReadStream, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
import { chromium } from "playwright";
import { build } from "vite";

const root = process.cwd();
const binary = process.env.TINIXCUT_TEST_BINARY ?? resolve(root, "src-tauri/target/debug/tinixcut.exe");
const fixture = JSON.parse(readFileSync(resolve(root, "scripts/qualification/background/caption-merge.tcproj"), "utf8"));
const dir = mkdtempSync(join(tmpdir(), "tinixcut-headless-app-"));
let server;
let browser;
let projectPath;
let failCommand = false;
let submitted = [];
let importedPaths = [];
let exportJob=0, exportStates={}, delayedCancel, staleOnce=true;

function project() { return JSON.parse(readFileSync(projectPath, "utf8")); }
function reset(name) {
  projectPath = join(dir, `${name}.tcproj`);
  writeFileSync(projectPath, JSON.stringify(fixture));
  submitted = [];
}

try {
  const site = join(dir, "site");
  await build({ configFile: false, root, logLevel: "error", build: { outDir: site, emptyOutDir: true,
    rollupOptions: { input: resolve(root, "scripts/qualification/background/headless-app.html") } } });
  server = createServer((req, res) => {
    const path = resolve(site, `.${decodeURIComponent(new URL(req.url, "http://127.0.0.1").pathname)}`);
    if (!path.startsWith(site + sep) || !existsSync(path)) { res.writeHead(404).end(); return; }
    res.setHeader("Content-Type", path.endsWith(".html") ? "text/html" : path.endsWith(".js") ? "text/javascript" : path.endsWith(".css") ? "text/css" : "application/octet-stream");
    createReadStream(path).pipe(res);
  });
  await new Promise(resolveListen => server.listen(0, "127.0.0.1", resolveListen));
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 } });
  await context.exposeFunction("qualificationInvoke", async (command, args) => {
    switch (command) {
      case "plugin:dialog|save": return `C:\\test\\export-${exportJob+1}.mp4`;
      case "project_export_preflight": return {sourceCount:0,width:1920,height:1080,durationSecs:2,availableDiskBytes:10000000000,outputExists:false};
      case "project_export_start": {const id=`job-${++exportJob}`;exportStates[id]='running';return {jobId:id,outputPath:args.outputPath,status:'running'};}
      case "project_export_status": if(args.jobId==='job-2' && staleOnce){staleOnce=false;return {jobId:'job-1',status:'completed',progress:1,error:null};} return {jobId:args.jobId,status:exportStates[args.jobId],progress:0.5,error:exportStates[args.jobId]==='failed'?'disk full':null};
      case "project_export_cancel": return new Promise((_,reject)=>{delayedCancel=()=>reject(new Error('late cancellation error'));});
      case "app_health_check": return { status: "healthy", version: "test", platform: "test", schema_version: 1 };
      case "project_get_current_session": return { project: project(), canonicalPath: projectPath, persistedRevision: project().revision };
      case "project_get_current": return project();
      case "app_settings_get": return { schemaVersion: 1, whisperCliPath: null, whisperModelPath: null, transcriptionLanguage: "auto", recentProjects: [], legacyBrowserSettingsMigrated: true, previewProxyMode: "off", shortcutOverrides: {} };
      case "project_list_unsaved_recoveries":
      case "media_list_offline_assets": return [];
      case "project_execute_command": {
        const envelope = args.envelope;
        submitted.push(envelope);
        if (failCommand) throw new Error("simulated command rejection");
        const path = join(dir, "envelope.json");
        writeFileSync(path, JSON.stringify(envelope));
        return JSON.parse(execFileSync(binary, ["apply", projectPath, path], { encoding: "utf8", timeout: 30000, windowsHide: true }));
      }
      case "project_save_recovery_snapshot": return { projectRevision: project().revision, recoveryPath: `${projectPath}.recovery` };
      case "plugin:dialog|open": return ["C:\\test\\good.mp4", "C:\\test\\corrupt.mp4"];
      case "media_import": {
        importedPaths.push(args.filePath);
        if (args.filePath.endsWith("corrupt.mp4")) throw new Error("simulated invalid media");
        return {};
      }
      default: throw new Error(`Unexpected IPC command: ${command}`);
    }
  });
  const page = await context.newPage();
  const url = `http://127.0.0.1:${address.port}/scripts/qualification/background/headless-app.html`;
  const selectCaptions = async () => {
    await page.getByRole("button", { name: /Caption editor/ }).click();
    await page.getByRole("checkbox", { name: "Select caption caption_left to merge" }).check();
    await page.getByRole("checkbox", { name: "Select caption caption_right to merge" }).check();
  };

  reset("success");
  await page.goto(url);
  const exportButton=page.getByRole('button',{name:'🎬 Export',exact:true});
  await exportButton.click();await page.getByRole('button',{name:'Cancel export',exact:true}).waitFor();
  await page.getByRole('button',{name:'Cancel export',exact:true}).click();exportStates['job-1']='completed';
  await page.getByText('Export completed',{exact:true}).waitFor();assert.ok(await page.getByText(/export-1\.mp4/).count());
  await exportButton.click();await page.getByRole('button',{name:'Cancel export',exact:true}).waitFor();delayedCancel();
  await page.waitForTimeout(500);assert.equal(await page.getByText(/late cancellation error/).count(),0);assert.equal(await page.getByText('Export completed',{exact:true}).count(),0);
  exportStates['job-2']='failed';await page.getByText('Export failed',{exact:true}).waitFor();await page.getByText('disk full',{exact:true}).waitFor();
  await page.getByRole('button',{name:'Dismiss export result'}).click();assert.equal(await page.getByText('Export failed',{exact:true}).count(),0);
  await exportButton.click();await page.getByRole('button',{name:'Cancel export',exact:true}).waitFor();exportStates['job-3']='cancelled';await page.getByText('Export cancelled',{exact:true}).first().waitFor();
  console.log('PASS: export completion, stale cancellation, failure, dismissal and cancellation');
  await selectCaptions();
  await page.getByRole("button", { name: "Merge adjacent" }).click();
  const merged = page.getByRole("textbox", { name: "Caption text 0.00–2.00s" });
  await merged.waitFor();
  assert.equal(await merged.inputValue(), "Xin chao\nViet Nam");
  assert.equal(submitted.length, 1);
  assert.deepEqual(submitted[0].operations.map(op => op.operation.type), ["ClipTextSet", "ClipDelete", "ClipTrim"]);
  assert.equal(submitted[0].project_id, fixture.project_id);
  assert.equal(submitted[0].base_revision, fixture.revision);
  assert.equal(project().revision, fixture.revision + 1);
  assert.equal(project().tracks[0].items.length, 1);

  const request = [{ protocolVersion: 1, requestId: "open", method: "session.open", params: { path: projectPath } },
    { protocolVersion: 1, requestId: "inspect", method: "project.inspect", params: {} },
    { protocolVersion: 1, requestId: "shutdown", method: "session.shutdown", params: {} }].map(JSON.stringify).join("\n") + "\n";
  const responses = execFileSync(binary, ["session"], { input: request, encoding: "utf8", timeout: 30000, windowsHide: true }).trim().split(/\r?\n/).map(JSON.parse);
  assert.equal(responses[0].ok, true);
  assert.equal(responses[1].result.revision, fixture.revision + 1);

  reset("failure");
  failCommand = true;
  await page.goto(url);
  await selectCaptions();
  await page.getByRole("button", { name: "Merge adjacent" }).click();
  await page.getByRole("alert").filter({ hasText: "simulated command rejection" }).waitFor();
  await page.getByText(/Command failed:.*simulated command rejection/).waitFor();
  assert.equal(submitted.length, 1);
  assert.equal(project().revision, fixture.revision);
  assert.equal(project().tracks[0].items.length, 2);
  assert.equal(await page.getByRole("checkbox", { name: /Select caption/ }).count(), 2);
  reset("import");
  await page.goto(url);
  await page.getByRole("button", { name: "+ Import", exact: true }).click();
  await page.getByText(/Imported 1 of 2 media file\(s\).*corrupt\.mp4.*simulated invalid media/).waitFor();
  assert.deepEqual(importedPaths, ["C:\\test\\good.mp4", "C:\\test\\corrupt.mp4"]);
  console.log("PASS: App mocked-IPC merge success/rejection and mixed import reporting; native CLI save/reopen");
} finally {
  if (browser) await browser.close();
  if (server?.listening) await new Promise(resolveClose => server.close(resolveClose));
  rmSync(dir, { recursive: true, force: true });
}
