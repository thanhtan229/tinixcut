import { chromium } from 'playwright';
import { execFileSync } from 'node:child_process';
import { writeFileSync, readFileSync } from 'node:fs';
import ts from 'typescript';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

const path = process.env.TINIXCUT_CDP_PROJECT;
assert.ok(path, 'Missing isolated fixture path');
const ffmpeg = resolve('src-tauri/resources/bin/ffmpeg.exe');
const source = path + '.gain.wav';
execFileSync(ffmpeg, ['-v', 'error', '-y', '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=20', '-ac', '2', source], { windowsHide: true, timeout: 15000 });
const transpile = code => ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const graphCode = transpile(readFileSync('src/components/audioMeterGraph.ts', 'utf8')).replaceAll('export ', '');
const outputCode = transpile(readFileSync('src/components/previewAudioOutput.ts', 'utf8')).replace(/^import .*;$/m, '').replaceAll('export ', '');
execFileSync(ffmpeg, ['-v', 'error', '-y', '-f', 'lavfi', '-i', 'sine=frequency=880:sample_rate=48000:duration=20', '-ac', '2', source.replace('.gain.wav', '.gain-b.wav')], { windowsHide: true, timeout: 15000 });
const report = { startedAt: new Date().toISOString(), scope: 'Qualification-only Web Audio output experiment; captured graph PCM versus native export, no physical-speaker claim', comparisons: [] };
const timer = setTimeout(() => process.exit(124), 120000);
const browser = await chromium.connectOverCDP('http://127.0.0.1:9445', { timeout: 10000 });
try {
  const page = browser.contexts().flatMap(c => c.pages()).find(p => /tauri|localhost/.test(p.url()));
  assert.ok(page);
  const invoke = (command, args = {}) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  report.health = await invoke('app_health_check'); assert.equal(report.health.status, 'healthy');
  const asset = await invoke('media_import', { filePath: source });
  const time = value => ({ value, rate: { numerator: 30, denominator: 1 } });
  const execute = async operations => {
    const p = await invoke('project_get_current');
    return invoke('project_execute_command', { envelope: { schema_version: p.schema_version, project_id: p.project_id, base_revision: p.revision, transaction_id: `gain_poc_${p.revision}`, operations: operations.map((operation, i) => ({ operation_id: `gain_${p.revision}_${i}`, operation })) } });
  };
  await execute([
    { type: 'TrackCreate', payload: { track_id: 'gain_track', name: 'Gain POC', kind: 'Audio' } },
    { type: 'ClipCreate', payload: { track_id: 'gain_track', item_id: 'gain_clip', name: 'Gain tone', kind: 'Audio', asset_id: asset.asset_id, timeline_start: time(0), duration: time(600), source_start: time(0) } },
  ]);
  await page.addInitScript({ content: graphCode + '\n' + outputCode + '\nwindow.__PreviewAudioOutput = PreviewAudioOutput;' });
  await invoke('project_save', { path }); await page.reload();
  await page.locator('audio').waitFor({ state: 'attached' });
  // A local gesture unlocks the context. CDP dispatch stays in the isolated WebView.
  await page.evaluate(() => {
    const button = document.createElement('button'); button.textContent = 'Start gain output experiment'; button.id = 'gain-poc-start';
    button.onclick = () => {
      const context = new AudioContext({ sampleRate: 48000 });
      window.gainPoc = { context, resume: context.resume(), errors: [] };
    };
    document.body.append(button);
  });
  await page.locator('#gain-poc-start').click();
  report.preview = await page.evaluate(async () => {
    const PreviewAudioOutput = window.__PreviewAudioOutput;
    const state = window.gainPoc; await state.resume;
    const context = state.context;
    const url = document.querySelector('audio').src;
    const element = new Audio(); element.crossOrigin = 'anonymous'; element.src = url;
    element.preload = 'auto'; element.volume = 1;
    const output = new PreviewAudioOutput(context);
    const tap = output.setGain(element, 0);
    const gain = tap.gain;
    const processor = context.createScriptProcessor(2048, 2, 2);
    const silent = context.createGain(); silent.gain.value = 0;
    output.bus.analyser.connect(processor); processor.connect(silent); silent.connect(context.destination);
    let captured = [];
    processor.onaudioprocess = event => captured.push(Array.from(event.inputBuffer.getChannelData(0)));
    const delay = ms => new Promise(r => setTimeout(r, ms));
    const measure = async (label, linear) => {
      output.setGain(element, linear); await delay(200); captured = []; await delay(500);
      const pcm = captured.flat(); let square = 0, peak = 0;
      for (const value of pcm) { square += value * value; peak = Math.max(peak, Math.abs(value)); }
      return { label, linear, contextState: context.state, count: pcm.length, rms: pcm.length ? Math.sqrt(square / pcm.length) : 0, peak, mediaTime: element.currentTime, mediaPaused: element.paused, mediaReady: element.readyState, mediaError: element.error?.message ?? null };
    };
    const results = [];
    try {
      await element.play();
      for (const db of [-6, 0, 6]) results.push(await measure(`${db} dB`, 10 ** (db / 20)));
      element.pause(); await delay(200); const pausedTime = element.currentTime;
      const paused = await measure('paused', 1);
      const pauseStable = Math.abs(element.currentTime - pausedTime) < 0.02;
      element.currentTime = 8; await element.play(); results.push(await measure('seek/resume', 1));
      results.push(await measure('muted graph', 0));
      const second = new Audio(); second.crossOrigin = 'anonymous'; second.src = url.replace('.gain.wav', '.gain-b.wav');
      output.setGain(second, 1); await second.play();
      const twoSources = await measure('two sources', 1);
      const onlySecond = await measure('solo second', 0);
      output.setGain(second, 0); const allMuted = await measure('all muted', 0);
      second.pause(); second.removeAttribute('src'); second.load(); output.retain(new Set([element]));
      state.multiple = { twoSources, onlySecond, allMuted, retainedCount: output.outputs.size };
      const reusedSource = output.setGain(element, 1).source === tap.source && output.outputs.size === 1;
      element.pause(); output.retain(new Set());
      const detachedCount = output.outputs.size;
      const replacement = new Audio(); replacement.crossOrigin = 'anonymous'; replacement.src = url;
      const replacementTap = output.setGain(replacement, 10 ** (6 / 20));
      await replacement.play();
      await delay(250); captured = []; await delay(500);
      const replacementPcm = captured.flat();
      const replacementRms = Math.sqrt(replacementPcm.reduce((sum, v) => sum + v*v, 0) / replacementPcm.length);
      replacement.pause(); output.retain(new Set()); replacement.removeAttribute('src'); replacement.load();
      state.lifecycle = { reusedSource, detachedCount, replacementRms, disposedCount: output.outputs.size };
      return { method: 'Production PreviewAudioOutput with local asset media; zero-gain PCM observer', lifecycle: state.lifecycle, multiple: state.multiple, sampleRate: context.sampleRate, multiple: state.multiple, results, paused, pauseStable, outputEdges: 1, recorderSilentGain: silent.gain.value };
    } finally {
      element.pause(); element.removeAttribute('src'); element.load(); processor.onaudioprocess = null;
      output.dispose(); processor.disconnect(); silent.disconnect(); await context.close();
      document.querySelector('#gain-poc-start').remove(); delete window.gainPoc;
    }
  });
  for (const db of [-6, 0, 6]) {
    const linear = 10 ** (db / 20);
    await execute([{ type: 'TrackVolumeSet', payload: { track_id: 'gain_track', volume: linear } }]);
    const output = path + `.gain${db}.mp4`;
    const job = await invoke('project_export_start', { outputPath: output, preset: 'balanced', range: { start_frame: 0, end_frame: 30 } });
    let status;
    for (let n = 0; n < 200; n++) {
      status = await invoke('project_export_status', { jobId: job.jobId });
      if (['completed', 'failed'].includes(status.status)) break;
      await new Promise(r => setTimeout(r, 150));
    }
    assert.equal(status.status, 'completed', JSON.stringify(status));
    const pcm = execFileSync(ffmpeg, ['-v', 'error', '-i', output, '-ss', '0.2', '-t', '0.5', '-map', '0:a:0', '-ar', '48000', '-ac', '2', '-f', 'f32le', '-'], { windowsHide: true, timeout: 15000, maxBuffer: 2000000 });
    let sum = 0, peak = 0, count = 0;
    for (let i = 0; i + 8 <= pcm.length; i += 8) { const v = pcm.readFloatLE(i); sum += v * v; peak = Math.max(peak, Math.abs(v)); count++; }
    assert.ok(count > 20000);
    const rms = Math.sqrt(sum / count);
    const reference = execFileSync(ffmpeg, ['-v', 'error', '-i', source, '-ss', '0.2', '-t', '0.5', '-af', `volume=${linear}`, '-ar', '48000', '-ac', '2', '-f', 'f32le', '-'], { windowsHide: true, timeout: 10000, maxBuffer: 2000000 });
    let refSum = 0, refCount = 0;
    for (let i = 0; i + 8 <= reference.length; i += 8) { refSum += reference.readFloatLE(i) ** 2; refCount++; }
    const referenceRms = Math.sqrt(refSum / refCount);
    const exportErrorDb = 20 * Math.log10(rms / referenceRms);
    assert.ok(Math.abs(exportErrorDb) <= 0.5, `Native ${db} dB export/reference mismatch`);
    const preview = report.preview.results.find(r => r.label === `${db} dB`);
    report.comparisons.push({ db, nativeExport: { rms, peak, samples: count, referenceRms, errorDb: exportErrorDb }, previewRms: preview.rms, previewExportErrorDb: preview.rms > 0 ? 20 * Math.log10(preview.rms / rms) : null });
  }
  const active = report.preview.results.filter(r => r.linear > 0);
  const nonSilent = active.every(r => r.count > 10000 && r.rms > 0.001 && r.contextState === 'running' && !r.mediaPaused && !r.mediaError);
  const levelMatch = report.comparisons.every(c => c.previewExportErrorDb !== null && Math.abs(c.previewExportErrorDb) <= 0.5);
  const silence = report.preview.paused.rms < 0.000001 && report.preview.results.find(r => r.label === 'muted graph').rms < 0.000001;
  assert.ok(report.preview.multiple.twoSources.rms > 0.07 && report.preview.multiple.onlySecond.rms > 0.05 && report.preview.multiple.allMuted.rms < 0.000001 && report.preview.multiple.retainedCount === 1, 'Multi-source/solo/mute must reach recorded production bus');
  assert.ok(report.preview.lifecycle.reusedSource && report.preview.lifecycle.detachedCount === 0 && report.preview.lifecycle.disposedCount === 0 && report.preview.lifecycle.replacementRms > 0.1, 'Production output lifecycle/replacement must retain signal without duplicate nodes');
  assert.ok(nonSilent && levelMatch && silence && report.preview.pauseStable, 'Production output must meet signal/level/pause criteria');
  report.qualification = 'Production output PCM/lifecycle qualification passed on this fixture; physical output and wider hardware acceptance OPEN';
  report.integrationAllowed = nonSilent && levelMatch && silence && report.preview.pauseStable;
  report.status = 'passed'; // The experiment completed; qualification/integration are separate.
  report.limitations = 'ScriptProcessor observes graph PCM, not physical output. Graph construction has one audible connection; this does not measure a physical speaker. Full editor solo/fade/multi-source behavior is covered separately by cdp-audio and cdp-controls.';
  console.log(`Experiment completed: ${report.qualification}`);
} catch (error) { report.status = 'failed'; report.error = String(error); throw error; }
finally { clearTimeout(timer); report.finishedAt = new Date().toISOString(); writeFileSync('artifacts/background/cdp-gain-preview.json', JSON.stringify(report, null, 2)); await browser.close(); }
