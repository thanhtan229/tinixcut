# AGENTS.md

> **Tinix Global Coding Agent Standard**
>
> This document defines the default operating rules for any coding agent working in this repository.
>
> It is intentionally **agent-neutral, framework-neutral, platform-neutral, and project-type-neutral**.

Version: 3.2

**Project adaptation:** Automation-First Desktop Video Editor bootstrap profile

---

## 0. Objective

Complete the current task correctly, safely, and efficiently with:

- the minimum sufficient context;
- the smallest coherent change;
- the least unnecessary work;
- appropriate verification;
- clear evidence of completion.

The default operating loop is:

`UNDERSTAND → LOCATE → READ ENOUGH → CHANGE COHERENTLY → TEST → VERIFY → REPORT`

Explicit current-task instructions and more-specific project rules take precedence over this file.

---

# 1. Instruction Priority

Use the most specific applicable instruction.

Priority order:

1. Current explicit user instruction.
2. Project-specific specification or active plan relevant to the task.
3. Subdirectory-specific `AGENTS.md`, if present.
4. Root `AGENTS.md`.
5. Existing repository conventions and established implementation patterns.

If instructions conflict:

- prefer the more specific rule;
- if equally specific, prefer the more recent explicit rule;
- do not silently ignore a higher-priority instruction.

Do not invent missing requirements.

---

# 2. Repository Documentation Model

Tinix projects may use the following standard project documents:

```text
AGENTS.md
DEVELOPMENT_WORKFLOW.md
UX_UI.md
ARCHITECTURE.md
PLAN.md
PROJECT_STATUS.md
RESEARCH.md
```

These files have different responsibilities.

```text
AGENTS.md
→ How coding agents should work.

DEVELOPMENT_WORKFLOW.md
→ How the project is validated, built, run, reloaded, restarted,
  health-checked, packaged, and prepared for testing.

UX_UI.md
→ How user-facing interfaces should look and behave.

ARCHITECTURE.md
→ How the current technical system is structured.

PLAN.md
→ What substantial work is intended and how it should be implemented.

PROJECT_STATUS.md
→ Where a long-running project or implementation currently stands.

RESEARCH.md
→ External research, experiments, evidence, comparisons, constraints,
  or feasibility findings that inform implementation decisions.
```

Do not duplicate the responsibility of one document inside another.

## 2.1. Project-Specific Source-of-Truth Rules

For this repository:

- `RESEARCH.md` records the evidence and constraints that led to the baseline decisions.
- `ARCHITECTURE.md` is the authoritative durable technical structure.
- `PLAN.md` is the authoritative implementation sequence, scope, phase gates, and acceptance path.
- `PROJECT_STATUS.md` records only the current execution checkpoint.
- `UX_UI.md` governs user-facing interaction and design behavior.
- `DEVELOPMENT_WORKFLOW.md` governs how code is validated and made testable.

Do not copy research prose into implementation code or duplicate architectural decisions across documents.

If implementation reveals that an approved architectural assumption is wrong:

1. gather concrete evidence;
2. stop before building a parallel architecture around the contradiction;
3. update or propose the appropriate architecture/plan decision;
4. then implement against the corrected source of truth.

## 2.2. Project-Specific Architectural Guardrails

The following are hard defaults from the approved architecture until explicitly changed:

1. **New application, not a forked editor codebase.**
2. **Windows-first desktop modular monolith.**
3. **Tauri + React/TypeScript + Rust baseline.**
4. **The versioned editable Project Domain Model is the canonical source of truth.**
5. **All authoritative project mutations pass through the Command Service.**
6. **UI, shortcuts, future CLI/MCP/AI automation must share that mutation path.**
7. **Do not use arbitrary floating-point seconds as canonical timeline time.**
8. **Do not persist FFmpeg graphs, decoder handles, WebView objects, cache paths, or renderer-specific state as project semantics.**
9. **High-frequency pointer/gesture state stays transient until a meaningful edit command is committed.**
10. **Persistence must preserve atomic-save/recovery semantics; derived cache must remain disposable.**
11. **Missing media must be explicit and relinkable; never silently substitute a different asset.**
12. **Preview and export may use different backends but must consume the same normalized composition semantics.**
13. **Do not permanently lock the preview backend before the Preview POC/gate in `PLAN.md`.**
14. **Background work must be bounded and cancellable where appropriate.**
15. **Do not introduce Redis, distributed workers, cloud render orchestration, or microservices for v1 without a new requirement and architecture decision.**
16. **FFmpeg access must stay behind the media/render abstraction; frontend code must not construct unrestricted shell commands.**
17. **Do not expand into deferred features merely because the architecture could support them.**
18. **Automation is a client of the editor, not UI mouse/keyboard simulation and not a second project reducer.**

When a requested change conflicts with one of these guardrails, read `ARCHITECTURE.md` and `PLAN.md` before editing.

---

# 3. Documentation Routing Principle

Do **not** read every project document automatically for every task.

Use the minimum relevant document set.

The purpose of documentation routing is to:

- reduce unnecessary context;
- avoid conflicting stale information;
- keep execution efficient;
- ensure important constraints are not missed.

Always read `AGENTS.md`.

Read other documents when the task matches the conditions defined below.

---

# 4. When to Read `DEVELOPMENT_WORKFLOW.md`

Read `DEVELOPMENT_WORKFLOW.md` when the task involves coding or any activity that may require:

- validation;
- tests;
- lint;
- typecheck;
- compile;
- build;
- development runtime;
- HMR/live reload;
- frontend reload;
- backend/service restart;
- whole-app restart;
- health checks;
- packaging;
- installer behavior;
- release behavior;
- deployment-related local verification.

For normal coding tasks, the agent SHOULD consult the relevant development workflow before final verification.

It is especially important when:

- the repository is unfamiliar;
- the user expects the application to be ready for testing afterward;
- runtime processes are already running;
- frontend/backend services may need reload or restart;
- packaging behavior differs from normal development.

Do not assume:

- Windows;
- desktop app;
- browser app;
- Node;
- Python;
- any specific build command;
- any specific packaging format.

Use the workflow actually documented by the project.

---

# 5. When to Read `UX_UI.md`

Read `UX_UI.md` before implementing or modifying user-facing UI or interaction behavior.

Examples:

- new screens;
- layout changes;
- navigation;
- forms;
- buttons;
- dialogs;
- tables;
- visual states;
- loading/error/empty states;
- responsive behavior;
- accessibility;
- design tokens;
- spacing;
- typography;
- colors;
- component variants;
- interaction patterns.

Also read it when reviewing or refactoring frontend UI that may affect product consistency.

Do not read it for purely backend, infrastructure, data-processing, or documentation tasks unless the task has user-facing implications.

Treat `UX_UI.md` as the source of truth for Tinix visual language and project UI rules.

---

# 6. When to Read `ARCHITECTURE.md`

Read `ARCHITECTURE.md` when the task may affect durable system structure.

Examples:

- adding or removing a service;
- changing module boundaries;
- changing major data flow;
- changing persistence/storage;
- changing database architecture;
- changing API boundaries;
- adding a queue/worker;
- changing authentication architecture;
- changing IPC/native boundaries;
- changing shared infrastructure;
- introducing a major external integration;
- changing deployment/runtime topology;
- modifying cross-module ownership;
- adding a new major subsystem.

Also read it when:

- the repository is unfamiliar and the task is cross-cutting;
- a local change unexpectedly touches multiple major components;
- the current implementation appears to conflict with documented architecture.

Do not require `ARCHITECTURE.md` for routine local implementation work that does not affect system structure.

Do not update `ARCHITECTURE.md` merely because implementation progress changed.

Update it only when a task introduces or changes a durable architectural decision.

---

# 7. When to Read `PLAN.md`

Read `PLAN.md` when:

- the user explicitly asks to implement the plan;
- the task belongs to a documented multi-step feature or project plan;
- the work spans multiple phases;
- acceptance criteria are defined there;
- the current task depends on sequencing defined there;
- the project is being implemented from a previously approved plan.

Use `PLAN.md` to understand:

- objective;
- scope;
- non-goals;
- requirements;
- implementation phases;
- dependencies;
- acceptance criteria;
- rollout/migration expectations.

Do not treat `PLAN.md` as a live status log.

Do not create a new plan file for every small request.

A new plan is appropriate for substantial, multi-step, high-risk, architecture-sensitive, or multi-system work.

Small scoped tasks SHOULD normally be implemented directly.

---

# 8. When to Read `PROJECT_STATUS.md`

Read `PROJECT_STATUS.md` when:

- resuming work from a previous session;
- continuing a partially completed implementation;
- the user says "continue", "resume", or equivalent;
- a long-running project spans multiple sessions;
- the current checkpoint matters;
- blockers, completed phases, or known failures may affect the next action.

Use it to understand:

- current objective;
- completed work;
- work in progress;
- blockers;
- known issues;
- verification state;
- next actions;
- resume context.

Do not use `PROJECT_STATUS.md` as architecture documentation.

Do not treat it as immutable history.

It should remain a concise snapshot of the current state.

Update it only when there is a meaningful checkpoint, such as:

- phase completion;
- major progress;
- new blocker;
- blocker resolution;
- important implementation decision;
- handoff to another session or agent.

Do not update it after every small code change.

---

# 9. When to Read `RESEARCH.md`

Read `RESEARCH.md` when implementation depends on prior research or evidence.

Examples:

- provider/API feasibility;
- library/framework comparison;
- protocol behavior;
- external service constraints;
- compatibility findings;
- performance benchmarks;
- proof-of-concept results;
- reverse-engineering findings;
- product/technical research;
- integration limitations;
- security/compliance findings;
- documented external assumptions.

Also read it when:

- `PLAN.md` references research findings;
- the user says implementation should follow prior research;
- architecture decisions were based on research;
- a current issue may be explained by a documented limitation.

Do not treat `RESEARCH.md` as higher priority than current explicit requirements.

Research records evidence and conclusions available at the time they were written.

If facts may have changed materially and the task requires current accuracy, verify them again before relying on them.

Do not create `RESEARCH.md` for ordinary implementation work that requires no durable research record.

---

# 10. Standard Document Sets by Task Type

Use these defaults unless the task requires more context.

## 10.1. Small local code change

Read:

```text
AGENTS.md
DEVELOPMENT_WORKFLOW.md     relevant sections
```

Additionally read project source/tests relevant to the change.

---

## 10.2. User-facing UI change

Read:

```text
AGENTS.md
UX_UI.md
DEVELOPMENT_WORKFLOW.md     relevant sections
```

Read `PLAN.md` if the UI work belongs to an active plan.

---

## 10.3. Backend/API change

Read:

```text
AGENTS.md
DEVELOPMENT_WORKFLOW.md
```

Read `ARCHITECTURE.md` if shared contracts, storage, module boundaries, or cross-system behavior are affected.

---

## 10.4. Architecture-sensitive task

Read:

```text
AGENTS.md
ARCHITECTURE.md
PLAN.md                     if applicable
DEVELOPMENT_WORKFLOW.md
RESEARCH.md                 if architecture depends on prior research
```

---

## 10.5. Implementing an approved project/feature plan

Read:

```text
AGENTS.md
PLAN.md
```

Then read, as applicable:

```text
PROJECT_STATUS.md           when resuming
ARCHITECTURE.md             for architecture-sensitive phases
UX_UI.md                    for UI phases
RESEARCH.md                 for research-dependent decisions
DEVELOPMENT_WORKFLOW.md     before coding/verification/runtime work
```

---

## 10.6. Resuming a long-running implementation

Read:

```text
AGENTS.md
PLAN.md
PROJECT_STATUS.md
```

Then route to other documents according to the current next action.

---

## 10.7. Research-driven implementation

Read:

```text
AGENTS.md
RESEARCH.md
PLAN.md                     if implementation plan exists
ARCHITECTURE.md             if research affects system structure
DEVELOPMENT_WORKFLOW.md     when implementation begins
```

---

## 10.8. Release / packaging / runtime task

Read:

```text
AGENTS.md
DEVELOPMENT_WORKFLOW.md
```

Read:

```text
ARCHITECTURE.md
```

only if runtime topology or system boundaries matter.

---

## 10.9. Documentation-only task

Read only the documents necessary to edit the requested documentation correctly.

Do not start application runtimes or run expensive tests unless needed to verify the documentation change.

---

# 11. Session Bootstrap Rules

## 11.0. First Repository Bootstrap for This Project

When implementation begins for the first time from the current documentation-only repository state, perform one deliberate bootstrap read:

1. `AGENTS.md`
2. `PLAN.md` — Phase 0 and the immediately following gate context
3. `ARCHITECTURE.md` — principles, runtime baseline, module boundaries, and architecture gates
4. `DEVELOPMENT_WORKFLOW.md` — especially the Project Runtime Profile
5. `UX_UI.md` — project profile only if Phase 0 creates user-facing shell/UI
6. `PROJECT_STATUS.md`
7. `RESEARCH.md` only for evidence needed to resolve a Phase 0 decision or a referenced architecture gate

Then inspect the actual repository state before creating scaffolding.

After this first bootstrap, return to the normal minimum-document routing rules. Do not reread every root document mechanically for every task.

## 11.1. New implementation from an existing plan

When the user asks to begin implementation from `PLAN.md`:

1. Read `AGENTS.md`.
2. Read `PLAN.md`.
3. Identify which companion documents apply to the first implementation phase.
4. Read only those relevant documents.
5. Inspect the relevant repository area.
6. Begin implementation.
7. Follow `DEVELOPMENT_WORKFLOW.md` for validation/runtime behavior.

---

## 11.2. Resume existing implementation

When the user asks to continue previous work:

1. Read `AGENTS.md`.
2. Read `PLAN.md` if the work belongs to a plan.
3. Read `PROJECT_STATUS.md`.
4. Inspect current repository state/diff.
5. Determine the next unfinished action.
6. Read additional companion documents only when relevant.
7. Continue from the current checkpoint.

Do not restart implementation from Phase 1 merely because a new agent/session is being used.

---

## 11.3. No plan exists

If the task is small and clear:

- implement directly;
- do not create a plan file merely for ceremony.

If the task is substantial, multi-phase, high-risk, or cross-system:

- create or propose a concise plan before large-scale implementation when appropriate.

---

# 12. Search Before Reading Broadly

Locate the likely relevant code first using:

- repository search;
- symbol/reference search;
- filenames;
- routes;
- error messages;
- logs;
- test names;
- configuration keys;
- distinctive strings.

For an unfamiliar repository, inspect only the minimum structure needed to orient yourself.

Read enough context to act reliably, but do not scan the entire repository by default.

Preferred sequence:

`SEARCH → READ ENOUGH → EDIT → VERIFY`

---

# 13. Stay in Scope

Do only what the current task requires.

Do not add unrelated:

- features;
- cleanup;
- refactors;
- architecture changes;
- redesigns;
- documentation rewrites;
- dependency upgrades.

Report unrelated issues instead of fixing them unless the task scope is explicitly expanded.

Do not turn a focused task into a general repository improvement pass.

---

# 14. Default Change Constraints

Unless the task explicitly requires otherwise:

- Do not change database schemas.
- Do not change public APIs or external contracts.
- Do not change authentication or authorization behavior.
- Do not add dependencies unnecessarily.
- Do not perform broad refactors.
- Do not change unrelated behavior.
- Do not modify unrelated files.
- Do not introduce new architecture when the existing structure can reasonably support the task.
- Do not rename or reorganize large parts of the repository without need.

If the task requires one of these changes, make only the minimum coherent change necessary.

---

# 15. Protect Existing Work

Assume the repository may contain user work unrelated to the current task.

When useful, inspect repository state and diffs before editing.

Rules:

- Do not overwrite unrelated user changes.
- Do not revert unrelated changes.
- Distinguish your changes from pre-existing modifications.
- Do not assume the working tree is clean.
- When a check fails, determine when practical whether the failure existed before your change.
- Avoid destructive repository operations unless explicitly required.

Preserve unrelated work even when it appears incomplete or inconsistent.

---

# 16. Follow Existing Repository Conventions

Before introducing a new pattern, inspect how the repository already handles similar behavior.

Follow established conventions for:

- naming;
- file organization;
- configuration;
- error handling;
- logging;
- testing;
- state management;
- API structure;
- component organization;
- dependency usage;
- documentation placement.

Prefer extending existing patterns over inventing parallel ones.

Do not introduce a new convention when an established one already works.

---

# 17. Code Quality

Write the simplest correct implementation that fits the existing architecture.

Prefer:

- clear and descriptive names;
- small cohesive functions;
- explicit control flow;
- readable code over clever code;
- reuse of existing abstractions;
- low duplication;
- narrow responsibilities;
- comments that explain **why**, not obvious **what**.

Avoid:

- premature abstraction;
- speculative generalization;
- unnecessary wrapper layers;
- unnecessary helper classes/services;
- dead code;
- commented-out code;
- magic values when an established constant/token exists;
- duplicated logic when a shared implementation is appropriate;
- hidden side effects;
- broad catch-all behavior.

Do not optimize for the fewest lines of code at the expense of clarity.

Do not create abstractions for hypothetical future needs.

---

# 18. Smallest Safe Change

Prefer focused, local, reversible edits.

The **smallest safe change** means the smallest **coherent solution**, not the fewest changed lines.

Prefer a slightly larger correct fix over a tiny workaround that introduces:

- hidden fragility;
- duplicated logic;
- inconsistent behavior;
- silent fallback;
- immediately avoidable technical debt.

Avoid rewriting entire files when a targeted edit is sufficient.

Do not combine unrelated cleanup with feature or bug-fix work.

---

# 19. Change Impact Analysis

Before modifying shared or widely reused code, inspect important callers and dependents.

This especially applies to:

- shared utilities;
- reusable components;
- public functions;
- API contracts;
- configuration;
- persistence/data models;
- common services;
- shared types;
- serialization/deserialization;
- routing/navigation primitives.

Do not assume a locally correct change is globally safe.

Preserve existing behavior unless the task explicitly requires changing it.

For shared changes, verify the most important affected paths.

---

# 20. Error Handling

Do not hide failures to make a task appear successful.

Avoid:

- empty catch blocks;
- silently ignored exceptions;
- broad exception handling without recovery;
- fake success states;
- fallback behavior that masks the real failure;
- logging an error and then pretending the operation succeeded.

Errors should retain enough context to diagnose the problem without exposing secrets.

When recovery is possible:

- handle it explicitly;
- communicate the resulting state clearly.

When recovery is not possible:

- fail clearly;
- preserve useful diagnostic evidence.

Do not convert real errors into silent behavior changes.

---

# 21. Dependency Discipline

Before adding a dependency:

1. Check whether the repository already provides the capability.
2. Check whether the platform/runtime already provides the capability.
3. Prefer an existing dependency when appropriate.
4. Add only the minimum dependency required.
5. Avoid overlapping libraries that solve the same problem.
6. Follow project package-management conventions.

Do not upgrade unrelated dependencies as part of an unrelated task.

Do not add a dependency merely to save a few lines of simple code.

Conversely, do not reimplement complex, security-sensitive, or standards-heavy capabilities merely to avoid a well-justified dependency.

---

# 22. Generated, Vendor, and Build Artifacts

Do not manually edit generated files, vendored source, compiled output, build artifacts, or third-party code unless the task specifically requires it.

Prefer changing the source of truth:

- source code;
- generator;
- schema;
- template;
- build configuration.

Do not preserve temporary debug/build artifacts unless the project explicitly requires them.

---

# 23. Secrets and Sensitive Configuration

Never hard-code:

- credentials;
- access tokens;
- API secrets;
- passwords;
- private keys;
- machine-specific secrets.

Do not expose secrets in:

- logs;
- terminal output;
- screenshots;
- fixtures;
- documentation;
- commits.

Use the project's established configuration and secret-management mechanism.

Do not silently copy local secrets into tracked files.

---

# 24. Testing Rules

Tests should validate behavior, not incidental implementation details.

When behavior changes:

- update or add the smallest relevant test when practical;
- for bug fixes, prefer a regression test that reproduces the original failure;
- test important success paths;
- test important failure paths when relevant;
- keep tests deterministic and isolated where practical.

Do not:

- delete tests merely to make the suite pass;
- disable tests without a concrete reason;
- weaken assertions only to accommodate a broken implementation;
- rewrite unrelated tests;
- over-mock behavior that can be tested reliably;
- create brittle tests tied to implementation details without need.

Run targeted tests first, then broaden according to risk.

For this project, changes to the following areas SHOULD carry the matching regression evidence whenever applicable:

- canonical time and timeline invariants;
- command validation, revision conflict, transaction, and idempotency behavior;
- undo/redo round trips;
- atomic save, recovery, schema migration, and media relink;
- job cancellation/supersession;
- preview/export composition parity;
- automation/manual-command equivalence.

Do not bypass a failing architecture-gate test by weakening the test or adding a second implementation path.

---

# 25. Verify Proportionally

Use actual project checks instead of guessing about correctness.

Start with the smallest meaningful verification and broaden only when risk or project rules justify it.

Possible checks include:

- targeted tests;
- unit tests;
- lint;
- typecheck;
- compile;
- build;
- integration tests;
- smoke tests;
- health checks;
- runtime verification.

Always run mandatory checks defined by applicable project instructions.

Do not run expensive full-suite validation mechanically when smaller checks provide sufficient confidence.

Do not skip meaningful verification simply to save time.

Fix failures caused by or directly related to the current task.

Report unrelated pre-existing failures instead of silently expanding scope.

---

# 26. Debug From Evidence

Use evidence such as:

- error messages;
- stack traces;
- failing tests;
- logs;
- diffs;
- runtime state;
- network/API responses;
- process state;
- reproducible behavior.

Do not chain blind speculative fixes.

If repeated hypotheses fail:

1. stop speculative editing;
2. return to evidence;
3. identify what is known;
4. identify what remains uncertain;
5. choose the next diagnostic step that reduces uncertainty.

When useful, summarize:

- current failure;
- evidence;
- attempted fixes;
- results;
- likely remaining causes;
- relevant files/subsystems;
- recommended next diagnostic action.

Continue editing when new evidence materially narrows the cause.

---

# 27. Runtime and Service Safety

Do not assume the project is a:

- desktop app;
- web app;
- SaaS product;
- backend service;
- CLI;
- mobile app;
- worker;
- hybrid system.

Follow `DEVELOPMENT_WORKFLOW.md` for runtime-specific behavior.

General rules:

- Restart only affected services.
- Do not stop unrelated processes.
- Do not restart production or remote services unless explicitly authorized.
- Do not identify ownership by port number alone when stronger evidence exists.
- Do not interrupt unrelated or data-sensitive work unless the applicable project workflow explicitly allows it.
- Leave the relevant development runtime in a testable state when runtime verification is required.

---

# 28. Git Rules

Do not create Git commits automatically.

Commit only when explicitly requested.

When a commit is requested:

- include only task-related changes;
- use one clear commit unless otherwise requested;
- do not silently include unrelated working-tree changes.

Do not:

- push;
- force-push;
- rebase;
- reset;
- rewrite history;

unless explicitly requested.

---

# 29. Plan Only When Useful

Create a short implementation plan before editing when the task is:

- broad;
- high-risk;
- cross-cutting;
- multi-system;
- migration-heavy;
- architecture-sensitive.

For small, obvious, local tasks, implement directly.

Do not produce a formal plan when it adds no execution value.

A plan should reduce risk or clarify sequencing, not create ceremony.

Do not create a new `PLAN_*.md` for every small feature or fix.

---

# 30. Delegate Sparingly

Do not create subagents or delegated workers by default.

Delegate only when work is genuinely separable and delegation reduces context, execution time, or cognitive load.

Avoid:

- several workers reading the same large repository;
- duplicated investigation;
- delegation for trivial work;
- parallel edits to tightly coupled files without coordination.

Prefer one focused execution path for small and medium tasks.

---

# 31. Documentation Discipline

Keep project documents current only when their subject actually changes.

## Update `ARCHITECTURE.md` when:

- a durable architecture decision changes;
- module/service boundaries change;
- major data flow changes;
- major infrastructure/integration changes.

Do not update it for routine implementation progress.

## Update `PLAN.md` when:

- planned scope changes;
- implementation sequencing changes;
- acceptance criteria change;
- the plan is materially revised.

Do not use it as a progress log.

## Update `PROJECT_STATUS.md` when:

- a meaningful checkpoint is reached;
- a phase completes;
- a major blocker appears or resolves;
- a session handoff needs durable context.

Do not update it after every minor edit.

## Update `RESEARCH.md` when:

- durable research evidence changes;
- a new experiment or comparison materially affects decisions;
- previously documented assumptions are disproven or superseded.

Do not use it as a scratchpad for ordinary debugging.

## Update `UX_UI.md` when:

- durable global/project UI rules change;
- project UI profile changes;
- a deliberate design-system exception is adopted.

Do not rewrite it for one-off screen details.

## Update `DEVELOPMENT_WORKFLOW.md` when:

- the durable build/test/run/reload/restart/package workflow changes.

Do not update it merely because one task used a particular command.

Prefer updating an existing canonical document over creating another source of truth.

---

# 32. Definition of Done

A coding task is complete only when the requested behavior is implemented and the relevant verification has been performed.

Before reporting completion, confirm when applicable:

- requested behavior works;
- acceptance criteria are satisfied;
- important edge cases were considered;
- relevant regression risk was checked;
- tests/checks pass;
- build succeeds when required;
- latest runtime is active when runtime verification is required;
- no unrelated behavior was intentionally changed;
- known blockers or limitations are clearly identified;
- any applicable `PLAN.md` phase/gate acceptance criteria are satisfied or explicitly reported as not yet satisfied;
- no project-specific architectural guardrail was bypassed by a parallel mutation, persistence, media, or automation path.

Compilation alone is not proof of task completion.

A passing test suite alone is not proof that the requested behavior is correct.

Do not claim success when required verification failed.

---

# 33. Final Report

Keep completion reports concise and evidence-based.

For coding tasks, include only what is relevant:

- what changed;
- key files changed;
- validation performed and result;
- runtime/reload/restart/health status when applicable;
- documentation updated when applicable;
- unresolved issue or limitation, if any.

Do not:

- restate unchanged code;
- explain routine edits at length;
- produce verbose summaries without value;
- claim something was tested when it was not.

Mention Git commit status only when relevant to the task.

---

# 34. Preferred Engineering Bias

When multiple valid solutions exist, prefer the one that is:

1. correct;
2. consistent with the existing repository;
3. simple;
4. easy to read;
5. easy to test;
6. easy to debug;
7. low in hidden coupling;
8. reversible;
9. efficient enough for the actual requirement.

Do not optimize prematurely.

Do not introduce complexity without demonstrated need.

---

# 35. Anti-Patterns

Avoid:

- reading every project document for every small task;
- broad refactors during small tasks;
- speculative abstractions;
- duplicated implementations of existing behavior;
- silent fallbacks;
- swallowing exceptions;
- disabling tests to get green status;
- changing many unrelated files;
- unnecessary dependency additions;
- modifying generated output instead of its source;
- architecture changes without need;
- large formatting-only diffs mixed with logic changes;
- claiming completion without verification;
- repeated blind retries;
- over-engineering a simple requirement;
- updating project status after every tiny edit;
- creating plan files for trivial tasks;
- treating stale research as current fact without checking when freshness matters.

---

# 36. Quick Document Router

Use this checklist before starting substantial work.

```text
Always:
[ ] AGENTS.md

Coding / test / runtime / build:
[ ] DEVELOPMENT_WORKFLOW.md

User-facing UI:
[ ] UX_UI.md

Architecture-sensitive:
[ ] ARCHITECTURE.md

Implementing an approved multi-step plan:
[ ] PLAN.md

Resuming previous work:
[ ] PROJECT_STATUS.md

Depends on prior research / external evidence:
[ ] RESEARCH.md
```

Read only the documents that apply.

---

# 37. Core Operating Principle

Prefer:

`UNDERSTAND → ROUTE DOCUMENTS → LOCATE → READ ENOUGH → CHANGE COHERENTLY → TEST → VERIFY → REPORT`

Avoid:

`READ EVERYTHING → SCAN EVERYTHING → REFACTOR BROADLY → GUESS → PATCH BLINDLY → OVER-REPORT`

The goal is not to make the repository better in general.

The goal is to complete the user's current task correctly, safely, efficiently, and consistently with the project.
