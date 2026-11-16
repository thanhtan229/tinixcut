// Run with node --test; this filename avoids Vitest's *.test discovery.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { classifyCdpStep, canStartCdpCase, summarizeCdpRun } from './cdp-report-policy.mjs';

const valid = () => ({
  parentExit: 0, provenanceVerified: true, result: { status: 'passed' },
  session: { functionalStatus: 'passed', cleanup: { passed: true }, stages: [{ exitCode: 0, timedOut: false, loopbackOwned: true, windowIsolated: true, inputDesktopUnchanged: true }] },
  afterParent: { runtimeSafe: true, appState: 'exited', inputUnchanged: true, listeners: [], processes: [], locks: [], budget: { allowed: true, bytes: 100, reserveBytes: 10, maxBytes: 1000, retainedRuns: 1, maxRetainedRuns: 64, errors: [] } },
});
test('successful function and cleanup pass with safe continuation', () => {
  const result = classifyCdpStep(valid());
  assert.equal(result.status, 'passed'); assert.equal(result.continuation.allowed, true);
});
test('cleanup-only failure preserves functional pass and permits safe evidence collection', () => {
  const data = valid(); data.session.cleanup.passed = false;
  data.parentExit = 1;
  const result = classifyCdpStep(data);
  assert.equal(result.functionalStatus, 'passed'); assert.equal(result.cleanupStatus, 'failed');
  assert.equal(result.status, 'failed'); assert.equal(result.continuation.allowed, true);
});
for (const [name, mutate] of [
  ['functional failure', d => { d.result.status = 'failed'; }],
  ['timed-out stage', d => { d.session.stages[0].timedOut = true; }],
  ['changed input desktop', d => { d.session.stages[0].inputDesktopUnchanged = false; }],
  ['stale evidence or changed binary', d => { d.provenanceVerified = false; }],
  ['live app, port or unknown process state', d => { d.afterParent.runtimeSafe = false; }],
  ['retention budget exceeded', d => { d.afterParent.budget.allowed = false; }],
  ['missing diagnostics', d => { delete d.afterParent; }],
  ['missing functional receipt', d => { delete d.session.functionalStatus; }],
  ['open port despite safe flag', d => { d.afterParent.listeners.push({ processId: 123 }); }],
  ['live process despite safe flag', d => { d.afterParent.processes.push({ state: 'running' }); }],
  ['unknown process state', d => { d.afterParent.processes.push({ state: 'unknown' }); }],
  ['over byte budget despite allowed flag', d => { d.afterParent.budget.bytes = 999; }],
  ['retained run cap reached', d => { d.afterParent.budget.retainedRuns = 64; }],
  ['incomplete inventory', d => { d.afterParent.budget.errors.push('access denied'); }],
  ['unexpected parent termination', d => { d.parentExit = 124; }],
  ['live lock owner', d => { d.afterParent.locks.push({ owners: [{ state: 'running' }] }); }],
  ['unreadable lock ownership', d => { d.afterParent.locks.push({ owners: [], error: 'RM failed' }); }],
]) test(`${name} stops continuation`, () => {
  const data = valid(); mutate(data); const result = classifyCdpStep(data);
  assert.equal(result.continuation.allowed, false); assert.equal(result.status, 'failed');
});
test('preflight permits a not-started app but post-parent requires exited proof', () => {
  const snapshot = valid().afterParent; snapshot.appState = 'not-started';
  assert.equal(canStartCdpCase(snapshot).allowed, true);
  assert.equal(canStartCdpCase(snapshot, true).allowed, false);
});
test('a failed post-parent safety probe preserves verified functional evidence', () => {
  const data = valid(); delete data.afterParent;
  const result = classifyCdpStep(data);
  assert.equal(result.functionalStatus, 'passed'); assert.equal(result.status, 'failed');
  assert.equal(result.continuation.allowed, false);
});
test('aggregate keeps cleanup failure after later functional passes', () => {
  assert.deepEqual(summarizeCdpRun([
    { functionalStatus: 'passed', cleanupStatus: 'failed' },
    { functionalStatus: 'passed', cleanupStatus: 'passed' },
  ], 2, true), { functionalStatus: 'passed', cleanupStatus: 'failed' });
});
test('aggregate distinguishes running, partial and not-run evidence', () => {
  const steps = [{ functionalStatus: 'passed', cleanupStatus: 'failed' }];
  assert.equal(summarizeCdpRun(steps, 2, false).functionalStatus, 'running');
  assert.equal(summarizeCdpRun(steps, 2, true).functionalStatus, 'partial');
  assert.equal(summarizeCdpRun([], 2, true).functionalStatus, 'not-run');
});
