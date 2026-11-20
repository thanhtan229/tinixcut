import { spawnSync } from 'node:child_process';
import { readFileSync, statSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const reportPath = resolve('artifacts/background/silence-review-latest.json');
const started = Date.now();
const run = (command, args, env = process.env) => {
  const result = spawnSync(command, args, { encoding: 'utf8', env, windowsHide: true, timeout: 300000 });
  if (result.error || result.status !== 0) throw new Error(`${command} failed: ${result.error ?? result.stderr ?? result.status}`);
  return (result.stdout ?? '') + (result.stderr ?? '');
};
run(process.execPath, ['scripts/qualification/background/headless-silence.mjs']);
const report = JSON.parse(readFileSync(reportPath, 'utf8'));
if (statSync(reportPath).mtimeMs < started - 1000 || Date.parse(report.startedAt) < started - 1000 || report.status !== 'passed' || report.multiOperations?.length !== 4 || report.longOperations?.length !== 4) throw new Error('Missing, stale or incomplete review evidence');
const args = ['-NoProfile', '-NonInteractive', '-File', 'scripts/qualification/background/run-mock-ipc.ps1', '-TestFilter', 'silence'];
const baseEnv = { ...process.env, TINIX_MULTI_CUT_REPORT: reportPath };
delete baseEnv.TINIX_LONG_CUT_SELECTION;
const standard = run('powershell.exe', args, baseEnv);
const output = run('powershell.exe', args, { ...baseEnv, TINIX_LONG_CUT_SELECTION: '1' });
writeFileSync('artifacts/background/silence-chain-native.log', output);
if (!standard.includes('MULTI_CUT_UI_COMMAND_HISTORY_PERSISTENCE_PASS') || !output.includes('MULTI_CUT_UI_COMMAND_HISTORY_PERSISTENCE_PASS')) throw new Error('Both multi-cut history verifications must run');
writeFileSync('artifacts/background/silence-chain-latest.json', JSON.stringify({ status: 'passed', startedAt: new Date(started).toISOString(), finishedAt: new Date().toISOString(), review: reportPath, nativeLog: resolve('artifacts/background/silence-chain-native.log'), scope: 'Muted headless review and separate MockRuntime Command Service; not installed WebView' }, null, 2));
console.log('PASS: silence review to Command Service history and persistence');
