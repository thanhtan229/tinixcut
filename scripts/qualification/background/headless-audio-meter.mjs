import { chromium } from 'playwright';
import ts from 'typescript';
import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const source = ts.transpileModule(readFileSync('src/components/audioMeterGraph.ts', 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const results = await page.evaluate(async url => {
    const { createAudioMeterBus, connectAudioMeterTap } = await import(url);
    const cases = [
      { name: 'single', frequencies: [440], gains: [1] },
      { name: 'two tones', frequencies: [440, 880], gains: [1, 1] },
      { name: 'coherent sum +6 dB', frequencies: [440, 440], gains: [1, 1] },
      { name: 'phase cancellation', frequencies: [440, 440], gains: [1, -1] },
      { name: 'mute all', frequencies: [440, 880], gains: [0, 0] },
      { name: 'gain -6 dB', frequencies: [440, 880], gains: [1, 10 ** (-6 / 20)] },
      { name: 'silent analysis destination', frequencies: [440, 880], gains: [1, 1], silent: true },
    ];
    const results = [];
    for (const fixture of cases) {
      const context = new OfflineAudioContext(1, 4800, 48000);
      const bus = createAudioMeterBus(context);
      const expected = new Float32Array(4800);
      for (const [index, frequency] of fixture.frequencies.entries()) {
        const buffer = context.createBuffer(1, 4800, 48000);
        const samples = buffer.getChannelData(0);
        for (let n = 0; n < samples.length; n++) {
          samples[n] = 0.125 * Math.sin(2 * Math.PI * frequency * n / 48000);
          if (!fixture.silent) expected[n] += samples[n] * fixture.gains[index];
        }
        const source = context.createBufferSource(); source.buffer = buffer;
        const tap = connectAudioMeterTap(context, source, bus.analyser);
        tap.gain.gain.value = fixture.gains[index]; source.start();
      }
      // Observe the bus only in this test. Production has solely the zero-gain output.
      if (!fixture.silent) bus.analyser.connect(context.destination);
      const rendered = (await context.startRendering()).getChannelData(0);
      let maxError = 0, peak = 0;
      for (let n = 0; n < rendered.length; n++) { maxError = Math.max(maxError, Math.abs(rendered[n] - expected[n])); peak = Math.max(peak, Math.abs(rendered[n])); }
      results.push({ ...fixture, maxError, peak, dbfs: peak > 0 ? 20 * Math.log10(peak) : null });
    }
    return results;
  }, moduleUrl);
  for (const result of results) assert.ok(result.maxError < 1e-6, `${result.name}: summed waveform mismatch ${result.maxError}`);
  for (const result of results) {
    const expression = result.silent ? '0' : result.frequencies.map((frequency, index) => `0.125*(${result.gains[index]})*sin(2*PI*${frequency}*t)`).join('+');
    const reference = spawnSync(resolve('src-tauri/resources/bin/ffmpeg.exe'), ['-hide_banner', '-f', 'lavfi', '-i', `aevalsrc=${expression}:s=48000:d=0.1`, '-af', 'volumedetect', '-f', 'null', '-'], { windowsHide: true, encoding: 'utf8', timeout: 10000 });
    assert.equal(reference.status, 0);
    result.ffmpegDbfs = Number(reference.stderr.match(/max_volume:\s*([-\d.]+) dB/)?.[1]);
    assert.ok(Number.isFinite(result.ffmpegDbfs));
    assert.ok(Math.abs(Math.max(-60, result.dbfs ?? -60) - Math.max(-60, result.ffmpegDbfs)) < 0.15, `${result.name}: FFmpeg peak differs`);
  }
  writeFileSync('artifacts/background/audio-meter-graph.json', JSON.stringify({ status: 'passed', scope: 'Production graph in real headless Chromium OfflineAudioContext; not capture/output/installed acceptance', results }, null, 2));
  console.log(`PASS: ${results.length} offline audio graph cases, including sum/cancellation and silent output`);
} finally { await browser.close(); }
