# Cumulative Baseline Commit Plan — Phases 19–29 (READY FOR COMMIT, not executed)

**Label: `phases-19-29-cumulative-baseline`. This is NOT a P29-only commit.**
Snapshot freeze: `CANDIDATE-FREEZE.json` · Inventory: `SNAPSHOT-INVENTORY.md`
P29 code fingerprint: `a4f00b9f4f481568d51085ee678eb986d209ce016a0aa11ec0539a83e2e00bc3`
Working-tree status hash: `3f1d077691d76be15e53e8d630639ebb31ed9e24f1b2cdd40d6dc31d75855f72`
HEAD: `e6462544b6956054ee50af40727d44e92121487b`
Verdicts on this snapshot: QA PASS · Code Reviewer LGTM
Status: **no commit, no merge, no push, no tag created**

## Why cumulative

HEAD is the Phase 18 commit. Phase 19–28 work was never committed and is interleaved with Phase 29
in the same files. There is no buildable/verifiable per-phase commit chain available from this
tree. The only traceable, verifiable snapshot is the cumulative working tree verified by QA and the
Reviewer (fingerprint above).

## Proposed commit message

```
feat: cumulative verified baseline through phase 29

Snapshot of the working tree from the phase-18 commit through phases 19-29.

This is a cumulative baseline, not a phase-29-only change. Phases 19-28 were
never committed and share files with phase 29, so no buildable per-phase chain
can be reconstructed from this tree.

Phase 29 (Gate 29 PASS):
- rich-text/animation domain model with grapheme-indexed spans and limits
- schema v10 additive, lossless v9->v10 migration
- 7 Command Service operations + ClipTextSet grapheme-span rebinding
- composition/preview carry-through, TypeScript mirrors, CLI/MCP parity
- 13 Phase 29 tests + 12 independent QA tests

Earlier phases 19-28 (text templates, captions, canvas/aspect, effects,
speed, auto-captions, text renderer POC) are included as their verified
working state. Phase 26 native qualification remains open.

Evidence: artifacts/animated-text-templates/phase-29/evidence-summary.json
QA: PASS. Code Reviewer: LGTM. No merge/release.
```

## Staging command (when approved)

```
git add -A -- . \
  ':(exclude)artifacts/**/*.png' \
  ':(exclude)artifacts/**/*.mp4' \
  ':(exclude)artifacts/animated-text-templates/phase-28/runs/**' \
  ':(exclude)artifacts/animated-text-templates/phase-29/phase28-benchmark-current-default-profile.csv' \
  ':(exclude)artifacts/animated-text-templates/phase-29/phase28-benchmark-current-opt3-profile.csv'
```

`node_modules/`, `dist/`, `src-tauri/target/` are already gitignored. No Auto-Recovery files exist in
this tree. Expected staged scope: 118 files, ~3.2 MB (135 files in the tree before binary/current-run
exclusions).

## Pre-commit verification (when approved)

1. `git status --porcelain` hash must equal `3f1d0776…855f72` from CANDIDATE-FREEZE.json.
2. Recompute the P29 fingerprint; must equal `a4f00b9f…e00bc3`.
3. `git diff --cached --check` — no whitespace errors or conflict markers.
4. `git diff --cached --stat` — confirm no `*.png`, `*.mp4`, `runs/`, current-benchmark CSVs,
   `tools/execution-guard`, `.auto-recovery`, `node_modules`, `dist`, `target`.
5. Secret scan on staged content (already clean at freeze time; re-check after staging).
6. Commit locally only. Do not push, tag, merge or release.

## Rollback

- Before push: `git reset --soft HEAD~1` (keep index/worktree) or `git reset --mixed HEAD~1`.
- After push (not authorized): `git revert <sha>`; never rewrite shared history.
- Note: once a project is saved as schema v10, pre-v10 code rejects it; keep `.tinix` backups before
  any code rollback (no down-migration exists).
