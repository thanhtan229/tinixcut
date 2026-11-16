export function canStartCdpCase(snapshot, afterParent = false) {
  const budget = snapshot?.budget;
  const budgetSafe = budget?.allowed === true &&
    [budget.bytes, budget.reserveBytes, budget.maxBytes, budget.retainedRuns, budget.maxRetainedRuns].every(Number.isFinite) &&
    budget.bytes >= 0 && budget.reserveBytes >= 0 && budget.maxBytes > 0 &&
    budget.bytes + budget.reserveBytes <= budget.maxBytes &&
    Number.isInteger(budget.retainedRuns) && budget.retainedRuns >= 0 && budget.retainedRuns < budget.maxRetainedRuns &&
    Array.isArray(budget.errors) && budget.errors.length === 0;
  const runtimeSafe = snapshot?.runtimeSafe === true && snapshot.inputUnchanged === true &&
    (snapshot.appState === 'exited' || (!afterParent && snapshot.appState === 'not-started')) &&
    Array.isArray(snapshot.listeners) && snapshot.listeners.length === 0 &&
    Array.isArray(snapshot.processes) && snapshot.processes.every(process => process.state === 'exited') &&
    Array.isArray(snapshot.locks) && snapshot.locks.every(lock => !lock.error && Array.isArray(lock.owners) && lock.owners.every(owner => owner.state === 'exited'));
  const reasons = [];
  if (!runtimeSafe) reasons.push('app/process/port/input state is unsafe or unknown');
  if (!budgetSafe) reasons.push('temporary-data budget exceeded or inspection incomplete');
  return { allowed: reasons.length === 0, reasons };
}

export function classifyCdpStep({ session, result, provenanceVerified, afterParent, parentExit }) {
  const stagesPassed = session?.stages?.length > 0 && session.stages.every(stage =>
    stage.exitCode === 0 && stage.timedOut === false && stage.loopbackOwned === true &&
    stage.windowIsolated === true && stage.inputDesktopUnchanged === true);
  const resultPassed = result?.status === 'passed' || (result?.status === undefined && result?.health?.status === 'healthy');
  const functionalStatus = [0, 1].includes(parentExit) && provenanceVerified === true && session?.functionalStatus === 'passed' && stagesPassed && resultPassed ? 'passed' : 'failed';
  const cleanupStatus = session?.cleanup?.passed === true ? 'passed' : 'failed';
  const reasons = [...canStartCdpCase(afterParent, true).reasons];
  if (functionalStatus !== 'passed') reasons.push('functional evidence or provenance failed');
  return {
    functionalStatus, cleanupStatus,
    status: functionalStatus === 'passed' && cleanupStatus === 'passed' && reasons.length === 0 ? 'passed' : 'failed',
    continuation: { allowed: reasons.length === 0, reasons },
  };
}

export function summarizeCdpRun(steps, requestedCount, finished) {
  const phase = field => {
    if (steps.some(step => step[field] === 'failed')) return 'failed';
    if (steps.length === requestedCount && steps.every(step => step[field] === 'passed')) return 'passed';
    return finished ? (steps.length ? 'partial' : 'not-run') : 'running';
  };
  return { functionalStatus: phase('functionalStatus'), cleanupStatus: phase('cleanupStatus') };
}
