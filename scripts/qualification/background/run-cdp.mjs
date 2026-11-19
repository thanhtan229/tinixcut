import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync, copyFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { canStartCdpCase, classifyCdpStep, summarizeCdpRun } from './cdp-report-policy.mjs';

const cases = [
  ['caption-source', '', 'cdp-smoke.json'],
  ['recovery', 'Recovery', 'cdp-recovery.json'],
  ['saved-recovery', 'SavedRecovery', 'cdp-saved-recovery.json'],
  ['pointer', 'Pointer', 'cdp-pointer.json'],
  ['clip-boundary-scope', 'ClipBoundaryScope', 'cdp-clip-boundary-scope.json'],
  ['marker-search', 'MarkerSearch', 'cdp-marker-search.json'],
  ['controls', 'Controls', 'cdp-controls.json'],
  ['audio', 'Audio', 'cdp-audio.json'],
  ['workflow', 'Workflow', 'cdp-workflow.json'],
  ['history-branch','HistoryBranch','cdp-history-branch.json'],
  ['captions', 'Captions', 'cdp-captions.json'],
  ['media', 'Media', 'cdp-media.json'],
  ['gain-preview', 'GainPreview', 'cdp-gain-preview.json'],
];
const selected = process.argv.slice(2);
if (process.platform !== 'win32') throw new Error('Isolated CDP qualification requires Windows');
for (const name of selected) if (!cases.some(([id]) => id === name)) throw new Error(`Unknown case: ${name}`);
const requestedCases = cases.filter(([id]) => !selected.length || selected.includes(id));
const binary = resolve('src-tauri/target-qualification/debug/tinixcut.exe');
if (!existsSync(binary)) throw new Error('Build the isolated qualification binary first; see DEVELOPMENT_WORKFLOW.md');
const directory = resolve('artifacts/background/cdp-runs', new Date().toISOString().replaceAll(':', '-') + '-' + randomUUID());
mkdirSync(directory, { recursive: true });
const git = args => spawnSync('git', args, { encoding: 'utf8', windowsHide: true }).stdout?.trim();
const report = {
  startedAt: new Date().toISOString(), revision: git(['rev-parse', 'HEAD']),
  dirty: Boolean(git(['status', '--porcelain', '--untracked-files=no'])),
  binary, binarySha256: createHash('sha256').update(readFileSync(binary)).digest('hex'),
  binaryModifiedAt: statSync(binary).mtime.toISOString(),
  previewAudioOutputSha256: createHash('sha256').update(readFileSync('src/components/previewAudioOutput.ts')).digest('hex'),
  audioMeterGraphSha256: createHash('sha256').update(readFileSync('src/components/audioMeterGraph.ts')).digest('hex'),
  harnessSha256: Object.fromEntries(['run-cdp.mjs', 'cdp-desktop-smoke.ps1', 'cdp-diagnostics.ps1', 'cdp-report-policy.mjs', 'cdp-close.mjs', 'cdp-identity.mjs', 'cdp-fallback-controls.ps1', 'cdp-isolation-storage.mjs', 'tauri-isolated.conf.json', 'caption-merge.tcproj', 'cdp-audio.mjs', 'cdp-gain-preview.mjs', 'cdp-captions.mjs', 'cdp-media.mjs', 'cdp-history-branch.mjs'].map(name => [name, createHash('sha256').update(readFileSync(resolve('scripts/qualification/background', name))).digest('hex')])),
  scope: 'Real qualification WebView on isolated Windows desktops; no OS input; native dialogs/installed/audio-output qualification excluded',
  directory, requestedCases: requestedCases.map(([id]) => id), status: 'running', preflights: [], steps: [],
};
const save = () => {
  Object.assign(report, summarizeCdpRun(report.steps, requestedCases.length, Boolean(report.finishedAt)));
  writeFileSync(resolve(directory, 'report.json'), JSON.stringify(report, null, 2));
  writeFileSync('artifacts/background/cdp-latest-report.json', JSON.stringify(report, null, 2));
};
const readJson = path => JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''));
const diagnose = (receipt, label) => {
  const output = resolve(directory, `${label}.json`);
  const args = ['-NoProfile', '-NonInteractive', '-File', 'scripts/qualification/background/cdp-diagnostics.ps1', '-Label', label, '-OutputPath', output];
  if (receipt && existsSync(receipt)) args.push('-SessionPath', receipt);
  const result = spawnSync('powershell.exe', args, { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], timeout: 30000 });
  if (result.error || result.status !== 0) throw new Error(`Read-only diagnostics failed: ${result.error ?? result.stderr}`);
  return readJson(output);
};
save();
try {
  for (const [name, flag, artifact] of requestedCases) {
    const snapshot = diagnose(null, `${name}-preflight`);
    const gate = canStartCdpCase(snapshot);
    report.preflights.push({ name, snapshot, gate }); save();
    if (!gate.allowed) throw new Error(`Preflight stopped ${name}: ${gate.reasons.join('; ')}`);
    const step = { name, status: 'running', startedAt: new Date().toISOString() };
    report.steps.push(step); save();
    console.log(`CDP: ${name}`);
    const receipt = resolve(directory, `${name}-session.json`);
    const args = ['-NoProfile', '-NonInteractive', '-File', 'scripts/qualification/background/cdp-desktop-smoke.ps1', '-TimeoutSeconds', '300'];
    if (flag) args.push(`-${flag}`);
    try {
      step.parentExit = await new Promise((resolveStep, rejectStep) => {
        const child = spawn('powershell.exe', args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, TINIXCUT_CDP_SESSION_REPORT: receipt } });
        let log = '';
        for (const stream of [child.stdout, child.stderr]) stream.on('data', data => { log += data; process.stdout.write(data); });
        // The parent has its own per-stage timeout, so its finally can clean the app/profile.
        child.once('error', rejectStep);
        child.once('close', code => {
          writeFileSync(resolve(directory, `${name}.log`), log);
          resolveStep(code);
        });
      });
      const session = readJson(receipt);
      step.session = session;
      const evidence = resolve(directory, artifact);
      const fresh = statSync(resolve('artifacts/background', artifact)).mtimeMs >= Date.parse(step.startedAt);
      if (!fresh) throw new Error(`${name}: stale evidence`);
      copyFileSync(resolve('artifacts/background', artifact), evidence);
      step.evidence = evidence;
      step.result = readJson(evidence);
      step.afterParent = diagnose(receipt, `${name}-after-parent`);
      step.provenanceVerified = fresh && session.binarySha256?.toLowerCase() === report.binarySha256 && Date.parse(session.startedAt) >= Date.parse(step.startedAt);
      Object.assign(step, classifyCdpStep(step));
      console.log(`CDP ${name}: function=${step.functionalStatus}, cleanup=${step.cleanupStatus}, continue=${step.continuation.allowed}`);
    } catch (error) {
      step.status = 'failed'; step.error = String(error); report.status = 'failed';
      step.functionalStatus = 'failed'; step.cleanupStatus = 'failed';
      step.continuation = { allowed: false, reasons: [step.error] };
      if (existsSync(receipt)) step.session = readJson(receipt);
      // Keep fresh functional evidence even when the parent fails in cleanup.
      const caseFile = resolve('artifacts/background', artifact);
      if (existsSync(caseFile) && statSync(caseFile).mtimeMs >= Date.parse(step.startedAt)) {
        step.evidence = resolve(directory, artifact);
        copyFileSync(caseFile, step.evidence);
        step.result = JSON.parse(readFileSync(caseFile, 'utf8'));
      }
      if (!step.afterParent) {
        try { step.afterParent = diagnose(receipt, `${name}-after-parent`); }
        catch (diagnosticError) { step.diagnosticError = String(diagnosticError); }
      }
      // A failed safety inspection must not erase already verified UI results.
      Object.assign(step, classifyCdpStep(step));
      step.status = 'failed';
      step.continuation = { allowed: false, reasons: [step.error, ...step.continuation.reasons] };
    } finally { step.finishedAt = new Date().toISOString(); save(); }
    if (step.status === 'failed') { report.status = 'failed'; process.exitCode = 1; }
    if (!step.continuation?.allowed) { report.stopReason = step.continuation?.reasons; break; }
  }
  if (report.status !== 'failed' && report.steps.length === requestedCases.length) report.status = 'passed';
} catch (error) {
  report.status = 'failed'; report.error = String(error); process.exitCode = 1;
} finally { report.finishedAt = new Date().toISOString(); save(); }
console.log(`${report.status.toUpperCase()}: CDP report ${directory}`);
