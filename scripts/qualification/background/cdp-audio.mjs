import { chromium } from 'playwright';
import { execFileSync, spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

const path = process.env.TINIXCUT_CDP_PROJECT;
if (!path) throw new Error('Missing isolated fixture path');
const ffmpeg = resolve('src-tauri/resources/bin/ffmpeg.exe');
const sources = ['tone-a', 'tone-b'].map(name => `${path}.${name}.wav`);
for (const [index, source] of sources.entries()) execFileSync(ffmpeg, [
  '-v', 'error', '-y', '-f', 'lavfi', '-i', `sine=frequency=${index ? 880 : 440}:sample_rate=48000:duration=90`, source,
], { windowsHide: true, timeout: 15000 });
const report = { startedAt: new Date().toISOString(), scope: 'Real isolated WebView: paired DOM/native clock and meter observations; no audible-output or calibrated clock acceptance', steps: [], samples: [] };
const watchdog = setTimeout(() => process.exit(124), 150000);
const browser = await chromium.connectOverCDP('http://127.0.0.1:9445', { timeout: 10000 });
try {
  const page = browser.contexts().flatMap(c => c.pages()).find(p => /tauri|localhost/.test(p.url()));
  assert.ok(page, 'Qualification WebView missing');
  const invoke = (command, args = {}) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  const delay = ms => new Promise(r => setTimeout(r, ms));
  const get = () => invoke('project_get_current');
  const poll = async predicate => {
    for (let n = 0; n < 80; n++) { const project = await get(); if (predicate(project)) return project; await delay(100); }
    throw new Error('Audio project-state timeout');
  };
  report.health = await invoke('app_health_check'); assert.equal(report.health.status, 'healthy');
  const assets = [];
  for (const source of sources) assets.push(await invoke('media_import', { filePath: source }));
  const project = await get();
  const time = value => ({ value, rate: { numerator: 30, denominator: 1 } });
  const operations = assets.flatMap((asset, index) => [
    { operation_id: `track_${index}`, operation: { type: 'TrackCreate', payload: { track_id: `audio_${index}`, name: `Tone ${index ? 'B' : 'A'}`, kind: 'Audio' } } },
    { operation_id: `clip_${index}`, operation: { type: 'ClipCreate', payload: { track_id: `audio_${index}`, item_id: `tone_${index}`, name: `Tone ${index ? 'B' : 'A'}`, kind: 'Audio', asset_id: asset.asset_id, timeline_start: time(0), duration: time(2700), source_start: time(0) } } },
  ]);
  await invoke('project_execute_command', { envelope: { schema_version: project.schema_version, project_id: project.project_id, base_revision: project.revision, transaction_id: 'audio_seed', operations } });
  await page.addInitScript(() => {
    window.__gainOutputs = new Map();
    const original = AudioContext.prototype.createMediaElementSource;
    AudioContext.prototype.createMediaElementSource = function(element) {
      const source = original.call(this, element); const connect = source.connect.bind(source);
      source.connect = function(target, ...rest) { if (target instanceof GainNode) window.__gainOutputs.set(element, target); return connect(target, ...rest); };
      return source;
    };
  });
  await invoke('project_save', { path }); await page.reload();
  const sample = async label => {
    const snapshot = await page.evaluate(() => ({
      browserMs: performance.now(), frame: Number(document.querySelector('#preview-frame-seek')?.value),
      media: [...document.querySelectorAll('audio')].map(e => ({ source: e.src.includes('tone-a') ? 'A' : e.src.includes('tone-b') ? 'B' : 'unknown', time: e.currentTime, volume: window.__gainOutputs?.get(e)?.gain.value ?? e.volume, nativeVolume: e.volume, muted: (window.__gainOutputs?.get(e)?.gain.value ?? (e.muted ? 0 : e.volume)) === 0, paused: e.paused, readyState: e.readyState })),
      meters: [...document.querySelectorAll('[role="meter"]')].map(e => ({ label: e.getAttribute('aria-label'), dbfs: Number(e.getAttribute('aria-valuenow')), text: e.getAttribute('aria-valuetext') })),
    }));
    const started = Date.now();
    const native = await invoke('preview_sample_at', { frame: snapshot.frame });
    return { label, wallMs: Date.now(), nativeReadMs: Date.now() - started, ...snapshot, nativeAudio: native.active_audio.map(a => ({ itemId: a.item_id, volume: a.effective_volume, sourceSeconds: a.source_time.value * a.source_time.rate.denominator / a.source_time.rate.numerator })), clockErrorsSeconds: snapshot.media.map(m => ({ source: m.source, error: m.time - snapshot.frame / 30 })) };
  };
  const play = () => page.getByRole('button', { name: /Play/ }).filter({ hasText: 'Play' }).first().click();
  const pause = () => page.getByRole('button', { name: /Pause/ }).first().click();
  const seek = async frame => {
    await page.getByLabel('Go to frame', { exact: true }).fill(String(frame));
    await page.getByRole('button', { name: 'Go', exact: true }).click();
    for (let n = 0; n < 60; n++) {
      const value = await sample('seek');
      if (value.frame === frame && value.media.length === 2 && value.media.every(m => Math.abs(m.time - frame / 30) < 0.2)) return value;
      await delay(100);
    }
    throw new Error('Paused seek did not synchronize both audio elements');
  };
  await play(); await delay(1000);
  report.baseline = await sample('baseline');
  assert.equal(report.baseline.media.length, 2);
  assert.ok(report.baseline.media.every(m => !m.paused && m.readyState >= 3));
  await page.getByRole('button', { name: 'Mute Tone A', exact: true }).click();
  await poll(p => p.tracks[0].muted); await delay(600); report.muted = await sample('Tone A muted');
  assert.ok(report.muted.nativeAudio.every(a => a.itemId !== 'tone_0' || a.volume === 0));
  assert.ok(report.muted.media.every(m => m.source !== 'A' || m.muted));
  await page.getByRole('button', { name: 'Unmute Tone A', exact: true }).click(); await poll(p => !p.tracks[0].muted);
  await page.getByRole('button', { name: 'Solo Tone B', exact: true }).click(); await poll(p => p.tracks[1].solo);
  await delay(600); report.solo = await sample('Tone B solo');
  assert.ok(report.solo.nativeAudio.every(a => a.itemId !== 'tone_0' || a.volume === 0));
  assert.ok(report.solo.media.every(m => m.source !== 'A' || m.muted));
  await page.getByRole('button', { name: 'Unsolo Tone B', exact: true }).click(); await poll(p => !p.tracks[1].solo);
  const slider = page.getByRole('slider', { name: 'Tone B gain', exact: true });
  for (let n = 1; n <= 6; n++) {
    await slider.press('ArrowLeft');
    await poll(p => Math.abs(p.tracks[1].volume - 10 ** (-n / 20)) < 0.001);
  }
  await delay(600); report.gain = await sample('Tone B -6 dB');
  const peak = (snapshot, label) => snapshot.meters.find(m => m.label === label)?.dbfs;
  assert.ok(peak(report.baseline, 'Playback peak level') > Math.max(peak(report.baseline, 'Tone A playback peak'), peak(report.baseline, 'Tone B playback peak')) + 3, 'Master must sum both fixture tones rather than take the largest source peak');
  assert.ok(Math.abs(peak(report.muted, 'Playback peak level') - peak(report.muted, 'Tone B playback peak')) < 0.5, 'Single-source master/track peak must agree');
  report.matrix = [];
  assert.ok(Math.abs(peak(report.gain, 'Tone B playback peak') - peak(report.baseline, 'Tone B playback peak') + 6) < 0.5, 'Relative Tone B meter response');
  report.steps.push('Mute/solo native and element suppression; relative -6 dB meter response');
  // Verify the supported native-output gain range and retain the positive-gain gap.
  for (let n = -5; n <= 6; n++) {
    await slider.press('ArrowRight');
    await poll(p => Math.abs(p.tracks[1].volume - 10 ** (n / 20)) < 0.001);
    await delay(80);
  }
  await delay(600); const positive = await sample('Tone B +6 dB'); report.matrix.push(positive);
  const nativePositive = positive.nativeAudio.find(a => a.itemId === 'tone_1').volume;
  const elementPositive = positive.media.find(m => m.source === 'B').volume;
  report.positiveGain = { requestedLinear: nativePositive, elementLinear: elementPositive, qualification: Math.abs(nativePositive - elementPositive) < 0.001 ? 'matched graph output gain' : 'FAILED graph output gain' };
  assert.ok(nativePositive > 1.9 && Math.abs(nativePositive - elementPositive) < 0.001, 'Preview output must retain +6 dB gain');
  assert.ok(Math.abs(peak(positive, 'Tone B playback peak') - peak(report.baseline, 'Tone B playback peak') - 6) < 0.5, 'Preview meter must measure +6 dB signal');
  // Isolate one source so this check measures positive gain, independently of
  // multi-source mix normalization and phase alignment.
  await page.getByRole('button', { name: 'Mute Tone A', exact: true }).click(); await poll(p => p.tracks[0].muted);
  const gainExport = path + '.positive.mp4';
  const gainJob = await invoke('project_export_start', { outputPath: gainExport, preset: 'balanced', range: { start_frame: 0, end_frame: 90 } });
  let gainStatus;
  for (let n = 0; n < 150; n++) {
    gainStatus = await invoke('project_export_status', { jobId: gainJob.jobId });
    if (['completed', 'failed'].includes(gainStatus.status)) break;
    await delay(200);
  }
  assert.equal(gainStatus.status, 'completed', JSON.stringify(gainStatus));
  const analysis = args => {
    const result = spawnSync(ffmpeg, ['-hide_banner', ...args, '-f', 'null', '-'], { windowsHide: true, encoding: 'utf8', timeout: 15000 });
    assert.equal(result.status, 0);
    const dbfs = Number(result.stderr.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
    assert.ok(Number.isFinite(dbfs)); return dbfs;
  };
  report.positiveGain.exportMaxDbfs = analysis(['-i', gainExport, '-vn', '-af', 'volumedetect']);
  report.positiveGain.referenceMaxDbfs = analysis(['-i', sources[1], '-af', `volume=${nativePositive},aformat=channel_layouts=${project.settings.channels === 1 ? 'mono' : 'stereo'},volumedetect`, '-t', '3']);
  assert.ok(Math.abs(report.positiveGain.exportMaxDbfs - report.positiveGain.referenceMaxDbfs) <= 0.5, 'Native export must retain positive gain within the locked 0.5 dB peak tolerance');
  await page.getByRole('button', { name: 'Unmute Tone A', exact: true }).click(); await poll(p => !p.tracks[0].muted);
  report.steps.push('Native +6 dB range export matches FFmpeg; production graph retains +6 dB and relative measured signal');
  // Return to -6 dB for repeatable long playback/reference.
  for (let n = 5; n >= -6; n--) {
    await slider.press('ArrowLeft');
    await poll(p => Math.abs(p.tracks[1].volume - 10 ** (n / 20)) < 0.001);
    await delay(80);
  }
  await page.getByRole('button', { name: 'Solo Tone A', exact: true }).click(); await poll(p => p.tracks[0].solo);
  await page.getByRole('button', { name: 'Solo Tone B', exact: true }).click(); await poll(p => p.tracks[1].solo);
  await delay(600); const bothSolo = await sample('Both tracks solo'); report.matrix.push(bothSolo);
  assert.equal(bothSolo.nativeAudio.length, 2); assert.ok(bothSolo.nativeAudio.every(a => a.volume > 0));
  await page.getByRole('button', { name: 'Unsolo Tone A', exact: true }).click(); await poll(p => !p.tracks[0].solo);
  await page.getByRole('button', { name: 'Unsolo Tone B', exact: true }).click(); await poll(p => !p.tracks[1].solo);
  for (const [index, name] of ['A','B'].entries()) {
    await page.getByRole('button', { name: `Mute Tone ${name}`, exact: true }).click(); await poll(p => p.tracks[index].muted);
  }
  await delay(600); const allMuted = await sample('All muted'); report.matrix.push(allMuted);
  assert.ok(allMuted.nativeAudio.every(a => a.volume === 0));
  assert.equal(peak(allMuted, 'Playback peak level'), -60);
  for (const [index, name] of ['A','B'].entries()) {
    await page.getByRole('button', { name: `Unmute Tone ${name}`, exact: true }).click(); await poll(p => !p.tracks[index].muted);
  }
  report.steps.push('Mute all, multiple solo, -6/0/+6 dB project/production graph gain');
  await pause(); await delay(200); const paused = await sample('paused'); await delay(500); const stable = await sample('paused +500ms');
  assert.equal(paused.frame, stable.frame);
  assert.ok(stable.media.every(m => m.paused && Math.abs(m.time - paused.media.find(p => p.source === m.source).time) < 0.1));
  report.pause = { paused, stable };
  report.seek = await seek(300);
  await play(); await delay(700); report.resume = await sample('resumed');
  assert.ok(report.resume.frame > 300 && report.resume.media.every(m => m.time > 10));
  await pause();
  const fadeProject = await get();
  await invoke('project_execute_command', { envelope: { schema_version: fadeProject.schema_version, project_id: fadeProject.project_id, base_revision: fadeProject.revision, transaction_id: 'audio_fade', operations: [{ operation_id: 'fade', operation: { type: 'ClipAudioFadeSet', payload: { item_id: 'tone_1', fade_in: time(60), fade_out: time(60) } } }] } });
  report.fade = [];
  for (const frame of [15, 30, 2690]) { await seek(frame); const value = await sample(`fade frame ${frame}`); report.fade.push(value); const expected = value.nativeAudio.find(a => a.itemId === 'tone_1').volume; const actual = value.media.find(m => m.source === 'B').volume; assert.ok(expected < 0.5012 && Math.abs(actual - expected) < 0.02, 'Fade must reach the native media output'); }
  report.steps.push('Command-backed fade in/out reaches paused native media element volume');
  await seek(0); await play();
  const started = Date.now();
  for (let n = 0; n < 60; n++) {
    await delay(1000); report.samples.push({ elapsedMs: Date.now() - started, ...await sample(`playback-${n + 1}`) });
    if (n % 20 === 19) console.log(`Audio paired observations: ${n + 1}/60`);
  }
  await pause();
  assert.ok(report.samples.at(-1).frame > report.samples[0].frame);
  assert.ok(report.samples.every(s => s.media.length === 2 && s.media.every(m => !m.paused)), 'Both sources must sustain playback');
  const errors = report.samples.flatMap(s => s.clockErrorsSeconds.map(e => Math.abs(e.error))).sort((a,b) => a-b);
  report.clock = { durationMs: Date.now() - started, sampleCount: report.samples.length, medianAbsSeconds: errors[Math.floor(errors.length / 2)], p95AbsSeconds: errors[Math.ceil(errors.length * 0.95) - 1], maxAbsSeconds: errors.at(-1), criterion: 'Paired observational measurements; no calibrated audible clock/drift budget asserted' };
  // Independent captured streams are not proven phase-aligned to offline PCM.
  const reference = spawnSync(ffmpeg, ['-hide_banner', '-i', sources[0], '-i', sources[1], '-filter_complex', '[1:a]volume=-6dB[b];[0:a][b]amix=inputs=2:normalize=0,volumedetect', '-t', '3', '-f', 'null', '-'], { windowsHide: true, encoding: 'utf8', timeout: 10000 });
  assert.equal(reference.status, 0);
  const master = report.samples.map(s => peak(s, 'Playback peak level')).filter(Number.isFinite).sort((a,b) => a-b);
  report.master = { uiMedianDbfs: master[Math.floor(master.length / 2)], ffmpegAlignedMixPeakDbfs: Number(reference.stderr.match(/max_volume:\s*([-\d.]+) dB/)?.[1]), qualification: 'Summed production output bus measured; hardware calibration OPEN', finding: 'Production gain nodes feed the single audible output and summed analyser; FFmpeg is an aligned offline reference, and independent media-element phases may differ.' };
  report.steps.push('Pause stability; seek both sources; resume; 60-second paired DOM/native/meter samples');
  report.status = 'passed';
  report.limitation = 'Functional checks pass; captured master is summed; positive-gain graph verified; hardware calibration, clocks and installed acceptance remain OPEN';
  console.log('PASS: G51 production output gain/fades/history observations; hardware calibration remains OPEN');
} catch (error) { report.status = 'failed'; report.error = String(error); throw error; }
finally { clearTimeout(watchdog); report.finishedAt = new Date().toISOString(); writeFileSync('artifacts/background/cdp-audio.json', JSON.stringify(report, null, 2)); await browser.close(); }
