# plan_phase9_crossdissolve_reconciliation.md

> Final reconciliation plan for **TinixCut Phase 9**.
>
> Current phase status: **INCOMPLETE — final reconciliation required**
>
> Scope: `CrossDissolve` capability/UI reconciliation only.
>
> Do not begin Phase 10 work during this task.

## 1. Objective

Resolve the final Phase 9 inconsistency:

`CrossDissolve` exists in the canonical transition type/schema, but actual adjacent-cut / overlap-based dissolve composition is not implemented.

Therefore `CrossDissolve` must **not** be exposed or reported as a production-supported working transition.

The smallest acceptable outcome is:

```text
CrossDissolve remains a canonical extension point
        |
        +--> hidden/disabled from production UI/capability discovery
        OR
        +--> explicitly rejected as unsupported
```

Actual CrossDissolve rendering remains deferred.

## 2. Architectural Requirement

Do not silently approximate an unsupported transition.

Not allowed:

```text
select CrossDissolve
-> accept command
-> preview/export behaves as no-op or invariance
-> report PASS
```

Unsupported semantics must be explicit.

## 3. Required Repository Audit

Locate every place `CrossDissolve` is surfaced:

- transition enum/domain type
- command validation
- capability discovery
- Inspector transition selector
- toolbar/menu if any
- preview normalization
- export normalization
- parity tests
- frontend tests
- automation/CLI capability schema if applicable
- docs/status

Record:

```text
Domain type:
Command path:
UI exposure:
Capability exposure:
Preview behavior:
Export behavior:
Tests:
```

## 4. Production-Support Decision

For Phase 9, set:

```text
CrossDissolve = DEFERRED / NOT PRODUCTION-SUPPORTED
```

Keep the canonical type/schema only if doing so does not imply that the feature is usable.

If persisted projects already contain active `CrossDissolve` semantics, report that before changing behavior.

## 5. UI Reconciliation

Preferred minimal option:

- remove `CrossDissolve` from production transition selectors/capability lists; or
- show it disabled and clearly unavailable.

Normal UI must not create a new active `CrossDissolve`.

Existing supported transitions must remain unaffected.

## 6. Command / Capability Reconciliation

Any attempt to create/set `CrossDissolve` through authoritative command APIs must either:

```text
return explicit unsupported capability/error
```

or be unreachable because capability discovery excludes it.

Prefer a stable structured error using existing conventions, conceptually:

```text
UNSUPPORTED_TRANSITION
transition = CrossDissolve
```

Do not silently downgrade it to another transition.
Do not silently store it as a no-op active feature.

## 7. Preview / Export Reconciliation

Preview and export must not claim working `CrossDissolve` semantics.

Allowed:

```text
unsupported error
```

Not allowed:

```text
success + no-op
success + approximation unrelated to real overlap semantics
success + parity PASS based only on invariance
```

Existing supported transition behavior must remain green.

## 8. Test Changes

Add/update focused tests proving:

### Capability/UI
- `CrossDissolve` is absent or disabled in the production-supported transition list.
- supported transitions remain selectable.

### Command API
- creating/setting `CrossDissolve` is rejected explicitly.
- rejection does not mutate project state.
- project revision does not advance on rejection.
- no history entry is created.

### Preview/export
- unsupported `CrossDissolve` is not silently rendered as success.
- supported transition regressions remain green.

### Capability discovery / automation
If exposed to CLI/automation:
- `CrossDissolve` is not advertised as supported.
- direct attempts still return explicit unsupported result.

## 9. Documentation Reconciliation

### PLAN.md

Keep Phase 9 open until this remediation passes.

After validation, prepare Phase 9 for:

```text
COMPLETE WITH DEFERRED ITEMS
```

Accepted deferred items:

- adjacent-cut / overlap-based `CrossDissolve`
- non-linear Bezier/custom easing keyframes
- multi-channel / spectral waveform visualization

### PROJECT_STATUS.md

Record `COMPLETE WITH DEFERRED ITEMS` only after review accepts the remediation evidence.

### ARCHITECTURE.md

No new architecture direction is required.

Only update if current text incorrectly claims `CrossDissolve` production support.

## 10. Validation

Run targeted tests first, then normal regression:

```text
transition command tests
transition UI/capability tests
transition preview/export tests
parity regression tests
cargo test
cargo clippy -- -D warnings
pnpm test
pnpm run build
pnpm run test:all
```

Use actual commands from `DEVELOPMENT_WORKFLOW.md`.

Do not weaken existing parity thresholds.

## 11. Manual QA

Verify in the running app:

1. open transition controls;
2. confirm `CrossDissolve` cannot be selected as a working transition;
3. confirm supported transitions still work;
4. preview a supported transition;
5. export it;
6. save/reopen;
7. verify no regression.

If `CrossDissolve` is shown disabled, verify the UI clearly communicates unavailability.

## 12. Acceptance Criteria

This remediation passes only when:

- [ ] `CrossDissolve` is explicitly classified as deferred/not production-supported.
- [ ] normal UI does not expose it as a working transition.
- [ ] capability discovery does not advertise it as supported.
- [ ] authoritative command attempts fail explicitly or are prevented by the supported-capability contract.
- [ ] rejected attempts do not mutate canonical project state.
- [ ] preview/export do not silently approximate/no-op it as success.
- [ ] supported transition tests remain green.
- [ ] full repository validation remains green.
- [ ] manual QA is recorded.
- [ ] `PLAN.md` / `PROJECT_STATUS.md` are ready for final Phase 9 reconciliation.

## 13. Required Codex Report

Return:

```text
PHASE 9 CROSSDISSOLVE RECONCILIATION: PASS
```

or:

```text
PHASE 9 CROSSDISSOLVE RECONCILIATION: BLOCKED
```

Include:

```text
UI exposure:
Capability exposure:
Command behavior:
Preview behavior:
Export behavior:
Tests added/updated:
Manual QA:
Full validation:
Docs updated:
```

Also state explicitly:

```text
CrossDissolve production support: DEFERRED
```

Do not mark Phase 10 started.

## 14. Expected Final Phase Decision

If this remediation passes and no new material issue appears during review, Phase 9 should be eligible for:

```text
COMPLETE WITH DEFERRED ITEMS
```

with `CrossDissolve`, advanced easing, and spectral/multi-channel waveform visualization recorded as deferred work.
