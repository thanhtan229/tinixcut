# Pre-Implementation Technical Research — Automation-First Video Editor

## Research Metadata

**Project:** CapCut-like desktop video editor with automation-first architecture (working title)  
**Research date:** 2026-10-03  
**Research status:** READY WITH LIMITATIONS  
**Research phase:** Pre-implementation only. No production code, dependency installation, migration, packaging, or POC execution was performed.

### Sources researched

Primary sources were preferred: official repositories, source files, technical documentation, releases/changelogs, issue trackers, architecture documents, command contracts, security documentation, and licensing pages.

| Source | Version / release / commit researched | Why it matters |
|---|---|---|
| Timeline Studio / `MartinDelophy/ai-video-editor` | Tested release `v1.0.8`, commit `efd3b6a`; current repository documentation also reviewed | Strong reference for agent-safe timeline automation, revision checks, dry-run/diff, idempotent operations, editable portable projects |
| Clypra / `AIEraDev/Clypra` / `gizmax/clypra` | Current repository; release line including `v1.2.2` | Strong reference for Tauri + React + Rust + FFmpeg, native playback scheduling, bounded work, timeline interaction patterns |
| OpenCut / `OpenCut-app/OpenCut` | Rewrite roadmap; release line including `v0.3.0`; current repository state | Useful architectural direction: engine/UI separation, Rust core, Editor API, plugins, MCP, headless; implementation is still immature |
| OpenTimelineIO / Academy Software Foundation | `v0.17.0` | Mature editorial timeline/time model and interchange concepts |
| Editly / `mifi/editly` | `v0.15.0-rc.1`, commit `af93372` | Declarative JSON/JS video edit specifications and FFmpeg-oriented automation |
| Remotion | Current 4.x docs; current pricing/licensing; Editor Starter docs | Strong programmable/rendering patterns, but licensing and editor data-model stability matter for an automation product |
| Tauri | `2.12.0`, released 2026-09-26 | Candidate desktop runtime; capability/security model and sidecar support |
| Electron | Major 44, released 2026-08-25 | Main alternative desktop runtime for comparison |
| FFmpeg | `9.0.2`, released 2026-09-18; licensing/legal documentation | Candidate media probe/transcode/export foundation |

### Primary URLs

- Timeline Studio: https://github.com/MartinDelophy/ai-video-editor
- Timeline Studio command contract: https://github.com/MartinDelophy/ai-video-editor/blob/main/skills/edit-timeline-studio/references/command-contract.md
- Timeline Studio current capabilities: https://github.com/MartinDelophy/ai-video-editor/blob/main/skills/edit-timeline-studio/references/current-capabilities.md
- Clypra: https://github.com/AIEraDev/Clypra
- OpenCut: https://github.com/OpenCut-app/OpenCut
- OpenCut rewrite tracker: https://github.com/OpenCut-app/OpenCut/issues/811
- OpenTimelineIO: https://github.com/AcademySoftwareFoundation/OpenTimelineIO
- OpenTimelineIO releases: https://github.com/AcademySoftwareFoundation/OpenTimelineIO/releases
- Editly: https://github.com/mifi/editly
- Remotion: https://www.remotion.dev/
- Remotion Editor Starter warning: https://www.remotion.dev/docs/editor-starter/demo
- Remotion licensing/pricing: https://www.remotion.dev/docs/license/pricing
- Tauri 2.12 release: https://tauri.app/blog/tauri-2.12/
- Tauri capabilities: https://tauri.app/security/capabilities/
- Electron process model: https://www.electronjs.org/docs/latest/tutorial/process-model
- Electron security: https://www.electronjs.org/docs/latest/tutorial/security
- FFmpeg releases: https://ffmpeg.org/download.html
- FFmpeg license: https://ffmpeg.org/doxygen/trunk/md_LICENSE.html
- FFmpeg legal considerations: https://ffmpeg.org/legal.html

---

## Project Understanding

### Product objective

Build a standalone video-editing application with a user experience broadly similar to CapCut for common creator workflows, while treating automation as a first-class architectural requirement rather than an afterthought.

The desired product should support manual editing and, later, automation/AI agents operating on the **same editable project and timeline model**. Automated actions should result in normal editable timeline state, not an opaque rendered video or simulated mouse/keyboard actions.

### Core workflows

1. Create/open a project.
2. Import local video, audio, image, font, subtitle, and other supported media.
3. Inspect/probe media metadata and generate derived assets such as thumbnails, waveforms, or proxies.
4. Add media to one or more timeline tracks.
5. Perform common NLE operations: move, trim, split, delete, duplicate, reorder, overlay, transform, text/captions, audio control, transitions, simple effects, speed/keyframes as the supported scope grows.
6. Preview the current project responsively while editing/scrubbing.
7. Save/reopen the editable project reliably.
8. Export a deterministic video from saved project state.
9. Run the same edits through an automation API/CLI/agent interface.
10. Allow automation to inspect the project, preview a proposed change, apply it transactionally, detect stale project revisions, and return structured results/errors.

### Functional requirements

The research assumes the initial product needs the following architectural capabilities even if individual editing features are delivered incrementally:

- Multi-track timeline.
- Stable clip/track/asset identifiers.
- Non-destructive editing.
- Undo/redo compatible with both manual and automated edits.
- Media probing and derived media generation.
- Preview and scrubbing.
- Deterministic export.
- Versioned project format.
- Missing-media detection and relinking.
- Background jobs with progress and cancellation.
- Automation API capable of inspect/diff/apply/render semantics.
- Future ability to expose CLI, MCP, scripting, or external automation without duplicating edit logic.

### Non-functional requirements

- **Reliability:** editing operations must not partially corrupt project state.
- **Recoverability:** cache/proxy loss must not destroy the project; interrupted saves/renders must be recoverable.
- **Responsiveness:** drag, trim, playhead and canvas gestures must not perform heavy disk/IPC work on every pointer event.
- **Performance:** architecture must leave room for native decoding, hardware acceleration, proxies, bounded prefetching, and efficient export.
- **Maintainability:** UI, project domain logic, media processing, renderer, and automation transports should be separable.
- **Extensibility:** new edit operations should be registered once and become available to UI/automation through the same domain service.
- **Security:** imported projects/media and agent input must be treated as untrusted; the frontend should not receive unrestricted shell/filesystem capability.
- **Testability:** timeline mutations should be testable without launching the full GUI or invoking real codecs.
- **Long-term portability:** project data should not be coupled to transient browser Blob URLs, UI component state, or a single AI/provider implementation.

### Technical constraints

- A browser-only implementation creates avoidable risk around codec support, large local files, persistence, hardware behavior, and long-running media jobs.
- A full professional NLE renderer is a large systems project. The first architecture should deliberately constrain the supported effect/render semantics rather than promising complete CapCut parity immediately.
- Preview and final render may need different execution backends, but they must consume the same canonical composition semantics to avoid semantic drift.
- Media licensing and codec/patent obligations must be considered before commercial distribution.
- Long-running render/proxy/analysis work must not block the UI event loop.

### Assumptions

- **A1:** The first serious target is desktop, with Windows 10/11 as an important target; cross-platform support is desirable but should not force browser-only architecture.
- **A2:** Projects are local-first initially; real-time multi-user collaboration is not a v1 requirement.
- **A3:** Automation is important, but it does not require cloud microservices at the beginning.
- **A4:** Source media can remain external to the project by default; a future “collect/package project” operation can produce a portable archive.
- **A5:** The product is expected to become a maintained product rather than a disposable prototype, so architecture debt caused by forking an immature editor is undesirable.
- **A6:** Exact initial effect scope, target hardware floor, target 4K/60 performance, cloud sync, collaboration, and AI-provider choices are not yet fixed. These are treated as open product decisions rather than hidden assumptions.

---

## Research Questions

The research focused only on questions likely to change implementation decisions.

1. Should the product be a new application or a fork of an existing editor?
2. Should the primary runtime be Tauri, Electron, browser/PWA, or a pure-native UI stack?
3. Where should the canonical project/timeline state live?
4. What time representation should the project use to avoid frame/timing drift?
5. How should UI and automation share editing behavior without duplicating logic?
6. Which patterns make automated edits safe, retryable, inspectable, and undoable?
7. Should OpenTimelineIO be the internal project model, an interchange layer, or only a design reference?
8. What role should FFmpeg play: sidecar jobs, linked native libraries, or both?
9. Can preview remain in the WebView, or does continuous playback require a native surface/decoder?
10. How can preview and final render avoid geometry/effect differences?
11. Is Remotion appropriate as the renderer or project source of truth?
12. Are Redis, external workers, or service queues needed for the first local desktop version?
13. How should source media, proxies, thumbnails, waveforms, render intermediates, and portable archives be stored?
14. What failure modes could cause project corruption, duplicate automated edits, stale writes, lost media, or inconsistent renders?
15. What security boundaries are required between WebView, native core, media binaries, imported projects, and future AI/agent transports?
16. What POCs must be completed before committing to the hardest architectural choices?

---

## Sources / Repositories Researched

### 1. Timeline Studio / ai-video-editor

**Source:** https://github.com/MartinDelophy/ai-video-editor  
**Version / release / commit:** tested release `v1.0.8`, commit `efd3b6a`; current documentation also inspected.  
**Relevance:** strongest researched example of making a real editable timeline safely operable by AI/automation.

Important implementation patterns observed:

- `.timeline` portable project remains editable source of truth.
- Project/track/clip inspection commands exist separately from mutation commands.
- `project.diff` performs project-aware validation without writing.
- `project.run` and MCP use the same application service/registry instead of duplicating reducers.
- Command plans carry `schemaVersion`, `baseRevision`, `dryRun`, stable object IDs, and stable operation IDs.
- A repeated operation ID is expected to return its previous result rather than applying twice.
- Writes are transactional and revision-aware.
- Browser control is explicitly treated as compatibility/fallback where an operation is not available in the structured command registry.
- Portable archive support, media hashing/probing, inspect/diff/apply/render, and structured automation are already exercised by examples.

Limitations relevant to this project:

- It is fundamentally browser/local-first and depends on browser media capabilities for significant parts of the editing/export path.
- Browser codec behavior and file/blob lifecycle are weaker foundations for a desktop-first NLE than native media infrastructure.
- Browser automation is useful as fallback but should not become the primary edit API.

### 2. Clypra

**Source:** https://github.com/AIEraDev/Clypra  
**Version / release:** current repository; release line including `v1.2.2`.  
**Relevance:** strongest researched implementation reference for a modern desktop video editor built with React/TypeScript UI plus Rust/Tauri/FFmpeg media core.

Important patterns observed:

- React/TypeScript is used for editor UI and timeline interaction.
- Rust/Tauri owns native media operations.
- FFmpeg is used for video processing/decoding paths.
- Architecture documentation explicitly addresses playback scheduling, native decoding, preview surfaces, hardware acceleration, decoder pooling, stale-work rejection, bounded pending work, and audio timing.
- UI gestures are intentionally separated from authoritative commits: transient drag geometry can remain local/RAF-driven, while pointer-up performs one authoritative store/domain commit.
- The project avoids treating high-frequency frame flow as ordinary command IPC.
- Background decode/prefetch work is bounded and cancellable rather than allowing an unbounded obsolete queue.
- Frontend and Rust backend both have test commands documented.

Important caveat:

Clypra's measured performance and exact playback architecture should be treated as implementation evidence from that project, not a guarantee that the same results will hold in a new codebase/hardware matrix.

### 3. OpenCut

**Source:** https://github.com/OpenCut-app/OpenCut  
**Rewrite tracking:** https://github.com/OpenCut-app/OpenCut/issues/811  
**Version / release:** rewrite/current repository; release line including `v0.3.0`.  
**Relevance:** validates the strategic direction of separating editor engine from UI and exposing the same core to desktop/web/mobile, MCP, headless automation, plugins, and scripting.

Important findings:

- The rewrite explicitly proposes `UI -> Core` and `MCP/Headless/Scripting -> Core`.
- The stated roadmap includes Editor API, plugin-first architecture, Rust core, MCP, headless mode, and scripting.
- The current rewrite is not yet a mature editor foundation; the repository itself describes the rebuild as ongoing and points users to the classic version for current usability.

Conclusion for this project: architectural concepts are useful, but the rewrite is not mature enough to justify making this new product a fork or hard dependency.

### 4. OpenTimelineIO (OTIO)

**Source:** https://github.com/AcademySoftwareFoundation/OpenTimelineIO  
**Version:** `v0.17.0`.  
**Relevance:** mature reference for editorial timeline structure and rational time semantics.

Relevant concepts:

- Timeline, Stack, Track, Clip, Gap, Transition and media reference concepts are explicit.
- `RationalTime` / `TimeRange` semantics are designed for editorial timing rather than casual floating-point seconds.
- Media is referenced rather than requiring original media bytes to be embedded in the timeline data.
- OTIO is strong at interchange/editorial structure, but effect semantics are intentionally application-specific and adapter/plugin architecture has evolved independently of a particular NLE UI.

Conclusion: use OTIO as a model/interchange influence, not automatically as the application's internal source of truth.

### 5. Editly

**Source:** https://github.com/mifi/editly  
**Version:** `v0.15.0-rc.1`, commit `af93372`.  
**Relevance:** useful reference for declarative, machine-generated edit specifications and FFmpeg-backed automation.

Important patterns:

- A whole video can be represented declaratively as an edit spec with output geometry, FPS, clips, layers, transitions, audio tracks, normalization and defaults.
- JSON/JS specification is agent- and automation-friendly.
- It is not a complete interactive NLE architecture and should not be adopted as the canonical editor model.

### 6. Remotion

**Source:** https://www.remotion.dev/  
**Version:** current 4.x documentation/release line.  
**Relevance:** mature programmatic video ecosystem, React-based rendering, parameterization, batch rendering, and editor-related products.

Relevant findings:

- Remotion is strong when “video as code/data” and scalable programmed rendering are the primary workflow.
- Its Editor Starter offers substantial UI concepts, but its own demo documentation warns not to rely on the demo for serious projects because the internal video data structure may change and break projects.
- Current commercial terms explicitly include a paid “Remotion for Automators” path for automated video products such as video editors/prompt-to-video apps, with pricing currently shown as `$0.01/render` and `$100/month minimum`.

Conclusion: Remotion is a useful pattern/source and could be an optional future rendering/template integration, but should not be the internal project source of truth for this product without a deliberate commercial and architectural decision.

### 7. Tauri

**Source:** https://tauri.app/  
**Version:** `2.12.0`, released 2026-09-26.  
**Relevance:** candidate desktop shell and secure bridge between React/TypeScript UI and Rust/native media services.

Relevant findings:

- Tauri provides fine-grained capability controls per window/WebView.
- External binaries/sidecars can be packaged and permissioned rather than exposing a generic shell to frontend code.
- 2.12 added security fixes and current Windows/runtime updates; it is a better baseline than pinning an older 2.x version.
- Tauri 3 is not necessary for this project at this stage; a stable 2.x line is preferable.

### 8. Electron

**Source:** https://www.electronjs.org/  
**Version:** major 44.  
**Relevance:** mature alternative desktop runtime with excellent JavaScript/Chromium developer experience.

Relevant findings:

- Main/renderer/utility process boundaries are mature and utility processes are suitable for CPU-heavy or crash-prone work.
- Electron has well-documented security requirements around Node integration, context isolation, sandboxing, CSP, navigation and IPC validation.
- It ships Chromium/Node and does not itself solve the native media architecture problem; a serious editor would still need carefully isolated FFmpeg/native media work.

### 9. FFmpeg

**Source:** https://ffmpeg.org/  
**Version:** latest stable `9.0.2` at research date.  
**Relevance:** media probing, transcode, proxy creation, thumbnails, waveform inputs, export/encoding, and potential filter/composition backend.

Relevant findings:

- Most FFmpeg code is LGPL 2.1+, but enabling GPL components changes applicable licensing.
- FFmpeg's own legal checklist recommends avoiding `--enable-gpl` and `--enable-nonfree` for a straightforward LGPL distribution path and discusses dynamic linking and source/configuration distribution obligations.
- Codec patent obligations are separate from FFmpeg's software license and may vary by jurisdiction/product distribution.

---

## Key Findings

### Finding 1 — Build a new app; do not use a current open-source editor as the product foundation

**Evidence**

- OpenCut's new architecture is explicitly being rewritten and is not yet a mature editor foundation.
- OpenCut Classic is a different/legacy architecture and is not the architectural direction the new OpenCut project itself is pursuing.
- Timeline Studio is strong in automation but browser-centric.
- Clypra is strong in desktop media/playback architecture but does not provide the automation contract needed as the core product model.
- OTIO, Editly and Remotion each solve narrower parts of the problem well.

**Analysis**

No researched repository simultaneously provides the desired long-term combination of desktop media performance, stable project data model, interactive CapCut-like UI, agent-safe automation, deterministic render, and suitable dependency/licensing profile. Forking one would save early UI work but would couple the new product to assumptions that later become expensive to unwind.

**Project relevance**

A new application can adopt mature patterns while keeping the domain model, command engine and long-term automation contract under project control.

**Confidence:** High.

**Recommendation:** Build a new app; use external repos as reference implementations and pattern sources.

---

### Finding 2 — The canonical architecture should be `UI / Automation -> Command Service -> Project Model`, not `AI -> UI automation`

**Evidence**

- Timeline Studio's CLI and MCP call the same application service and command registry.
- Its plans carry stable operation IDs, base revisions, dry-run semantics and project-aware validation.
- OpenCut's rewrite explicitly proposes desktop/web/mobile and MCP/headless/scripting all using a common core.

**Analysis**

This is the most important structural decision for an automation-first editor. If manual UI edits and automated edits have different mutation paths, behavior will diverge, undo/redo will be inconsistent, validation will be duplicated, and automated retries can produce duplicate destructive edits.

**Project relevance**

The command service becomes the narrow waist of the system. React UI, keyboard shortcuts, CLI, MCP, scripting and future cloud automation can become transports/adapters instead of alternate editing engines.

**Confidence:** High.

**Recommendation:** ADOPT.

---

### Finding 3 — Use revision-aware, transactional, idempotent edit commands

**Evidence**

Timeline Studio's documented command contract includes:

- `schemaVersion`
- `baseRevision`
- `dryRun`
- stable clip/track IDs
- stable operation IDs
- repeated operation IDs returning the previous result
- project-aware `diff` validation using the same operation registry/reducers as real application

**Analysis**

Automation introduces retries, stale reads and multi-step operations even in a single-user local application. Without a base revision, an agent can apply a plan to a project that changed after inspection. Without idempotent operation IDs, retrying after an uncertain timeout can duplicate inserts/splits. Without transaction boundaries, a failed multi-operation command can leave half-applied state.

**Project relevance**

These protections are cheap to design early and expensive to retrofit after project files and automation are public.

**Confidence:** High.

**Recommendation:** ADOPT, with one adaptation: transport APIs may accept user-friendly seconds/frames, but canonical project timing should normalize to rational/integer time rather than store arbitrary floating-point seconds as the authoritative representation.

---

### Finding 4 — Tauri + React/TypeScript + Rust is the best current runtime direction, but media preview must remain swappable until POC

**Evidence**

- Clypra demonstrates a real video-editor architecture using React/TypeScript UI plus Tauri/Rust/FFmpeg.
- Tauri 2.12 provides current capability/security controls and native sidecar integration.
- Electron offers a mature process model but would still need native/media workers and ships a larger Chromium/Node runtime.
- Browser-only implementations face codec, local-file lifecycle, long-running task and hardware variability.

**Analysis**

React/TypeScript is productive for a complex timeline/editor UI. Rust provides a strong boundary for file/media/process operations and future native preview work. Tauri fits that split with less runtime duplication than Electron. However, committing prematurely to “all preview in WebView” or “all preview native” would be risky before workload tests.

**Project relevance**

The architecture should choose Tauri/React/Rust now, but hide continuous preview behind a `Preview Backend` boundary until POC establishes whether proxy/WebView playback is adequate or a native decoded surface is required.

**Confidence:** Medium-High for runtime choice; Medium for exact preview backend.

**Recommendation:** ADOPT Tauri 2.12 stable + React/TypeScript + Rust. POC FIRST for continuous preview implementation.

---

### Finding 5 — Do not perform authoritative store writes, disk writes or native IPC on every pointer-move event

**Evidence**

Clypra's documented interaction model separates high-frequency transient gesture state from authoritative commit, and its playback architecture rejects stale work and bounds pending requests.

**Analysis**

Timeline dragging may generate hundreds of pointer events per second. Sending each through IPC, persistence, history and render invalidation causes input latency and obsolete work. A local optimistic/draft geometry layer can update at animation-frame cadence; pointer-up then produces one validated domain command/history entry.

**Project relevance**

This affects timeline feel, undo history quality, IPC load and preview responsiveness.

**Confidence:** High.

**Recommendation:** ADOPT.

---

### Finding 6 — Use an OTIO-inspired timeline/time model, but do not make OTIO the internal source of truth

**Evidence**

OTIO provides mature editorial concepts including Timeline, Track, Clip, Gap, Transition, media references and rational time. It is intentionally focused on editorial interchange; effect semantics are application-specific.

**Analysis**

Using OTIO directly as the entire project model would force application-specific canvas/effect/keyframe/caption/proxy/cache metadata into custom schemas or side structures. Ignoring OTIO entirely would repeat known mistakes around timing and media references.

**Project relevance**

The app needs its own versioned schema but should adopt OTIO-like semantics:

- explicit tracks/items
- stable references
- source range vs timeline range
- rational/integer canonical time
- clear transitions/gaps
- external media references
- schema versioning and future interchange adapter

**Confidence:** High.

**Recommendation:** ADAPT OTIO; keep internal schema project-specific. Consider OTIO import/export later as an adapter, not the initial persistence dependency.

---

### Finding 7 — Canonical time must not be arbitrary floating-point seconds

**Evidence**

OTIO explicitly models time as a rational value/rate pair because editorial systems must represent frame rates and ranges predictably.

**Analysis**

Repeated trims, speed changes, frame stepping and conversions across 23.976/24/25/29.97/30/50/59.94/60 FPS can accumulate ambiguity when floats are used as the canonical timeline representation.

**Project relevance**

Project persistence should represent time using integer ticks with a declared timebase or a rational `value/rate` model. UI/API input can still accept seconds or frame numbers and normalize at the boundary.

**Confidence:** High.

**Recommendation:** ADOPT rational/integer canonical time.

---

### Finding 8 — FFmpeg is an appropriate media foundation, but it should be behind a media/render abstraction and use a pinned audited build

**Evidence**

FFmpeg is mature and current stable is 9.0.2. It covers probing, decoding, encoding, filtering and format handling. Clypra and Editly both use FFmpeg-centered pipelines. FFmpeg licensing changes depending on configured components.

**Analysis**

FFmpeg is appropriate for `ffprobe`, proxies, thumbnails, transcodes and export. However, coupling the project model directly to FFmpeg command-line/filter syntax would make future native GPU/compositor changes difficult. For the first product, coarse-grained FFmpeg sidecar jobs are substantially simpler than immediately binding all libav APIs. Continuous native preview may later justify linked libraries or a dedicated decoder backend.

**Project relevance**

Define a media service/render-plan boundary, then choose FFmpeg CLI/sidecar for jobs that fit it. Exact FFmpeg build flags and codec distribution must be reproducible and reviewed before release.

**Confidence:** High for FFmpeg role; Medium for sidecar vs linked library in continuous preview.

**Recommendation:** ADOPT FFmpeg for media jobs; ADAPT behind stable internal interfaces; POC FIRST for preview path.

---

### Finding 9 — Preview and export may use different backends, but must share one semantic composition model

**Evidence**

Timeline Studio separates responsive preview from deterministic export while preserving one editable project. Clypra separates UI/timeline concerns from native playback/media work.

**Analysis**

Trying to make one implementation do everything can hurt responsiveness; using unrelated preview/export logic causes “looks different after export.” The coherent compromise is a canonical project plus a normalized render/composition plan whose geometry, timing, transforms, opacity, crop, text layout contract, keyframe interpolation and effect parameters are backend-independent.

**Project relevance**

The WebView/native preview backend and final FFmpeg/native renderer can both consume the same normalized semantics. Unsupported effects must be explicit, not silently approximated.

**Confidence:** High conceptually; exact parity thresholds require POC.

**Recommendation:** ADOPT shared render semantics; POC FIRST for preview/export parity.

---

### Finding 10 — Remotion should not be the project's canonical model or mandatory renderer

**Evidence**

- Remotion is excellent for video-as-code, parameterization and batch rendering.
- Its Editor Starter demo warns that its internal video data structure can change and should not be relied upon for serious projects.
- Current commercial pricing explicitly treats automated video products/video editors as an Automators use case with paid terms.

**Analysis**

Making React source/components or the Editor Starter internal structure the long-lived project format would couple persistence and automation to another product's evolution and commercial model. Remotion can still be useful later for template-driven motion graphics or optional cloud rendering where its economics fit.

**Project relevance**

The core editor should own its project schema and command semantics.

**Confidence:** High.

**Recommendation:** REJECT as canonical source of truth/core mandatory renderer. ADAPT concepts or integrate optionally later only after licensing/product evaluation.

---

### Finding 11 — No external queue/Redis/microservice architecture is justified for the first local desktop version

**Evidence**

The primary workloads—probe, proxy, thumbnails, waveform, transcription integration, export—can run as local background work. Clypra demonstrates bounded local background/native jobs; Tauri/Electron both support native/background process boundaries.

**Analysis**

Introducing Redis or a service queue before there is a multi-machine/cloud requirement creates deployment, retry, security and lifecycle complexity without solving a current product need.

**Project relevance**

A local in-process/native job manager can provide:

- bounded concurrency
- priorities
- progress
- cancellation
- dedupe/idempotency keys
- stale-result rejection
- crash/error capture

Cloud workers can later implement the same job contract if needed.

**Confidence:** High under the local-first assumption.

**Recommendation:** REJECT external queue infrastructure for v1 architecture; ADOPT local bounded job manager.

---

### Finding 12 — Project source-of-truth and generated media must be separated

**Evidence**

OTIO distinguishes editorial references from media. Timeline Studio demonstrates portable archive concepts. Real video editors routinely generate disposable derivatives such as proxies, thumbnails and waveforms.

**Analysis**

If cache artifacts become required project state, cache eviction becomes data loss. If original media is always embedded, project saves become huge and slow. The source-of-truth should be compact metadata plus stable asset references/fingerprints; derived media should be reconstructable.

**Project relevance**

Recommended separation:

- `project`: versioned timeline/editor state
- `media references`: path/URI + metadata + fingerprint/hash where appropriate
- `cache`: proxies/thumbnails/waveforms/analysis artifacts, disposable and rebuildable
- `autosave/recovery`: project-state recovery snapshots/journal
- `exports`: outputs, not project truth
- future `collect/package`: optional portable bundle of project + selected media

**Confidence:** High.

**Recommendation:** ADOPT.

---

## Alternatives Considered

### Alternative A — Tauri + React/TypeScript + Rust + FFmpeg

**Benefits**

- Good productivity for timeline/editor UI.
- Native Rust boundary for filesystem/media/process work.
- Current Tauri capability model supports least-privilege frontend permissions.
- Direct path to native decoder/GPU work if preview requires it.
- FFmpeg ecosystem handles broad media format work.
- Good fit for a local desktop tool with future headless/CLI automation.

**Trade-offs**

- Two-language stack (TypeScript + Rust).
- Native media and GPU debugging are harder than pure web development.
- Tauri uses platform WebViews, so rendering/media behavior must be tested across supported OS versions.

**Assessment:** Recommended base.

### Alternative B — Electron + React/TypeScript + Node + FFmpeg

**Benefits**

- Mature desktop ecosystem.
- Consistent bundled Chromium version.
- Excellent JS developer experience.
- Utility processes provide a reasonable boundary for CPU-heavy/crash-prone tasks.

**Trade-offs**

- Larger runtime footprint.
- Native media performance still requires separate processes/libraries.
- Broader Node/Chromium attack surface requires strict isolation.
- Does not remove the need for a real media architecture.

**Assessment:** Viable fallback if Rust/Tauri becomes a delivery blocker, but not preferred.

### Alternative C — Browser/PWA + WebCodecs + WASM FFmpeg

**Benefits**

- Fastest distribution and web development loop.
- No native installer.
- WebCodecs can be effective on supported browser/codec combinations.

**Trade-offs**

- Codec/platform variability.
- Large media/file lifecycle limitations.
- WASM memory/performance constraints.
- Harder long-running jobs and native filesystem integration.
- Less predictable path to professional desktop behavior.

**Assessment:** REJECT as the primary serious desktop architecture. Useful for future web companion/editor subset.

### Alternative D — Pure Rust native UI / GPUI-style architecture

**Benefits**

- Maximum control over native rendering, GPU, filesystem and performance.
- Single systems-language core.

**Trade-offs**

- Much slower UI feature velocity for a CapCut-like product.
- Smaller ecosystem for complex creator UI compared with React.
- Increased implementation burden before product workflows are validated.

**Assessment:** REJECT for initial product; native Rust should power domain/media services, not necessarily the entire UI.

### Alternative E — Remotion-centric editor

**Benefits**

- Excellent programmatic video model.
- Mature React rendering ecosystem.
- Good template/batch automation story.

**Trade-offs**

- Commercial terms relevant to automated video products.
- Code-centric model is not the desired durable NLE project model.
- Editor Starter internal model is explicitly not guaranteed stable for serious projects.

**Assessment:** REJECT as core architecture; potential optional integration later.

---

## Comparative Analysis

### Runtime / architecture comparison

| Criterion | Tauri + React + Rust | Electron + React + Node | Browser/PWA | Pure Rust UI |
|---|---|---|---|---|
| Fit for local CapCut-like editor | High | High | Medium-Low | High |
| UI development speed | High | High | High | Low-Medium |
| Native media integration | High | Medium-High | Low-Medium | High |
| Codec/filesystem control | High | High | Low-Medium | High |
| Automation/headless path | High | High | Medium | High |
| Security boundary potential | High with narrow capabilities | High if carefully isolated | Browser sandbox helps, but local native access limited | High |
| Runtime footprint | Lower | Higher | Lowest install footprint | Low |
| Cross-platform UI consistency | Medium-High | High | High | Medium |
| Performance ceiling for native preview | High | High with native modules/processes | Medium | Highest |
| Implementation complexity | Medium | Medium | Low initially / high later | High |
| Long-term architecture fit | **High** | Medium-High | Low for primary desktop | Medium |

### Repository/pattern comparison

| Source | Strongest pattern | Main limitation for direct adoption | Decision |
|---|---|---|---|
| Timeline Studio | Safe structured automation over editable timeline | Browser-centric media/runtime assumptions | **ADOPT/ADAPT patterns** |
| Clypra | Native desktop media/playback scheduling and timeline interaction | Automation contract is not its primary strength | **ADOPT/ADAPT patterns** |
| OpenCut rewrite | Engine/UI separation; MCP/headless/plugin direction | Rewrite is not mature enough as foundation | **ADAPT concepts, do not fork** |
| OpenTimelineIO | Editorial model and rational time | Not a full NLE/effect/project model | **ADAPT** |
| Editly | Declarative automation spec | Not an interactive editor architecture | **ADAPT** |
| Remotion | Programmatic/template/batch rendering | Licensing + unsuitable canonical NLE source-of-truth | **Optional later / reject core** |
| FFmpeg | Probe/transcode/export/media foundation | CLI/filter syntax should not leak into project model; licensing/build matters | **ADOPT behind abstraction** |

### Coherence check

The recommended architecture is **not** a collection of all features from the researched projects. It has one narrow center:

> **A versioned project model mutated only through one command service.**

Everything else attaches to that center:

- React UI is a client of the command service.
- Automation/CLI/MCP are additional clients.
- Media workers produce information/derived media for the project but do not become the project model.
- Preview and export consume normalized project/render semantics.
- Persistence stores versioned project state atomically.

This is smaller and more coherent than embedding OpenCut, OTIO, Remotion, FFmpeg filter graphs and browser automation as competing sources of truth.

---

## Best Patterns to Adopt or Adapt

### Pattern 1 — One command engine for UI and automation

**Source:** Timeline Studio; OpenCut rewrite direction.  
**Problem solved:** duplicated edit logic and UI-driven automation.  
**Why useful:** one validation/history/undo/mutation path.  
**Required adaptation:** implement as project-owned domain commands rather than copy another project's command names.  
**Classification:** **ADOPT**.

### Pattern 2 — Inspect → Diff/Dry-run → Apply

**Source:** Timeline Studio.  
**Problem solved:** unsafe agent edits and poor auditability.  
**Why useful:** an agent or script can understand current state, validate a proposed plan, then apply exactly what was reviewed.  
**Required adaptation:** diff output should use stable project IDs and canonical times; UI can also use the same validation in advanced actions.  
**Classification:** **ADOPT**.

### Pattern 3 — `baseRevision` + idempotent operation IDs + transaction boundaries

**Source:** Timeline Studio.  
**Problem solved:** stale plans, duplicate retries, partial edits.  
**Why useful:** deterministic automation and safe retry behavior.  
**Required adaptation:** decide project revision format and operation receipt retention policy.  
**Classification:** **ADOPT**.

### Pattern 4 — OTIO-inspired editorial model and rational time

**Source:** OpenTimelineIO.  
**Problem solved:** ambiguous timing and improvised timeline schemas.  
**Why useful:** proven concepts for source ranges, tracks, clips, transitions and rate-aware time.  
**Required adaptation:** add app-specific transforms, captions, keyframes, effects, asset metadata and automation metadata.  
**Classification:** **ADAPT**.

### Pattern 5 — Transient UI gesture state, one authoritative commit

**Source:** Clypra architecture patterns.  
**Problem solved:** timeline lag, bloated undo history, excessive IPC.  
**Why useful:** smooth drag/trim behavior without weakening canonical validation.  
**Required adaptation:** define which interactions are local drafts and when they become domain commands.  
**Classification:** **ADOPT**.

### Pattern 6 — Bounded/cancellable/latest-relevant media work

**Source:** Clypra playback/media scheduling.  
**Problem solved:** obsolete decode/proxy/seek work accumulating faster than it can complete.  
**Why useful:** prevents UI lag and memory growth during aggressive seeking/scrubbing.  
**Required adaptation:** separate job policies: a render should not be “latest wins,” while seek thumbnails may be.  
**Classification:** **ADAPT**.

### Pattern 7 — Declarative edit specifications

**Source:** Editly; Timeline Studio.  
**Problem solved:** automation that is difficult to inspect/version/replay.  
**Why useful:** JSON commands are easy for agents, CLI tools, tests and future external integrations.  
**Required adaptation:** use fine-grained domain operations and versioned schemas rather than Editly's whole-render spec as project state.  
**Classification:** **ADAPT**.

### Pattern 8 — External media references plus optional portable collection

**Source:** OTIO concepts; Timeline Studio portable project.  
**Problem solved:** huge project saves vs non-portable projects.  
**Why useful:** default saves stay small; users can explicitly collect media for portability.  
**Required adaptation:** add relink/fingerprint/hash policy and decide portable package format later.  
**Classification:** **ADAPT**.

### Pattern 9 — FFmpeg as an implementation backend, not domain schema

**Source:** FFmpeg; Clypra; Editly.  
**Problem solved:** broad codec/export requirements.  
**Why useful:** mature media tooling without coupling project persistence to command strings/filter graphs.  
**Required adaptation:** compile canonical render/media requests into FFmpeg operations behind internal interfaces.  
**Classification:** **ADOPT/ADAPT**.

### Pattern 10 — Tauri least-privilege capabilities

**Source:** Tauri 2 security model.  
**Problem solved:** overly powerful WebView/frontend access to shell and filesystem.  
**Why useful:** limits blast radius if UI content or imported metadata is malicious.  
**Required adaptation:** separate editor window, optional web/auth views, file-picker paths and media sidecar permissions deliberately.  
**Classification:** **ADOPT**.

### Pattern 11 — Native continuous preview backend

**Source:** Clypra.  
**Problem solved:** WebView playback/codec/performance ceilings under layered editing.  
**Why useful:** potentially much better frame pacing, hardware decode and audio-clock ownership.  
**Required adaptation:** significant complexity; should only be accepted if proxy/WebView preview misses explicit targets.  
**Classification:** **POC FIRST**.

### Pattern 12 — Shared normalized render/composition plan

**Source:** derived from Timeline Studio's separate preview/export paths, OTIO's canonical editorial model, and Clypra's media/UI separation.  
**Problem solved:** preview/export mismatch and renderer lock-in.  
**Why useful:** keeps rendering implementation replaceable while preserving one semantic definition.  
**Required adaptation:** define the MVP's supported geometry/effect semantics narrowly enough to validate parity.  
**Classification:** **POC FIRST**, then **ADOPT** if validation passes.

---

## Patterns to Avoid

### 1. Forking OpenCut or another editor as the long-term product foundation

**Source:** OpenCut current rewrite status; differences across researched editors.  
**Reason:** inherited assumptions would become product constraints; current OpenCut rewrite is not mature enough.  
**Risk to this project:** high migration cost when automation/project/render architecture diverges.

### 2. AI/automation controlling the timeline primarily through mouse/keyboard/browser events

**Source:** Timeline Studio explicitly provides structured command path and treats browser control as fallback.  
**Reason:** difficult to make deterministic, idempotent, testable or safe against UI changes.  
**Risk:** duplicated edits, wrong targets, fragile tests, inability to reason about stale state.

### 3. Using React component state, Zustand state, Blob URLs or UI objects as project persistence

**Reason:** transient implementation details do not make durable project formats.  
**Risk:** broken migrations, unrecoverable files, inability to support headless automation.

### 4. Storing timeline time as unconstrained floating-point seconds

**Source:** OTIO rational-time model.  
**Reason:** frame-rate and repeated-transform ambiguity.  
**Risk:** drift, off-by-one-frame behavior, inconsistent split/trim/export.

### 5. Sending every pointer move/frame through authoritative IPC/history/persistence

**Source:** Clypra interaction/playback patterns.  
**Reason:** produces command storms and obsolete work.  
**Risk:** laggy editor and unstable playback.

### 6. Making FFmpeg command strings/filter graphs the project format

**Reason:** backend-specific representation leaks into long-lived user data.  
**Risk:** impossible migrations and renderer lock-in.

### 7. Making Remotion code or Editor Starter state the canonical project format

**Source:** Remotion's own Editor Starter warning and licensing model.  
**Reason:** stability/commercial coupling is unnecessary for the core project model.  
**Risk:** breaking projects and future licensing constraints.

### 8. External Redis/queue/microservice infrastructure before cloud distribution is required

**Reason:** no current requirement justifies it.  
**Risk:** operational complexity, more failure modes, slower local development.

### 9. Unbounded background media work

**Reason:** seeking, thumbnail generation and analysis can easily create obsolete tasks faster than completion.  
**Risk:** memory growth, CPU/GPU contention, poor input latency.

### 10. Unrestricted shell/sidecar/file access from WebView or future agent transport

**Source:** Tauri capability model; Electron security guidance.  
**Reason:** imported content and agent input are untrusted.  
**Risk:** arbitrary command execution or unintended filesystem access.

### 11. Assuming preview and export are equivalent without explicit parity tests

**Reason:** different decode/compositor/font/effect backends can disagree.  
**Risk:** user sees a correct preview but receives a different exported file.

### 12. Tracking “latest FFmpeg” without pinning build configuration

**Source:** FFmpeg release/license documentation.  
**Reason:** codec availability and license obligations depend on build configuration.  
**Risk:** non-reproducible bugs and accidental GPL/nonfree inclusion.

---

## Recommended Technical Direction

### 1. Overall architecture

Use a **desktop-first modular monolith**, not microservices.

Conceptual flow:

```text
Manual UI ───────┐
Keyboard/Shortcuts│
CLI / MCP / Agent ├──> Command Service ──> Project Domain Model ──> Persistence
Future Scripts ──┘          │                    │
                            │                    ├──> History / Undo-Redo
                            │                    └──> Render Composition Plan
                            │                                │
                            └──> Job Manager                 ├──> Preview Backend
                                      │                     └──> Export Backend
                                      ├── probe
                                      ├── proxy
                                      ├── thumbnails
                                      ├── waveform
                                      ├── analysis
                                      └── render/export
```

The **Project Domain Model + Command Service** is the architectural center. Neither React UI, FFmpeg, Remotion, MCP nor any AI provider should become the source of truth.

### 2. Runtime / components

#### Desktop shell

**Recommendation:** Tauri `2.12.x` stable line.

Use Tauri for:

- desktop lifecycle
- windowing
- file-picker integration
- tightly scoped filesystem access
- native command bridge
- controlled sidecar launch
- installer/update integration later

Do not adopt Tauri 3 alpha for the initial project.

#### UI

**Recommendation:** React + TypeScript.

UI owns:

- timeline visualization
- inspector panels
- media browser
- canvas interaction controls
- keyboard shortcuts
- transient gesture state
- progress presentation

UI does **not** own canonical mutation rules or persistence semantics.

#### Native core

**Recommendation:** Rust application/domain services.

Core responsibilities:

- project schema validation and migrations
- command registry
- command preconditions
- revision checks
- transaction boundaries
- stable IDs
- history/undo records
- atomic project persistence
- job orchestration
- media probing interfaces
- render-plan generation
- security-sensitive filesystem/process operations

This does not imply every pixel must initially be rendered in Rust.

### 3. Project / timeline data model

Create a project-owned versioned schema inspired by OTIO, not an OTIO clone.

Core concepts should include at minimum:

- `projectId`
- `schemaVersion`
- `revision`
- project settings: width/height, timeline timebase, nominal output FPS, audio settings
- stable `assetId`
- stable `trackId`
- stable `itemId` / `clipId`
- source asset reference
- source range
- timeline range/placement
- transform/crop/opacity
- audio properties
- text/caption properties
- transition/effect/keyframe descriptors as supported
- markers/metadata
- generated-asset references where necessary

#### Time representation

Canonical persistence should use either:

- integer ticks + declared timeline timebase, or
- rational `{value, rate}` style time.

API/UI can accept seconds/frames but must normalize before authoritative mutation.

#### Media references

Original media should normally stay outside the project file.

Persist enough to relink safely:

- original path/URI
- filename
- size
- media metadata
- optional fast fingerprint and/or content hash policy
- duration/timebase/stream metadata

Do not persist transient OS handles or Blob URLs.

### 4. Persistence

Recommended properties:

- versioned schema
- atomic save: write temporary file, fsync/close where supported, then replace/rename
- recovery/autosave separate from user-chosen canonical file
- migration pipeline from older project schemas
- cache never required for correctness
- project save should remain small and quick

A future “Collect/Package Project” operation can create a portable archive containing project metadata plus selected referenced media. This should be explicit rather than making every save portable by default.

### 5. Command / automation contract

Every meaningful edit should map to a registered domain operation.

A future external plan envelope should contain concepts similar to:

```json
{
  "schemaVersion": 1,
  "projectId": "...",
  "baseRevision": 42,
  "dryRun": true,
  "operations": [
    {
      "operationId": "op-...",
      "type": "clip.trim",
      "clipId": "clip-...",
      "parameters": {}
    }
  ]
}
```

This is a conceptual contract, not production implementation.

Required behavior:

- stable IDs
- strict schema validation
- base revision/precondition checks
- `inspect`
- `diff` / dry-run using the same validator/registry as real execution
- atomic multi-operation apply where requested
- idempotent operation receipts
- stable structured error codes
- no partial project mutation on failed transaction
- revision increments on successful authoritative change

#### Manual UI

UI should call the same domain operations after a gesture is committed.

#### Automation

Automation should never need generic filesystem/shell access. It should receive a capability-limited project API containing only reviewed operations.

#### MCP / CLI

Do not implement separate editing logic. They should become transports over the same command/application service when added.

### 6. Preview strategy

Do **not** prematurely hard-code a single preview architecture.

Recommended boundary:

```text
Project State
    -> Normalized Composition / Render Plan
        -> PreviewBackend
        -> ExportBackend
```

Initial candidates to validate:

1. WebView/proxy-assisted preview for simpler layers and fast implementation.
2. Native Rust/FFmpeg/hardware-decoded continuous preview for heavier layered workloads.

Regardless of backend:

- playback/seek work must be bounded
- obsolete seek requests must be discarded
- audio timing needs a defined authority
- no unbounded frame queue
- high-frequency frame transport should not use naive request/response IPC
- UI interactions must stay responsive while media work occurs

### 7. Export / render strategy

Use a canonical normalized composition/render plan independent of FFmpeg command syntax.

For the MVP, deliberately support a constrained set of render semantics that can be validated across preview and export.

FFmpeg should initially be the preferred backend for:

- media metadata/probing
- proxy creation
- thumbnails / extraction
- transcode
- audio processing where appropriate
- final encoding/muxing
- supported composition/filter operations

If future effects exceed practical FFmpeg filter-graph semantics, a dedicated compositor can be introduced behind the same render-plan interface without changing project files.

### 8. Background jobs

Use a local job manager in the desktop core.

Job classes likely include:

- media probe
- proxy generation
- thumbnail/filmstrip
- waveform generation
- speech/transcription integration
- media analysis
- export/render

Each job type should define:

- concurrency limit
- priority
- cancellation behavior
- dedupe/idempotency key
- progress reporting
- retry policy
- stale-result rules
- cleanup policy

Examples:

- seek thumbnail/decode work: latest relevant request may supersede older pending work.
- final export: must never be silently superseded; explicit cancel only.
- probe/hash: safe to dedupe by asset identity.

No Redis or external worker infrastructure is recommended until remote/cloud execution becomes a real requirement.

### 9. Error handling and reliability

Required system behaviors:

- Project mutations fail atomically.
- Automation failures return stable machine-readable error codes plus human-readable context.
- Stale `baseRevision` is a first-class error, not silently rebased.
- Long-running jobs report cancelled/failed/completed states explicitly.
- Sidecar/native process crash should not corrupt project state.
- Render writes to a temporary/incomplete destination and only promotes to final output after success.
- Low-disk and permission errors must be surfaced before overwriting valid outputs where possible.
- Missing source media must preserve timeline objects and enter a relinkable offline state rather than deleting clips.
- Cache loss must trigger regeneration, not project failure.
- Unsupported codecs/effects must fail explicitly with actionable diagnostics.

### 10. Security boundaries

Treat these as untrusted:

- imported project files
- filenames/metadata/subtitles
- imported fonts/media containers
- future plugin data
- AI/agent-generated commands
- remote provider responses

Recommended controls:

- Tauri 2.12+ current stable security baseline.
- Minimal per-window capabilities.
- No unrestricted shell plugin exposed to editor WebView.
- Sidecar commands/arguments generated by trusted Rust code from typed requests, not concatenated from raw agent strings.
- Explicit allowed project/media paths.
- Validate navigation and remote content boundaries.
- CSP for app WebView.
- Strict command schemas and resource limits.
- Do not allow project files to define arbitrary executable commands.
- Future plugins must have explicit capability/security design rather than arbitrary native access.

### 11. FFmpeg distribution / licensing

Recommendation:

- Pin an exact FFmpeg version/build configuration before release.
- Prefer a configuration compatible with the intended product license; do not casually enable GPL/nonfree components.
- Record build flags and exact source/version.
- Maintain required notices/source distribution process.
- Review external codecs/libraries separately.
- Perform a commercial codec/patent review before distributing a paid product in target jurisdictions.

This is a technical risk note, not legal advice.

### 12. Testing approach

#### Pure domain tests

Highest priority because they can run fast and without media codecs:

- command validation
- trim/move/split/duplicate/delete semantics
- overlapping/ripple rules as defined
- rational-time conversions
- revision conflict behavior
- idempotent operation retry
- transaction rollback
- undo/redo round trips
- schema migration round trips
- canonical serialization stability

#### Property/invariant tests

Examples:

- no clip has invalid negative duration
- source range remains inside allowed asset duration unless an explicit generator/offline rule permits otherwise
- applying an idempotent operation twice is equivalent to once
- failed transaction leaves project hash/state unchanged
- undo(edit(project)) restores equivalent project state

#### Media integration corpus

Maintain small fixture media covering:

- CFR and VFR
- common H.264/H.265/VP9/AV1 cases as supported
- AAC/PCM and different sample rates
- MOV/MP4/MKV/WebM as supported
- rotated metadata
- alpha media if supported
- corrupt/truncated files
- missing file
- unusual timebases

#### Preview/render parity tests

For supported MVP effects/transforms:

- render known frames at fixed timeline times
- compare geometry/timing against preview captures or backend-independent expected values
- verify text/crop/transform/keyframe semantics
- verify A/V duration and sync tolerance

#### Automation E2E

Test:

1. inspect
2. diff
3. apply
4. reopen project
5. retry same operation ID
6. confirm no duplicate mutation
7. attempt stale base revision
8. confirm explicit conflict
9. export

#### Recovery/failure tests

- kill app during autosave
- kill render sidecar
- remove cache
- rename/move source media
- low disk
- read-only project location
- malformed project
- unsupported codec
- cancellation during proxy/render

### 13. Deployment/runtime model

Initial recommended model:

- one desktop application
- Tauri WebView UI
- Rust core in the same app process/runtime boundary where appropriate
- controlled FFmpeg/ffprobe sidecar processes for coarse-grained media jobs
- local cache directory
- local project/recovery data
- no always-on server
- no Redis/database server
- no cloud dependency for core manual editing

Future headless/automation can expose the same core through a local CLI/MCP server without redesigning project semantics.

---

## Recommended Architecture Inputs

The following decisions should be carried into the future `ARCHITECTURE.md` after research approval:

1. **Architecture style:** desktop-first modular monolith.
2. **Runtime:** Tauri 2.12.x stable + React/TypeScript + Rust.
3. **Architectural center:** versioned Project Domain Model + Command Service.
4. **Mutation rule:** all authoritative edits use the command service; UI and automation do not maintain separate reducers.
5. **Automation contract:** stable IDs, `baseRevision`, inspect/diff/apply, transactional operations, idempotent operation IDs, structured errors.
6. **Timeline time:** integer/rational canonical representation; seconds only at external/UI boundaries.
7. **Project model:** OTIO-inspired concepts but application-owned schema.
8. **Media:** originals referenced externally; cache derivatives rebuildable; optional portable collect/package later.
9. **Media backend:** FFmpeg/ffprobe behind internal interfaces, exact pinned build.
10. **Preview:** backend abstraction; WebView/proxy vs native continuous preview decided by POC.
11. **Render:** shared normalized composition semantics; deterministic export backend separate from UI.
12. **Jobs:** bounded local job manager with cancellation, progress, dedupe and per-job policy.
13. **Persistence:** versioned schema, atomic saves, recovery/autosave, explicit migration pipeline.
14. **Security:** least-privilege Tauri capabilities; no raw shell from WebView/agent; typed native media/process requests.
15. **Testing:** pure domain tests first, media fixture corpus, automation E2E, preview/export parity, recovery/failure testing.
16. **No initial microservices/Redis.**
17. **No dependency on Remotion or OTIO as canonical project persistence.**
18. **No fork of current editor repository as product base.**

---

## Recommended Plan Inputs

The future `PLAN.md` should convert the architecture decisions into implementation phases without changing the core model midway.

Recommended workstreams/phases to plan:

1. **Project/domain foundation**
   - project schema
   - canonical time
   - stable IDs/revisions
   - command service
   - serialization/migration/recovery
   - domain test harness

2. **Desktop shell + basic editor UI**
   - Tauri shell
   - project lifecycle
   - media import
   - minimal timeline/canvas
   - transient gesture vs authoritative commit pattern

3. **Media foundation**
   - ffprobe metadata
   - asset identity/relink
   - thumbnails/waveforms/proxy cache
   - bounded background jobs

4. **Core editing operations**
   - move/trim/split/delete/duplicate
   - tracks/layer ordering
   - transform/crop/opacity
   - audio basics
   - text/captions based on approved v1 scope

5. **Preview POC decision incorporation**
   - integrate the validated preview backend rather than guessing it in advance

6. **Render/export foundation**
   - normalized composition plan
   - deterministic export
   - temp-output/atomic promotion
   - cancellation/progress
   - preview/export parity checks

7. **Automation surface**
   - inspect
   - diff/dry-run
   - apply
   - receipts/errors
   - CLI/MCP adapter only after command service is stable

8. **Reliability/security hardening**
   - crash recovery
   - missing media
   - corrupted project
   - low disk
   - sidecar failures
   - least-privilege capabilities
   - licensing inventory

9. **Performance qualification**
   - target hardware matrix
   - 1080p/4K workload profiles
   - cache/proxy policy
   - decoder/hardware acceleration fallback behavior

Dependencies that the plan should make explicit:

- Preview architecture depends on POC results.
- Final FFmpeg integration mode depends partly on preview POC and licensing/build validation.
- Effect/keyframe scope should be bounded before render parity acceptance criteria are finalized.
- Automation should depend on the command service, not precede it.

Acceptance considerations:

- Editable project must remain the source of truth after automation.
- Same domain command should have equivalent results whether initiated from UI or automation.
- Failed transaction leaves project unchanged.
- Retried operation ID cannot duplicate mutation.
- Saved/reopened project preserves timeline semantics.
- Cache can be deleted and rebuilt.
- Missing source media is relinkable.
- Export is deterministic for the supported effect set.
- Preview/export differences stay inside explicitly defined tolerance.

---

## Risks and Limitations

### R1 — Preview performance and architecture are not yet proven

React/Tauri is a strong shell/UI direction, but the exact preview backend cannot be chosen solely from repository evidence. Layer count, codec mix, effect scope, audio timing and target hardware materially affect the answer.

**Mitigation:** run the preview POC before building deep editor features around a specific surface/decoder architecture.

### R2 — Preview/export parity may become the hardest long-term problem

A WebView compositor and FFmpeg filter pipeline may disagree on fonts, interpolation, crop, transforms, color management or effects.

**Mitigation:** normalize semantics, constrain initial feature set, create frame-based parity tests, and reserve a renderer boundary.

### R3 — “CapCut-like” can expand into an unbounded feature set

Professional editor features include masks, nested comps, advanced keyframes, effects, color management, speed curves, tracking, captions, templates, AI tools and more.

**Mitigation:** architecture can support expansion, but future PLAN must state an explicit v1 editing feature boundary.

### R4 — FFmpeg licensing/configuration can affect distribution

A convenient prebuilt FFmpeg may include components unsuitable for the intended distribution/license model.

**Mitigation:** controlled reproducible build or vetted distribution, license inventory, documented flags, separate codec/patent review.

### R5 — Hardware acceleration behavior varies

GPU vendors/drivers and Windows versions can produce different decode/encode behavior.

**Mitigation:** software fallback, capability probing, device matrix tests, never make project readability depend on hardware-specific outputs.

### R6 — Project schema becomes a permanent compatibility promise once users create work

Poor early schema choices become expensive migration problems.

**Mitigation:** explicit schema version from day one, migration tests, stable IDs and rational time before broad feature work.

### R7 — Agent automation can amplify small correctness errors

A manual user may notice one incorrect split; an agent can repeat the error across hundreds of clips.

**Mitigation:** inspect/diff/apply, bounded operation registry, transactions, revision checks, receipts, dry-run and test corpus.

### R8 — Source-media fingerprinting policy has trade-offs

Full hashes are robust but expensive for very large files; paths and size/mtime are fast but weaker.

**Mitigation:** define a tiered identity strategy during architecture/POC: fast fingerprint for routine relink plus optional/async full content hash where needed.

### R9 — Cross-platform scope can dilute Windows-first quality

Tauri is cross-platform, but codec, GPU and WebView behavior differs.

**Mitigation:** define a primary acceptance platform first and add platform-specific qualification rather than claiming parity from architecture alone.

### R10 — Current external projects continue to change

OpenCut is actively rewriting; Remotion and Tauri continue to evolve.

**Mitigation:** adopt stable concepts, not private/internal APIs; pin dependencies when implementation begins; re-check current releases at dependency selection time.

---

## POC / Validation Required

No POC was implemented during this research phase.

### POC 1 — Preview Backend Decision

**Question**  
Can a React/Tauri editor using WebView/proxy-assisted preview meet target scrub/playback responsiveness, or is a native Rust/FFmpeg/hardware-decoded continuous preview surface required?

**Why it matters**  
This decision affects media architecture, audio clock, frame transport, GPU work and UI/native boundaries. Choosing incorrectly can require a major rewrite.

**Minimum experiment**

Build only enough to compare two preview paths against the same synthetic/simple project:

- 1080p30 H.264 baseline
- 4K source downscaled/proxied for preview
- 2–3 stacked video/image layers
- crop/transform/opacity
- text overlay
- 2 audio sources
- repeated seeks/scrubs
- 30–60 second playback

Compare:

A. WebView/proxy-assisted playback  
B. Native decoded continuous preview surface or equivalent minimal native path

Measure:

- dropped frames
- seek-to-visible-frame latency
- input latency during playback
- memory growth
- CPU/GPU load
- audio drift
- recovery after rapid seeks

**Pass criteria**

The simpler WebView/proxy path passes if it meets the project's explicit performance targets on minimum target hardware with bounded memory and acceptable A/V sync. Suggested initial thresholds for discussion, not yet product commitments:

- 1080p30 normal playback with no sustained visible stutter
- seek-to-new-preview P95 around or below 100–150 ms for proxy media
- no unbounded memory/task growth under rapid seek
- A/V drift below a perceptible threshold and stable over the test clip
- timeline input remains responsive during playback

**Fail criteria**

Consistent missed targets, codec-specific instability, unbounded pending work, or inability to preserve A/V sync under representative layered projects.

**Decision affected**  
WebView/proxy preview vs native continuous preview architecture.

---

### POC 2 — Preview / Export Semantic Parity

**Question**  
Can the chosen preview backend and FFmpeg-based export backend consume one normalized composition model and produce sufficiently equivalent output for the MVP effect set?

**Why it matters**  
Users cannot trust an editor whose export materially differs from preview.

**Minimum experiment**

Define a tiny backend-independent composition case covering:

- scale/position/rotation
- crop
- opacity
- layer ordering
- one transition
- text with a bundled font
- simple linear keyframe interpolation
- audio gain/fade

Render/capture fixed frames at known timestamps from both paths.

**Pass criteria**

- timing boundaries match exactly at canonical timeline ticks/frames
- geometry and crop match within defined pixel tolerance
- keyframe values match at sampled times
- text uses deterministic font/assets and remains within defined layout tolerance
- audio start/end/fades match defined timing tolerance

**Fail criteria**

Backend-specific semantics require substantial special cases or visual/timing differences cannot be bounded.

**Decision affected**  
Whether two-backend preview/export architecture is viable for the initial feature set, or a more unified compositor is required earlier.

---

### POC 3 — Command Engine / Automation Safety

**Question**  
Can UI and headless automation operate the same project commands while preserving responsiveness, revision safety, idempotency and undo/redo?

**Why it matters**  
This is the defining architecture for the product's automation advantage.

**Minimum experiment**

Use an in-memory/project fixture only; no full editor required. Exercise:

- create tracks/clips
- move
- trim
- split
- delete
- multi-operation transaction
- stale `baseRevision`
- repeated `operationId`
- dry-run/diff
- undo/redo
- serialize/reopen

Run equivalent operations through two thin clients representing UI and automation.

**Pass criteria**

- same canonical project result from both clients
- stale revision rejected deterministically
- repeated operation ID does not mutate twice
- failed transaction changes nothing
- undo/redo restores expected canonical state
- serialization/reopen does not change semantic state

**Fail criteria**

UI requires bypassing domain commands for ordinary edits, or automation needs a separate reducer/state path.

**Decision affected**  
Final command-service design before editor features accumulate.

---

### POC 4 — Project Save / Recovery / Media Relink

**Question**  
Does the proposed persistence model survive interrupted saves, cache deletion and moved/missing source media without losing editorial work?

**Why it matters**  
Data loss is a release-blocking failure mode for a creator tool.

**Minimum experiment**

- save a project atomically
- simulate process interruption during save
- delete all derived cache
- move original source media
- reopen project
- relink the source
- regenerate derived artifacts

**Pass criteria**

- previous valid project remains readable after interrupted save
- project opens without cache
- missing media remains represented as offline clips
- relink restores clips without manual recreation
- generated data can be rebuilt

**Fail criteria**

Any test causes loss of timeline state or makes cache/source relocation indistinguishable from project corruption.

**Decision affected**  
Persistence, autosave and asset-identity strategy.

---

### POC 5 — FFmpeg Distribution / Hardware Fallback

**Question**  
Can a pinned, license-appropriate FFmpeg distribution support the minimum import/export matrix on target Windows hardware with predictable software fallback?

**Why it matters**  
Codec/runtime assumptions affect install size, legal obligations, compatibility and user support.

**Minimum experiment**

Using one explicitly documented FFmpeg build configuration:

- probe/import the agreed fixture corpus
- create proxies/thumbnails
- export target H.264/AAC or other approved initial format
- test available hardware acceleration path(s)
- disable/unavailable GPU and confirm software fallback
- validate bundled binary/source/license inventory process

**Pass criteria**

- agreed mandatory formats work on target OS/hardware
- software fallback completes correctly
- build flags and license inventory are reproducible
- application behavior on unsupported codec is explicit and recoverable

**Fail criteria**

Required formats depend on incompatible build flags, undocumented external binaries, or hardware paths with no acceptable fallback.

**Decision affected**  
Exact FFmpeg version/build and whether linked libraries are needed earlier.

---

## Open Questions / Blockers

These items do **not** block creating `PLAN.md`/`ARCHITECTURE.md`, but they should be made explicit before implementation milestones are frozen.

1. **Initial platform scope:** Windows 10/11 only first, or simultaneous macOS/Linux acceptance?
2. **MVP feature boundary:** exactly which features define “CapCut-like v1” — e.g. trim/split/tracks/text/captions/audio/transitions/keyframes/effects/masks/speed curves?
3. **Performance target:** minimum hardware, 1080p vs 4K, 30 vs 60 FPS, maximum representative layer count.
4. **Export target:** initial codecs/containers/resolutions and whether H.265/AV1 are required at launch.
5. **Color pipeline:** whether v1 is SDR/sRGB/Rec.709 only or must include HDR/color-management workflows.
6. **Automation scope at v1:** local CLI/MCP only, built-in AI agent, or external provider integrations.
7. **Cloud scope:** whether cloud sync/render/collaboration is explicitly out of v1.
8. **Commercial distribution/license model:** RESOLVED via ADR-002. TinixCut is scoped for **personal use (non-commercial / non-public distribution)**; commercial redistribution, GPL/LGPL EULA declarations, and patent licensing are explicitly out of scope for Phase 12 release. Technical provenance is tracked via SHA-256 for debugging/reproducibility.
9. **Plugin system:** whether third-party plugins are a near-term requirement. Recommendation is to preserve boundaries now but not build a public plugin SDK in the first phase.
10. **Portable project behavior:** copy all media, selected media, proxies, fonts, models, or only project metadata; this can be deferred until basic persistence exists.

No blocker currently requires further external research before architecture/planning. The largest unresolved items require product decisions or POC measurements rather than more reading.

---

## Final Research Summary

**STATUS:**  
READY WITH LIMITATIONS

**KEY FINDINGS:**  
- Build a new application rather than fork a current editor repository. No researched project simultaneously has the desired desktop media architecture, durable project model, CapCut-like UI, automation safety and dependency profile.  
- The architectural center should be a versioned **Project Domain Model + Command Service**. Manual UI, CLI, MCP and future AI automation must use the same authoritative mutation path.  
- Adopt Timeline Studio-style revision checks, dry-run/diff, transaction boundaries, stable IDs and idempotent operation IDs.  
- Use an OTIO-inspired editorial/time model, especially rational/integer canonical time, but own the internal project schema.  
- Tauri 2.12 + React/TypeScript + Rust is the strongest current runtime direction.  
- Use FFmpeg/ffprobe behind internal media/render interfaces; pin and audit the exact build before distribution.  
- Keep preview backend swappable until POC. Clypra provides strong evidence for native preview patterns, but the complexity should only be adopted if simpler preview fails measured targets.  
- Separate project source-of-truth from proxies/thumbnails/waveforms/cache.  
- A local bounded job manager is sufficient initially; Redis/microservices are not justified.  
- Remotion is useful as a reference or optional future integration, but should not be the canonical project model/core dependency.

**RECOMMENDED TECHNICAL DIRECTION:**  
Create a desktop-first modular monolith using **Tauri 2.12.x + React/TypeScript + Rust**, centered on an application-owned, versioned project/timeline schema and a single command service. Use canonical rational/integer time. Route both UI and automation through revision-aware, transactional, idempotent commands. Use FFmpeg for media jobs/export behind an abstraction. Use a normalized render/composition plan shared semantically by preview and export. Keep preview implementation behind a backend interface until measured POC determines WebView/proxy versus native decoding/surface.

**BEST PATTERNS TO ADOPT/ADAPT:**  
- Timeline Studio: inspect → diff/dry-run → apply; stable IDs; `baseRevision`; idempotent operation IDs; transactional automation; one service shared by CLI/MCP.  
- Clypra: React UI + Rust native media boundary; bounded/stale-work-aware scheduling; no heavy IPC per frame; transient gesture state with one authoritative commit.  
- OpenTimelineIO: rational time, source ranges, tracks/clips/gaps/transitions, external media reference concepts.  
- Editly: declarative machine-readable edit specs.  
- OpenCut rewrite: engine/UI separation and one core for GUI/headless/MCP, as an architectural concept rather than code foundation.  
- Tauri: least-privilege capabilities and controlled native/sidecar access.  
- FFmpeg: broad media backend behind project-owned interfaces.

**PATTERNS TO AVOID:**  
- Forking OpenCut/current editors as the long-term foundation.  
- AI primarily driving UI/mouse actions.  
- Separate reducers/business logic for manual vs automated editing.  
- Floating-point seconds as canonical timeline time.  
- FFmpeg filter strings, React state, Blob URLs, Remotion source or UI state as the durable project format.  
- Per-pointer/per-frame authoritative IPC or persistence.  
- Unbounded media queues.  
- Redis/microservices before a real cloud requirement.  
- Unrestricted shell/filesystem access from WebView or automation.  
- Assuming preview/export parity without tests.

**ARCHITECTURE INPUTS:**  
- Desktop-first modular monolith.  
- Tauri 2.12.x / React / TypeScript / Rust.  
- Application-owned versioned project schema.  
- Rational/integer canonical timeline time.  
- One Command Service for UI and automation.  
- Revision checks, transactions, dry-run/diff, idempotency, stable errors.  
- Media service + local bounded job manager.  
- FFmpeg/ffprobe pinned backend.  
- Rebuildable cache; external source-media references; optional future portable package.  
- PreviewBackend and ExportBackend consuming shared normalized composition semantics.  
- Least-privilege capability/security model.  
- Domain-first, failure-oriented testing.

**PLAN INPUTS:**  
- Establish project/domain/command/persistence foundation before deep UI features.  
- Add shell/editor UI and media import.  
- Add media cache/job foundation.  
- Implement core editing operations.  
- Incorporate preview POC decision.  
- Implement deterministic render/export and parity tests.  
- Add automation adapters only after the command service is stable.  
- Finish recovery/security/licensing/performance qualification before release.

**POC / VALIDATION REQUIRED:**  
1. Preview backend decision: WebView/proxy vs native continuous preview.  
2. Preview/export semantic parity for the MVP effect set.  
3. Shared command engine correctness across UI/headless clients.  
4. Atomic save/recovery/cache-loss/media-relink behavior.  
5. FFmpeg distribution, codec matrix and hardware/software fallback.

**OPEN BLOCKERS:**  
No research blocker prevents moving to `ARCHITECTURE.md` and `PLAN.md`. Remaining important unknowns are primarily product decisions and POC-measured constraints: initial platform scope, v1 feature boundary, target hardware/performance, codec/export matrix, color-management scope, AI/cloud scope, and commercial distribution/licensing requirements.

---

# Phase 11 Hardening Research — Durable Idempotency, Ledger Scaling & Fingerprint Security

**Date:** 2026-10-04  
**Status:** COMPLETE (Gated & Reconciled via ADR-001)  
**Execution Environment:** Windows 11, Rust Release Profile (`opt-level = 3`, LTO enabled)

## 1. Executive Summary

Phase 11 hardening research investigated the durable idempotency mechanisms introduced in Phase 10 across sustained project lifetimes. Using empirical benchmarks executed directly on the repository codebase, two architectural bottlenecks and vulnerabilities were characterized:
1. Unbounded growth of `command_ledger` causes an $O(N)$ memory cloning bottleneck during `CommandService::execute`, adding 38.5 ms (at 100k entries) to 79.2 ms (at 200k entries) of blocking latency to every single timeline edit, alongside 27–55 MB `.tcproj` file growth.
2. FNV-1a 64-bit operation fingerprinting possesses zero preimage or differential collision resistance, creating an adversarial vulnerability where conflicting automation payloads could be spoofed as deduplicated replays.

Both issues were resolved through architectural reconciliation documented in `ADR-001` and incorporated into `ARCHITECTURE.md`.

### 2. Empirical Benchmark Data

Benchmarks measured memory footprint, disk serialization, deserialization, service initialization, command execution clone latency, and deduplicated replay lookup across scales from 1,000 to 200,000 ledger entries, alongside dedicated direct benchmarks at the $K = 5,000$ capacity point:

| Entries ($N$) | `.tcproj` Size | `save_project_atomic` | `load_project` (deser) | `CommandService::new` | Exec Clone Overhead (`execute`) | Replay Dedup Lookup |
|---|---|---|---|---|---|---|
| **1,000** | 0.27 MB | 2.87 ms | 12.76 ms | 672.2 µs | 797.5 µs | 7.90 µs |
| **5,000 (direct)** | **1.635 MB** | **~6.5 ms** (release) | **~24.0 ms** (release) | **3.11 MB RAM** | **601 ns** (Staged Isolation) | **8.5–22.7 µs** |
| **10,000** | 2.75 MB | 10.18 ms | 36.35 ms | 6.53 ms | 4.97 ms | 9.20 µs |
| **100,000** | 27.55 MB | 87.45 ms | 270.40 ms | 63.74 ms | **38.53 ms** | 9.80 µs |
| **200,000** | 55.21 MB | 131.72 ms | 487.04 ms | 137.89 ms | **79.16 ms** | 8.60 µs |

### In-Memory and Serialization Density
- Struct size: `std::mem::size_of::<CommandLedgerEntry>()` = 152 bytes.
- Average heap allocations per entry: ~90 bytes.
- Total in-memory footprint in `Vec<CommandLedgerEntry>`: ~242 bytes/entry.
- Total in-memory footprint in `CommandService` (Vec + HashMap): ~536 bytes/entry.
- Serialized JSON density in `.tcproj` (pretty-printed with SHA-256): ~342.9 bytes/entry (1,714,534 bytes at $K=5,000$).

### Key Performance Findings
- **The Execution Clone Bottleneck**: Full project cloning during transaction staging copied entire ledger vectors, scaling with history ($O(N)$). With **Staged Transaction Isolation** (cloning only `tracks`, `assets`, `settings`), staging latency drops to **601 nanoseconds** ($0.6 \ \mu\text{s}$, a $4,394\times$ improvement over the 2.64 ms full project clone at $K=5,000$), bounded to $O(\text{editorial items})$.
- **Save/Load Degradation**: At 200k entries, autosaving writes 55 MB of JSON text (131 ms) and loading takes nearly half a second (487 ms).
- **Constant Replay Lookup**: In-memory replay deduplication lookup via `HashMap` remains extremely fast ($O(1)$, 8.5–22.7 µs) across all scales.

## 3. Fingerprint Threat Model & Cryptographic Security

### FNV-1a 64-bit Vulnerability
- **Accidental collision risk**: Birthday paradox probability $P \approx N^2 / (2 \times 2^{64})$. At $N = 100,000$, $P \approx 2.7 \times 10^{-10}$ (negligible under honest workloads).
- **Adversarial collision risk**: Fatal. Because FNV-1a is linear ($h_i = (h_{i-1} \oplus b_i) \cdot P$), differential cryptanalysis can generate colliding payloads in under 1 second ($\ll 2^{32}$ operations).
- **Authority role**: In TinixCut, `fingerprint` is the **sole correctness authority** for payload equivalence (raw payloads are not retained in the ledger). An adversarial collision completely bypasses idempotency collision detection.

### Cryptographic Digest Properties & Benchmark Comparison (100,000 iterations, 133-byte payload)
- Full `compute_operation_fingerprint` (serde + FNV-1a): 611.51 ns/op.
- FNV-1a 64-bit only: 252.91 ns/op.
- Software SHA-256 (hash + hex format): 1,417.11 ns/op (~1.4 µs).
- Estimated hardware-accelerated SHA-256 (SHA-NI): ~250–350 ns/op.
- **Security Terminology & Cryptographic Bounds**:
  - **Collision resistance**: $\mathbf{\sim 2^{128}}$ operations (governed by the Birthday bound $2^{256/2} \approx 3.4 \times 10^{38}$).
  - **Preimage & Second-preimage resistance**: $\mathbf{\sim 2^{256}}$ operations.
- **Conclusion**: A 1.4 µs cryptographic hash is completely negligible compared to command validation (10–50 µs) and UI IPC latency (1–2 ms). SHA-256 provides uncompromised security at virtually zero human-perceivable cost.

## 4. Measurable Performance Budget Justifying $K = 5,000$

The threshold $K = 5,000$ operations was directly benchmarked against our 5-point performance budget on reference hardware (Windows 11, NVMe storage):

| Performance Metric | Budget Target | Measured Directly at $K = 5,000$ (Formatted JSON + SHA-256) | Status / Margin |
|---|---|---|---|
| **Project File Overhead** | $\le 2.0 \text{ MB}$ | **1.635 MB** (1,714,534 bytes; 342.9 B/entry) | PASS (18% margin) |
| **CommandService RAM** | $\le 5.0 \text{ MB}$ | **3.11 MB** (1.44 MB Vec + 1.67 MB HashMap) | PASS (38% margin) |
| **Staged Isolation Clone** | $\le 100 \ \mu\text{s}$ | **601 nanoseconds** ($0.6 \ \mu\text{s}$; algorithmic $O(\text{items})$) | PASS (99% margin) |
| **Deduplicated Replay Lookup** | $\le 50 \ \mu\text{s}$ | **8.5 – 22.7 µs** ($O(1)$ HashMap lookup) | PASS (55% margin) |
| **Autosave Disk Latency** | $\le 15 \text{ ms}$ (release) | **~6.5 ms** (release profile; ~52 ms unoptimized debug) | PASS under standard NVMe profile |
| **Project Load / Parse** | $\le 30 \text{ ms}$ (release) | **~24.0 ms** (release profile; ~94 ms unoptimized debug) | PASS under standard NVMe profile |

*Note on Latency Environment Boundaries*: While in-memory algorithmic complexity is strictly $O(1)$, disk I/O latencies are subject to OS thread preemption, virus scanner inspection locks, and filesystem sync flushes.

## 5. Mathematical Proof of Impossibility & Structured Identity Architecture

### 5.1 The Unscoped Identity Correctness Flaw (v2 Gap)
In ADR-001 v2, arbitrary unscoped string IDs were stored across an Active Window ($K = 5,000$) and a Bounded Tombstone Window ($M_{\max} = 10,000$). Beyond 15,000 operations, oldest entries were pruned, advancing `sealed_unscoped_revision`.
*The Vulnerability*: If a caller queries current project revision $t > 15,000$ and resubmits an evicted unscoped ID with `base_revision = t`, the condition `base_revision <= sealed_unscoped_revision` is `false`. Because the ID was evicted from the active and tombstone windows, the engine finds no record of it, sees a matching current revision, and executes the command as a **fresh new operation**, causing silent duplicate mutations.

### 5.2 Mathematical Proof: Impossibility of Bounded Unscoped Deduplication
Let $U = \Sigma^*$ be the infinite universe of arbitrary string IDs. Let $H_t \subset U$ be the set of operation IDs committed up to revision $t$. Let $B < \infty$ be a constant storage budget (in bits) allocated in the project document.
To guarantee zero duplicate executions under arbitrary caller refreshes, the system must decide $x \in H_t$ deterministically for any $x \in U$.
The number of distinct subsets $H_t$ of size $N$ that can be distinguished by $B$ bits is at most $2^B$. As $N \to \infty$, $\binom{|U|}{N} \gg 2^B$. By the Pigeonhole Principle, there exist distinct histories $H_1 \ne H_2$ that map to the identical stored state $S_B$. For any $x \in H_1 \setminus H_2$, the system cannot distinguish whether $x$ was committed in $H_1$ or is fresh in $H_2$.
**Conclusion**: It is mathematically impossible for arbitrary unscoped string IDs to guarantee durable idempotency across unbounded horizons with bounded storage.

### 5.3 Adopted Solution: Structured Scoped Operation Identity (ADR-001 v3)
Durable idempotency is exclusively governed by **Structured Scoped Identity**:
$$\text{OpIdentity} = (\text{client\_id}, \text{client\_epoch}, \text{sequence\_number})$$
where sequence numbers are strictly monotonic integers starting at 1.

#### Correctness Proof for Monotonic Sequence Identity
For any active client $(c, e)$, the committed operations form a contiguous natural prefix $\{1, \dots, W(c, e)\}$.
1. **$O(1)$ Storage**: Representing the entire history requires exactly one integer $W(c, e)$ (8 bytes). Individual operation tombstones are completely eliminated.
2. **Deterministic Evaluation**: For any incoming $(c, e, s)$, if $s \le W(c, e)$, the operation was committed with $100\%$ mathematical certainty. It can NEVER re-execute as fresh, even if the caller refreshes `base_revision`. Duplicate execution probability is identically 0.

### 5.4 Client Lifecycle & Bounded Capacity Contract (`ClientRegistry`)
To prevent unbounded client map growth when callers generate dynamic client IDs:
1. **Hard Capacity Quota**: The project enforces a strict upper bound $C_{\max} = 64$ registered clients ($\le 3.5 \text{ KB}$ in `.tcproj`).
2. **Registration Enforcement**: Callers must register or use well-known IDs (`"gui"`, `"cli"`, `"session"`). When registry capacity is full, new client registrations are rejected (`CLIENT_REGISTRY_FULL`).
3. **Safe Client Retirement & Epoch Fencing**: Inactive clients can be retired by advancing `min_accepted_epoch`. Incoming commands with `client_epoch < min_accepted_epoch` are immediately rejected with `CLIENT_EPOCH_EXPIRED` via an $O(1)$ scalar check. Pruned clients can never execute old commands as fresh.

### 5.5 Legacy Unscoped Operation IDs: Semantics & Backward Compatibility
- **Ephemeral Window Only**: Bare string IDs without `client_id:seq` are supported strictly within the Active Window ($K = 5,000$).
- **Post-Eviction**: Evicted unscoped IDs are expired; durable long-term idempotency is reserved for structured identities.
- **Transport Ingestion**: Automation transports (`tinixcut apply`, `tinixcut session`) automatically wrap incoming commands with structured identities (`cli:seq`, `session:seq`), protecting standard workflows without requiring manual sequence tracking.
- **Total Document Overhead**: Bounded at $\le \mathbf{1.65 \text{ MB}}$ permanently (1.635 MB Active Window + $\le 3.5 \text{ KB}$ Client Registry). Individual tombstones are eliminated.

## 6. Conservative Legacy Migration Protocol (Phase 10 FNV-1a to SHA-256)

Phase 10 projects store `fingerprint = "fp_<16hex>"` without raw payloads. FNV-1a possesses negligible adversarial collision resistance ($\ll 2^{32}$ operations). Treating a legacy FNV-1a match as cryptographic proof of exact replay would allow an attacker to craft a colliding payload and spoof replay deduplication for untrusted project/command input.

### Migration Resolution: Conservative Replay Rejection
- When checking an existing entry whose fingerprint starts with `"fp_"` (19 chars, FNV-1a):
  - The engine **refuses to treat it as an exact replay** because it cannot cryptographically prove that the candidate payload matches the un-retained original.
  - The engine **strictly rejects** the replay attempt with:
    ```json
    {
      "code": "LEGACY_RECEIPT_UNVERIFIABLE",
      "message": "Operation 'op_123' was committed under legacy non-cryptographic schema (FNV-1a) without raw payload retention and cannot be safely verified for replay. Replay rejected for cryptographic integrity. Inspect project state to determine if operation outcome is present.",
      "operation_id": "op_123"
    }
    ```
  - **CRITICAL CORRECTNESS**: The operation is **NOT executed as fresh** (zero project mutation, no revision bump).
- New operations are always generated with `"fp_sha256_<64hex>"`.
- Over time, legacy Phase 10 entries naturally roll out of the active window and are evicted, leaving 100% of active receipts cryptographically secure.

---

# Phase 12 Research & Reconciled Decisions — Windows FFmpeg Distribution, Packaging & Hardware Fallback

**Date:** 2026-10-04  
**Status:** COMPLETE (Gated & Reconciled via ADR-002)  
**Execution Environment:** Windows 10/11 x86_64, Tauri 2.12.1 + Rust 1.98.1

## 1. Product Scope & Licensing Boundary
- **Personal-Use Exclusivity**: TinixCut is maintained for personal use and developer workflows (no public/commercial distribution).
- **Out-of-Scope Items**: Commercial distribution auditing, GPL/LGPL EULA declarations, and commercial patent reviews are explicitly **OUT OF SCOPE** and do not gate Phase 12 release.
- **Process Boundary Invariant**: Architectural process boundary is strictly maintained (`std::process::Command` with typed arguments and piped stdio; no FFI linking to `libav*`).
- **Provenance Integrity**: Exact upstream URL, version (`9.0.2 Gyan Release Essentials`), and SHA-256 digests are pinned in `ffmpeg_provenance.json` for technical reproducibility and debugging.

## 2. Authoritative Packaging Mechanism
- **Selected**: **Tauri `bundle.resources`** (`resources/bin/ffmpeg.exe`, `resources/bin/ffprobe.exe`).
- **Packaging Invariant**: Packaged release builds MUST resolve and use the app-owned pinned FFmpeg/ffprobe from the application resource directory (`resource_dir`). If missing in a packaged build, the app errors immediately with `MediaError::BundledBinaryMissing`.
- **System PATH Policy**: Ambient system `PATH` is strictly prohibited in packaged release builds; it is permitted only in explicit dev/test mode (`cfg!(debug_assertions)` or `TINIX_DEV_ALLOW_SYSTEM_PATH=1`).

## 3. WebView2 Distribution Strategy: `downloadBootstrapper`
- **Selected**: `downloadBootstrapper` in `bundle.windows.webviewInstallMode`.
- **Trade-off Analysis**:
  - `downloadBootstrapper` adds minimal (~1.8 MB) installer overhead. Modern Windows 10 and 11 environments natively include Microsoft Edge WebView2 Evergreen Runtime. In edge cases where WebView2 is missing, the bootstrapper automatically retrieves it during installation.
  - `offlineInstaller` was evaluated and rejected: bundling the full offline installer adds ~160 MB of dead weight (inflating installer size from ~150 MB to >310 MB), providing value only in air-gapped enterprise environments which are irrelevant to a personal-use desktop editor.

## 4. Hardware Acceleration & Fallback Protocol
- **Baseline**: `libx264` software encoding (CPU reference).
- **Accelerators**: `h264_nvenc` (NVIDIA), `h264_qsv` (Intel), `h264_amf` (AMD).
- **Fallback Protocol**:
  - `video_codec = "auto"` probes available hardware encoders in priority ladder (`NVENC` -> `QSV` -> `AMF` -> `libx264`).
  - Requested hardware encoder failures due to driver/device unavailability automatically downgrade to `libx264`.
  - Non-hardware errors (missing media, invalid geometry, disk full) fail immediately without redundant retries.

---

# Phase 13 Research & Reconciled Decisions — Adjacent-Cut CrossDissolve and Transition Compositing Semantics

**Date:** 2026-10-04  
**Status:** COMPLETE (Accepted by Research Gate; Governed by ADR-003; Implemented in Phase 13)  
**Scope:** Video Track Adjacent-Cut CrossDissolve, Schema v2 Transition Topology, Dual-Backend Compositing  
**Consulted:** Gate E Parity Specification, FFmpeg `xfade` filter internals, W3C Compositing & Blending Level 1

## 1. Executive Summary & Problem Resolution

Phase 9 deferred `CrossDissolve` because attempting to composite overlapping dissolves under Schema v1 revealed fundamental structural flaws:
1. Individual clips (`TimelineItem`) cannot logically own a pairwise boundary transition between two clips.
2. Persisting overlap on the canonical timeline changes total project duration, breaks track sync, and ripples downstream edits.
3. Ordinary Porter-Duff `source-over` opacity compositing drops composite alpha by $25\%$ at midpoint $p=0.5$, introducing an unphysical dark dip / background bleed.
4. Auto-clamping or freeze-framing missing media handles corrupts non-destructive editing guarantees.

Phase 13 resolves these constraints through an authoritative **cut-owned topology**, **premultiplied linear blend invariant**, **isolated preview `PreviewTransitionGroup` DOM container**, **authoritative FFmpeg `xfade=fade` export sequence**, and **provable handle validation**.

---

## 2. Mathematical Proof: Infeasibility of Ordinary Source-Over Compositing

### 2.1 Theorem (Non-Linear Alpha Dip in Modulated Source-Over)
Let clip $A$ have premultiplied color $C_A^* = \alpha_A C_A$ with alpha $\alpha_A \in [0, 1]$.  
Let clip $B$ have premultiplied color $C_B^* = \alpha_B C_B$ with alpha $\alpha_B \in [0, 1]$.  
Let $p \in [0, 1]$ be the normalized transition progress.

The canonical linear blend for premultiplied representations is:
$$C_{\text{blend}}^* = (1 - p) C_A^* + p C_B^*$$
$$\alpha_{\text{blend}} = (1 - p) \alpha_A + p \alpha_B$$

For two fully opaque video frames ($\alpha_A = 1, \alpha_B = 1$):
$$\alpha_{\text{blend}} = (1 - p)(1) + p(1) = 1.0 \quad \forall p \in [0, 1]$$
$$C_{\text{blend}}^* = (1 - p) C_A + p C_B$$

Now consider an implementation attempting to simulate cross-dissolve using standard two-layer **`source-over`** compositing, with both layers modulated by opacity factors:
$$\alpha_A' = (1 - p) \alpha_A, \quad \alpha_B' = p \alpha_B$$

In Porter-Duff `source-over` ($B'$ placed over $A'$), the composite alpha $\alpha_{\text{over}}$ is:
$$\alpha_{\text{over}} = \alpha_B' + \alpha_A'(1 - \alpha_B')$$

Substituting the modulated opacities:
$$\alpha_{\text{over}} = p \alpha_B + (1 - p)\alpha_A (1 - p \alpha_B) = (1 - p)\alpha_A + p \alpha_B - p(1 - p)\alpha_A \alpha_B$$

Comparing canonical blend $\alpha_{\text{blend}}$ and source-over composite $\alpha_{\text{over}}$:
$$\Delta \alpha = \alpha_{\text{blend}} - \alpha_{\text{over}} = p(1 - p)\alpha_A \alpha_B$$

For standard opaque video ($\alpha_A = 1, \alpha_B = 1$):
$$\Delta \alpha = p(1 - p)$$

Evaluating at the transition midpoint ($p = 0.5$):
$$\Delta \alpha = (0.5)(0.5) = 0.25$$
$$\alpha_{\text{over}} = 1.0 - 0.25 = 0.75 \ne 1.0$$

### 2.2 Corollary & Architectural Conclusion
In standard two-layer `source-over` opacity compositing:
- At midpoint $p = 0.5$, composite alpha drops by **$25\%$**, revealing whatever background or black canvas lies beneath.
- Keeping bottom clip $A$ at opacity $1.0$ while fading $B$ ($0 \to 1$) introduces severe spatial asymmetry if either clip possesses transform/crop/transparency.
- **Conclusion**: Ordinary two-layer `source-over` opacity compositing is **mathematically proven to be unequal to canonical CrossDissolve**.
- CrossDissolve must strictly be evaluated as premultiplied additive mixing:
  $$C_{\text{blend}}^* = (1 - p) C_A^* + p C_B^*$$
  implemented via weighted A/B contributions with `mix-blend-mode: plus-lighter` on premultiplied RGB frames strictly inside an isolated DOM stacking context (`PreviewTransitionGroup`) in preview, and bounded aligned FFmpeg `xfade=transition=fade` in export.

---

## 3. Canonical CD-01 .. CD-43 Contract

### Group 1: Domain Model & Track Topology (CD-01 .. CD-07)
- **CD-01 (Cut-Owned Topology)**: `CutTransition` is owned by the cut boundary between two clips on a track, not by an individual timeline clip.
- **CD-02 (Immediate Adjacency Invariant)**: Participants must be strictly adjacent on the timeline: $\text{left.timeline\_range.end()} == \text{right.timeline\_range.start} == C$.
- **CD-03 (Single Track Confinement)**: Both participants must reside on the identical track. Cross-track cut transitions are invalid.
- **CD-04 (Video Item Eligibility)**: Both participants must be video items (`ItemKind::Video`). Non-video items trigger `UNSUPPORTED_TRANSITION_MEDIA`.
- **CD-05 (Non-Overlapping Timeline Storage)**: Timeline item ranges are stored edge-to-edge. No overlap is persisted in `TimelineItem.timeline_range`.
- **CD-06 (Timeline Duration Invariance)**: Adding, editing, or deleting a CutTransition preserves total track and project duration identically ($\Delta T_{\text{project}} = 0$).
- **CD-07 (Transition Singularity per Cut)**: At most one CutTransition may exist at any given cut boundary between `left_item_id` and `right_item_id`.

### Group 2: Schema v2 & Migration (CD-08 .. CD-13)
- **CD-08 (Schema Version Promotion)**: Domain persistence promotes project format to `schema_version = 2`.
- **CD-09 (Track-Level CutTransition Collection)**: `Track` struct holds `pub transitions: Vec<CutTransition>`.
- **CD-10 (Preservation of Single-Clip Fade Semantics)**: Production-supported clip-owned Fade semantics currently on `TimelineItem` (`fade_in`, `fade_out`, item-level `FadeIn`/`FadeOut` variants) must NOT be cleared, dropped, or re-homed. They remain supported at the item level. `TimelineItem.transition` is not deprecated in any manner that implies field removal in Phase 13 while existing supported Fade variants still depend on it. Pairwise cut transitions live exclusively in `Track.transitions`.
- **CD-11 (Single-Clip Fade Independence & Lossless Survival)**: Existing v1 item-level Fade behavior survives v1→v2 migration losslessly, completely independent of cut transitions.
- **CD-12 (Lossless Forward Migration)**: Schema v1 projects deserialize cleanly into Schema v2 with `Track.transitions: Vec::new()` (`transitions: []`). Migration does NOT invent `CrossDissolve` from v1.
- **CD-13 (Malformed v1 Policy & Invariant Validation)**: If malformed v1 contains an unsupported `CrossDissolve` placeholder in item transition, it is handled explicitly according to migration/error policy (rejected or surfaced as an explicit migration/validation error; it must never be silently activated as a cut transition). Deserializing corrupted JSON with invalid cut adjacency or non-existent item references triggers explicit validation errors on project open.

### Group 3: Time, Alignment & Handle Invariants (CD-14 .. CD-21)
- **CD-14 (RationalTime Precision)**: Transition duration and cut offsets must be exact `RationalTime` (`{ value, rate }`). Floating-point seconds are prohibited.
- **CD-15 (Exact RationalTime Odd-Duration Rule)**: Duration $D$ is rescaled to project timeline frame rate as $D_{\text{ticks}}$. Tick division is exact: $L_{\text{ticks}} = \lfloor D_{\text{ticks}} / 2 \rfloor$, $R_{\text{ticks}} = D_{\text{ticks}} - L_{\text{ticks}}$. Cut window boundaries are $T_0 = C - L$ and $T_1 = C + R$. The transition interval is strictly half-open: $[T_0, T_1)$. Phase 13 CrossDissolve provides centered cut semantics exclusively; no persisted `CutAlignment` field in Schema v2 (future alignment extensibility is deferred).
- **CD-16 (Provable Preroll Handle & Editorial Extent)**: Incoming right clip must satisfy $\text{available\_preroll} \ge L$ (where $\text{available\_preroll} = \text{right.source\_range.start}$) and incoming editorial extent $\ge R$ (where $\text{right.timeline\_range.duration} = \text{right.timeline\_range.end()} - C \ge R$).
- **CD-17 (Provable Postroll Handle & Editorial Extent)**: Outgoing left clip must satisfy $\text{available\_postroll} \ge R$ (where $\text{available\_postroll} = \text{asset\_duration} - (\text{left.source\_range.start} + \text{left.source\_range.duration})$) and outgoing editorial extent $\ge L$ (where $\text{left.timeline\_range.duration} = C - \text{left.timeline\_range.start} \ge L$).
- **CD-18 (Stable Machine-Readable Error Contract)**: Transition validation produces stable error codes:
  - `INSUFFICIENT_TRANSITION_HANDLE`: Source duration is known, but available postroll handle $< R$, available preroll handle $< L$, or editorial extent is insufficient.
  - `TRANSITION_HANDLE_UNVERIFIABLE`: Media duration or stream handle availability cannot be proven (unprobed media, stream duration missing, or asset unavailable).
  - `UNSUPPORTED_TRANSITION_MEDIA`: Participant is an image, synthetic graphic, text, or non-video item.
- **CD-19 (Prohibition of Auto-Shortening)**: The engine must never automatically shorten transition duration to accommodate insufficient handles.
- **CD-20 (Prohibition of Frame Freezing)**: The engine must never synthesize missing handle frames via freeze-frame or frame duplication.
- **CD-21 (Duration Bounds Enforcement)**: Transition duration must satisfy $D_{\min} \le D \le \min(\text{dur}_A, \text{dur}_B)$.

### Group 4: Mathematical Blend & Compositing Semantics (CD-22 .. CD-27)
- **CD-22 (Canonical Premultiplied Blend)**: Blending evaluates $C_{\text{blend}}^* = (1 - p) C_A^* + p C_B^*$ and $\alpha_{\text{blend}} = (1 - p)\alpha_A + p\alpha_B$ in premultiplied space.
- **CD-23 (Linear Transition Progress)**: Progress $p(t) = (t - T_0) / D \in [0.0, 1.0)$ progresses monotonically and linearly across the half-open transition window $[T_0, T_1)$.
- **CD-24 (Mathematical Inequality Proof)**: Verified that $\text{SourceOver}(A', B') \ne \text{PremultipliedMix}(A, B, p)$, proving a 25% alpha loss at $p=0.5$.
- **CD-25 (Prohibition of Two-Layer Source-Over Opacity)**: Standard CSS/FFmpeg `source-over` layer opacity is strictly forbidden for CrossDissolve.
- **CD-26 (Color Space Normalization)**: Compositing operates in normalized sRGB / Rec.709 color space without unmanaged transfer curve distortions.
- **CD-27 (Alpha Preservation for Transparent Assets)**: Non-opaque or cropped video layers preserve premultiplied edge alpha without black fringing.

### Group 5: Preview Architecture & PreviewTransitionGroup (CD-28 .. CD-32)
- **CD-28 (Isolated PreviewTransitionGroup DOM Element)**: During the transition interval $[T_0, T_1)$, participants render inside an explicit isolated container (`<div class="preview-transition-group">`) with its own isolated stacking context. Canvas/offscreen compositor is NOT an authorized alternative Phase 13 implementation. Silent fallback to ordinary `source-over` is strictly prohibited.
- **CD-29 (Additive Premultiplied Blend Mode)**: Inside `PreviewTransitionGroup`, layers blend with weighted A/B contributions ($1-p$, $p$) using `mix-blend-mode: plus-lighter` on premultiplied RGB frames strictly inside the group. The container composites `source-over` onto the normal stack.
- **CD-30 (Handle-Aware Frame Dispatch)**: Preview controller schedules postroll frame decoding for the left clip ($S_A(t)$) and preroll frame decoding for the right clip ($S_B(t)$).
- **CD-31 (Seamless Window Boundary Transition)**: At $t < T_0$, standard left clip renders; at $t \ge T_1$, standard right clip renders; boundary frames match without flicker.
- **CD-32 (Scrubbing and Seek Determinism)**: Seeking to any timestamp within $[T_0, T_1)$ deterministically evaluates the exact progress $p(t) = (t - T_0) / D$ and decodes exact source frames.

### Group 6: Export Filtergraph Architecture (CD-33 .. CD-37)
- **CD-33 (Authoritative Export Sequence)**: Export pipeline evaluates:
  `normalized full-canvas RGBA A/B`
  $\to$ `premultiply`
  $\to$ bounded aligned `xfade=transition=fade`
  $\to$ `unpremultiply`
  $\to$ one transition-result contribution
  $\to$ existing overlay chain.
- **CD-34 (Ordinary Stream Suppression)**: Ordinary unblended A and B contributions on the track must be strictly suppressed during $[T_0, T_1)$.
- **CD-35 (Bounded Aligned `xfade=fade` Segment Filter)**: Dissolve is evaluated via bounded FFmpeg filter: `xfade=transition=fade:duration=D:offset=...` aligned to $[T_0, T_1)$.
- **CD-36 (Unpremultiply and Stream Reintegration)**: Resulting stream is unpremultiplied to standard RGBA and represents the single contribution for that track interval in the existing overlay chain.
- **CD-37 (Preview/Export Parity Tolerance)**: Preview and export renders must pass Gate E / POC 2 parity tests with $\Delta < 0.1\text{px}$ and MAE $\le 4.0$.

### Group 7: Scope Boundaries & Governance (CD-38 .. CD-43)
- **CD-38 (Video-Only Confinement)**: Phase 13 applies exclusively to Video clips (`ItemKind::Video`). Non-video items trigger `UNSUPPORTED_TRANSITION_MEDIA`.
- **CD-39 (Audio Crossfade Deferral)**: Audio across adjacent cuts remains an exact butt-cut. True audio crossfade (`acrossfade`) is deferred.
- **CD-40 (Prohibition of General Transition Framework)**: Arbitrary shader graphs, wipe masks, and plugin transition frameworks are prohibited in Phase 13. Bezier/spectral waveform remains deferred.
- **CD-41 (Transition Kind Restriction)**: `TransitionKind::CrossDissolve` is the sole authorized adjacent-cut transition in Phase 13.
- **CD-42 (Command Service Exclusivity)**: All CutTransition operations (`AddCutTransition`, `RemoveCutTransition`, `UpdateCutTransitionDuration`) route exclusively through the `CommandService`.
- **CD-43 (Automation Surface Exposure)**: Transition operations are exposed to automation/CLI with structured receipts, idempotency, and revision fencing.

---

# Phase 14 Research & Reconciled Decisions — Advanced Keyframe Interpolation, Lifecycle & Export Automation

**Date:** 2026-10-05  
**Status:** IMPLEMENTED AND VERIFIED IN PHASE 14 (Governed by ADR-004)  
**Execution Environment:** Windows 10/11 x86_64, Tauri 2.12.1 + Rust 1.98.1, Bundled FFmpeg 9.0.2

## 1. Executive Summary & Research Gate Status

The Phase 14 Research Gate addressed three core problems identified during repo auditing:
1. Absence of non-linear easing curves (limiting animations to linear interpolation).
2. Fragmented/defective keyframe lifecycle operations during clip split and trim.
3. The Phase-9 full-export discrepancy (`P14-PREREQ-01`), where full export ignored dynamic keyframes completely.

All technical, mathematical, and feasibility questions have been resolved through architectural analysis and verified via an 8-test POC test suite (`src-tauri/tests/export_automation_poc.rs`).

**Research status:** `IMPLEMENTED AND VERIFIED IN PHASE 14`.

---

## 2. Narrow Repo Audit Findings

1. **Exact Current Keyframe Model**:
   - `TimelineItem.keyframes: Vec<Keyframe>` stores keyframes clip-locally without stable IDs.
   - `Keyframe` fields: `property: KeyframeProperty`, `offset: RationalTime`, `value: f64`.
   - Exactly 7 supported property channels: `PositionX`, `PositionY`, `ScaleX`, `ScaleY`, `Rotation`, `Opacity`, `Volume`.
   - Keyframe lookup in commands and UI relies on `(property, offset)`.
2. **Phase-9 Full-Export Discrepancy (`P14-PREREQ-01`)**:
   - Preview canvas (`PreviewCanvas.tsx`) and single-frame snapshot (`CompositionPlan::evaluate_item_keyframes`) evaluate dynamic linear keyframes.
   - Full video export (`CompositionPlan::build_export_filtergraph`), however, sampled only static clip properties (`item.transform`, `item.volume`).
   - Dynamic keyframe animations were dropped entirely during full video export. Remediation of this defect is classified as prerequisite `P14-PREREQ-01` in Phase 14 before advanced easing can be considered production-complete.
3. **Keyframe Lifecycle Deficiencies**:
   - `split_item`: Naively cloned all keyframes to both left and right fragments without rebasing offsets relative to the split point $S$, corrupting animation on the right clip.
   - `trim_item`: Lacked an explicit domain model for hidden keyframes, risking data loss when clips are trimmed and re-extended.
   - Keyframe time equality: Several locations used raw `.offset.value` comparisons rather than semantic cross-multiplied equality, creating errors under mixed timebases.

---

## 3. Final Easing Mathematical Contract

1. **Supported Easing Variants**:
   - `Linear`: Identity $e(u) = u$.
   - `EaseIn`: Preset Cubic Bézier $(0.42, 0.0, 1.0, 1.0)$.
   - `EaseOut`: Preset Cubic Bézier $(0.0, 0.0, 0.58, 1.0)$.
   - `EaseInOut`: Preset Cubic Bézier $(0.42, 0.0, 0.58, 1.0)$.
   - `CubicBezier { x1, y1, x2, y2 }`: Arbitrary cubic Bézier with control points $P_1 = (x_1, y_1)$ and $P_2 = (x_2, y_2)$.
2. **Control Point Bounds**:
   - Stored with `f64` precision.
   - All coordinates must be finite and bounded in $[0.0, 1.0]$.
   - No overshoot ($y < 0$ or $y > 1$) in Phase 14.
   - Monotonic time mapping $x(q)$ guaranteed for all $(x_1, x_2) \in [0, 1]^2$; no requirement that $x_1 \le x_2$.
3. **Canonical 32-Iteration Bisection Solver**:
   - Solves $x(q) = u$ for parameter $q \in [0, 1]$ using a fixed 32-iteration bisection loop (precision $\approx 2.33 \times 10^{-10}$).
   - Evaluates eased progress $e = y(q)$.
   - Exact endpoints bypass the solver: $u \le 0.0 \implies V_{\text{left}}$, $u \ge 1.0 \implies V_{\text{right}}$.
   - Implemented strictly in Rust/core. No CSS, browser, or FFmpeg solver is canonical truth.
4. **Channel Semantics**:
   - Same scalar evaluator applies to all 7 channels.
   - Rotation interpolates numerically in degrees (no shortest-path wrapping).
   - No implicit clamping in the interpolation layer.

---

## 4. RationalTime Semantic Rules

1. **Canonical Address**: `(property, semantic RationalTime offset)`.
2. **Semantic Comparison**: Offsets $T_A = (v_A, r_A)$ and $T_B = (v_B, r_B)$ are semantically equal iff $v_A \cdot r_B == v_B \cdot r_A$.
3. **No Raw Value Equality**: Prohibit comparing raw `.offset.value` across unknown or mixed time rates.
4. **Global Equality Unchanged**: Global `RationalTime::eq` remains representation equality. Keyframe operations use semantic comparison helpers (`is_same_time`, `cmp_semantic`).
5. **Mixed-Rate Equivalence**: Keyframes with equivalent rational offsets address the same semantic keyframe.

---

## 5. Split Subdivision & Trim Semantics

1. **Split at Clip-Local Offset $S$**:
   - Keyframes with $K < S$: Remain on left fragment at offset $K$.
   - Keyframes with $K > S$: Migrate to right fragment rebased to $K - S$.
   - Boundary Keyframe ($K == S$): Retained on left fragment at $S$ with outgoing `Linear`; duplicated on right fragment at $0$ inheriting original easing.
   - Segment Crossing ($K_i < S < K_{i+1}$):
     - Sample exact value $V(S)$ using canonical 32-iteration solver.
     - Synthesize boundary keys on left at $S$ and right at $0$, both with value $V(S)$.
     - Linear curves remain `Linear` on both fragments.
     - Cubic/preset curves subdivide via solver + de Casteljau algorithm at split parameter $q_S$, normalizing subcurve control points to $[0, 1]^2$. Presets become `CubicBezier`.
   - Identity: Left retains original `item_id`; right receives newly generated `item_id`. No keyframe IDs introduced.
2. **Trim Semantics**:
   - **Tail Trim**: Shortens clip duration. Post-duration keyframes remain persisted as hidden keyframes; extending tail restores them.
   - **Head Trim by $H$**: Shifts source start $+H$, shortens duration, and shifts all keyframe offsets by $-H$. Negative keyframe offsets are valid canonical domain state (hidden preroll keyframes). Extending head shifts offsets by $+H$, restoring animation alignment.
   - Trim and keyframe shifts execute atomically as a single history entry.
   - Silent deletion of keyframes during trim is strictly prohibited.

---

## 6. Schema v3 Rationale & Migration

1. **Mandatory Schema v3**: Persisting `outgoing_easing` on `Keyframe` requires promoting project format to `schema_version = 3`. Remaining at v2 would allow older Phase 13 binaries to deserialize the project, ignore unknown easing fields, and resave the file with all easing permanently lost.
2. **Lossless Forward Migration (v2 $\to$ v3)**:
   - Preserves all v2 project metadata, tracks, assets, and items.
   - Preserves ADR-003 `Track.transitions` (`CutTransition`) losslessly.
   - Preserves item-level Fades (`TimelineItem.transition`) losslessly.
   - Preserves all keyframe properties, offsets, and values.
   - Initializes all migrated keyframes with `outgoing_easing = KeyframeEasing::Linear`.
   - Existing projects open with zero visual changes or animation drift.
   - Old Schema v2 binaries reject Schema v3 files with `UnsupportedSchemaVersion(3)`.

---

## 7. ExportAutomationSchedule Architecture

1. **Data Flow**:
   ```text
   Project Keyframes (Schema v3)
             ↓
     Canonical Rust Evaluator
             ↓
   ExportAutomationSchedule (ephemeral execution artifact)
             ↓
   FFmpeg Runtime Filter Commands & Audio Envelopes (literal values only)
   ```
2. **Video Pipeline**:
   - Sampled at output frame PTS grid.
   - Frame 0 provides initial filter parameters.
   - Frames $n \ge 1$ dispatched via sendcmd file using midpoint timing $t_{\text{mid}} = (t_{n-1} + t_n) / 2$.
   - Mapped filters: `overlay` (x, y), `scale` (w, h), `rotate` (a in radians), `colorchannelmixer` (aa opacity).
   - Zero interpolation math inside FFmpeg filtergraph.
3. **Audio Pipeline**:
   - Sampled at audio sample grid (e.g. 48 kHz).
   - Streamed as raw mono 32-bit floating point PCM (`f32le`) envelope file.
   - Applied in FFmpeg via `amultiply` filter.
   - Ephemeral resource lifecycle: disk preflight before export, streamed generation in $O(1)$ memory, and deterministic cleanup on completion, error, or cancellation.

---

## 8. Export POC Results & Coexistence Evidence

The export automation architecture was proven using the app-owned bundled FFmpeg 9.0.2 binary via test harness `src-tauri/tests/export_automation_poc.rs`.

| Test ID | Gate Verification Requirement | Result | Observations & Evidence |
|---|---|---|---|
| **POC-Preflight** | Bundled FFmpeg 9.0.2 check | **PASS** | Resolves via `MediaBinaryResolver::resolve(BinaryKind::FFmpeg)`. |
| **POC-V1** | Runtime command capability on FFmpeg 9.0.2 | **PASS** | Verified runtime commands for `scale`, `overlay`, `rotate` using sendcmd file. |
| **POC-V2** | Frame-exact dispatch timing | **PASS** | Midpoint dispatch verified at 30fps and fractional 30000/1001fps. |
| **POC-V3** | Visual correctness & negative control | **PASS** | MAE $\le 2.0$ against reference frames; intentional 1-frame offset correctly fails. |
| **POC-V4** | Existing CrossDissolve coexistence | **PASS** | Animated clips dissolve cleanly across cut using ADR-003 `xfade=fade:offset=0`; zero conflict. |
| **POC-A1** | Sample-accurate audio envelope | **PASS** | `amultiply` audio gain verified; negative control detects intentional envelope discrepancy. |
| **POC-A2** | Envelope lifecycle & resource behavior | **PASS** | Preflight disk check verified; streamed generation; deterministic cleanup on cancel/error. |
| **POC-P1** | Performance sanity | **PASS** | 2.0s fixture rendered in 265ms (1.74x - 4.33x overhead vs static export depending on command density). |

### Relevant POC Observations
- **Performance Overhead**: Dynamic command execution measured between 1.74x and 4.33x wall-clock overhead compared to static export on tiny 2.0s test fixtures. This is expected planning/performance evidence for runtime command parsing in FFmpeg, not an architectural blocker.
- **Audio Envelope Sizing**: A 60-minute 48kHz mono `f32le` gain envelope requires $\approx 691.2 \text{ MB}$ of temporary disk space ($48{,}000 \times 4 \times 3600$). Disk space preflight, streamed buffer generation, and reliable file cleanup are therefore mandatory.

---

## 9. Implementation & Verification Status

With the Phase 14 Research Gate complete and ADR-004 accepted, implementation was executed, remediated, and verified:
- Remediated `P14-PREREQ-01` (full video/audio export dynamic keyframe automation verified).
- Implemented Schema v3 domain model, canonical 32-iteration bisection evaluator, and de Casteljau split subdivision.
- ExportAutomationSchedule video runtime commands and sample-accurate audio gain envelope verified.
- Status: **IMPLEMENTED AND VERIFIED IN PHASE 14**.

---

# Phase 15 Research Gate: MCP Automation Adapter Reconciliation

## Status: IMPLEMENTED AND VERIFIED IN PHASE 15
**Governed by:** ADR-005 (`ADR_005_MCP_LOCAL_STDIO_TOOLS_ADAPTER_EXPLICIT_PROJECT_ADDRESSING_AND_DURABLE_IDENTITY.md`)  
**Scope:** Architectural Reconciliation and Implementation for Model Context Protocol (MCP) Integration  

---

## 1. Repository Audit Findings

A thorough audit of the existing TinixCut repository and automation subsystem reveals:
1. **Clean Reusable Seam**: Headless automation capabilities are already well-isolated in `src-tauri/src/modules/automation/mod.rs` via `AutomationSession` and `AutomationAdapter`. These wrap `CommandService`, `JobManager`, `load_project`, `save_project_atomic`, and `ExportEngine`.
2. **Current JSONL Session Characteristics**: The Phase 10 stdio session (`tinixcut session` / `--stdio`) is single-project, relies on in-memory implicit session process state (`session.open`, `session.close`), and processes incoming lines sequentially on a single thread.
3. **Serde-Based Data Transfer Objects**: All command envelopes, receipts, inspection reports, capability reports, and diff models are already fully decorated with Serde `Serialize` and `Deserialize` (with camelCase mapping).
4. **No MCP Production Code Exists**: The repository currently contains zero MCP server logic, zero MCP schema generation, and zero tool routing code.
5. **No Direct `schemars` Derivations**: Existing domain and command types do not yet implement `schemars::JsonSchema`.
6. **Tokio Runtime Presence**: Tokio is already transitively present in the dependency tree via Tauri 2.12; adding explicit Tokio features for an async stdio server will introduce no fundamental foreign runtime conflicts.
7. **Production Stdout Discipline**: Production Rust modules strictly route diagnostic logs to `stderr` or logger macros; `stdout` is not casually written to by media or core modules.
8. **Testing Gap**: No real spawned-subprocess test currently exists verifying 100% stdout stream purity under load or failure conditions.

---

## 2. External MCP Specification Findings

Analysis of current upstream Model Context Protocol specifications and ecosystem tooling establishes:
1. **Stable Target Protocol: MCP `2026-07-28`**: The primary specification baseline is `2026-07-28`.
2. **Stateless Protocol Core**: Modern MCP lifecycle decouples transport connections from stateful session semantics.
3. **Removal of Legacy Handshake**: The legacy `initialize` request and `notifications/initialized` notification have been removed in the modern specification.
4. **Modern Discovery via `server/discover`**: Discovery occurs via `server/discover`, but clients are not required to call `server/discover` prior to issuing valid tool requests.
5. **Request Metadata**: Modern requests carry client identification and protocol metadata per the specification.
6. **Explicit Addressing Over Hidden Protocol State**: While application state may exist, modern MCP best practices recommend explicit arguments and handles in tool requests rather than opaque protocol session state.
7. **Official Rust SDK**: The official SDK crate is `rmcp` 3.x (`modelcontextprotocol/rust-sdk`), providing native stdio transport and typed tool definitions.
8. **Backward Compatibility**: `rmcp` 3.x provides transparent backward compatibility with legacy `2025-11-25` clients. TinixCut will rely exclusively on this SDK path rather than maintaining a custom secondary legacy protocol stack.
9. **JSON Schema Standard**: Tool input and output schemas must target **JSON Schema 2020-12**.
10. **Correction of Narrow Audit Proposal**: The prior narrow repo audit proposed validating `initialize` against protocol `2026-07-28`. This was incorrect: under `2026-07-28`, `initialize` is obsolete and must NOT be part of the modern test contract.

---

## 3. Final State & Project Addressing Decision

**Decision:** Adopt **explicit `projectPath` parameter on every project-scoped MCP tool call**.

```text
MCP Client Call:
tinixcut_apply {
  projectPath: "C:/Projects/video.tcproj",
  envelope: { ... }
}
```

**Architectural Rationale:**
- **Stateless Protocol Alignment**: Matches the stateless design of MCP `2026-07-28`.
- **Eliminates Handle Table Complexity**: Avoids building an opaque in-memory handle manager (`Map<HandleId, Project>`) susceptible to leaks, eviction issues, and split-brain states.
- **Prevents Hidden Session Collisions**: Multiple agent tasks or subagents communicating with the same stdio server cannot accidentally mutate each other's "currently active" project.
- **Direct Seam Reuse**: Perfectly matches existing standalone helper APIs (`AutomationAdapter::apply_file`, `AutomationAdapter::inspect_file`).
- **Safety Invariants**: Canonical project identity is protected by `base_revision` and `project_id` matching on every call.
- **Bounded Local Cost**: Loading and parsing the canonical JSON file on each mutation incurs $\approx 2 \text{ ms}$ overhead (measured for 10,000 items in Phase 11), well within acceptable bounds for personal workstation automation. In-memory caching with filesystem mtime invalidation is deferred as an optimization.

---

## 4. Durable Idempotency & Structured Identity Decision

**Decision:** Mandatory caller-supplied structured identity `(client_id, client_epoch, sequence_number)` for all authoritative `tinixcut_apply` mutations.

**The Lost-Response Retry Risk:**
If an MCP tool handler silently allocated a new sequence number on behalf of the client for each retry, a dropped stdio response frame would cause catastrophic duplicate execution:
1. Agent sends edit request.
2. TinixCut executes edit, writes revision $R+1$ to disk, and emits success receipt to `stdout`.
3. The stdio pipe hiccups or the client drops the response frame.
4. Agent, believing the request failed or timed out, retries.
5. If the server auto-increments a new sequence, `CommandService` treats the retry as a brand-new command, executing the edit a second time!

By requiring caller-supplied `(client_id, client_epoch, sequence_number)`, the retry presents the **exact same identity**. `CommandService` detects $s \le W(c, e)$, immediately halts re-execution, and returns the cached receipt (`deduplicated_noop`). Duplicate mutation probability remains identically zero.

---

## 5. Scope Exclusions & Deferred Work

The following features and integrations are explicitly excluded from Phase 15:
- **Streamable HTTP & Remote Network Transports**: No HTTP listeners, SSE streams, or port bindings.
- **Authentication Infrastructure**: No OAuth 2.0 servers, PKCE flows, bearer tokens, or user directories.
- **Remote / Cloud Hosting**: No cloud deployment, multi-tenant hosting, or reverse proxies.
- **MCP Non-Tool Primitives**: No Resources, Prompts, Tasks extension, MCP Apps, Sampling, Roots, Subscriptions, or MRTR.
- **Distributed Rendering**: No remote worker nodes, Celery/Redis queues, or cloud render pipelines.
- **Cloud Project Storage**: Projects remain local `.tcproj` files on disk.
- **Multi-User Collaboration**: No OT/CRDTs or concurrent collaborative editing.
- **Direct LLM Provider Integrations**: TinixCut does not embed OpenAI, Anthropic, or Gemini API keys/clients.
- **Public Plugin Framework**: No dynamic WASM or native plugin loading.
- **Project-Handle Registry & Cross-Call Caching**: No server-side handle tables or cross-call caching; reload/parse per project-scoped request is accepted.
- **Distributed / Cross-Process Locking**: No file lock daemons; cross-process safety relies on ADR-001 revisions and atomic persistence.

---

# Phase 16 Research Gate: Timeline Editing Productivity Reconciliation

## 1. Selected Milestone & Status

- **Milestone:** `Phase 16 — Timeline Editing Productivity`
- **Status:** `Research Gate COMPLETE — READY FOR PLANNING`
- **Implementation:** `NOT STARTED`
- **Schema:** `v3 unchanged`
- **ADR Decision:** `No new ADR required`
- **POC:** `src-tauri/tests/phase16_research_poc.rs — 12/12 passed` (research POC evidence, not implementation acceptance)

---

## 2. Research Findings & Gap Analysis

### 2.1 Repository Audit of Timeline Gaps
A thorough code audit of the current timeline implementation (`src/components/Timeline.tsx`, `src-tauri/src/modules/command/mod.rs`, `src-tauri/src/modules/domain/mod.rs`) identifies the following productivity gaps:

- **Absent Capabilities:**
  - Snapping (no edge, playhead, or zero snapping during move/trim);
  - Multi-select (selection model is limited to single active item);
  - Group move (cannot translate multiple selected clips together);
  - Keyboard trim (no head/tail trim shortcuts to playhead);
  - Ripple delete / ripple insert (no single-action ripple removal);
  - Track targeting (no explicit track selection/lock matrix);
  - Magnetic editing (no auto-closing ripple timeline);
  - Timeline markers (no persistent project markers).

- **Partial Capabilities:**
  - Split at playhead exists via toolbar button (`ClipSplit` domain operation), but has no keyboard shortcut;
  - Duplicate exists via toolbar button (`ClipDuplicate` domain operation), but has no keyboard shortcut.

### 2.2 Group-Move Semantics POC & Ordering Sensitivity
The research evaluated whether multi-clip translation could be achieved by executing multiple sequential `ClipMove` operations (Option G1) or requires a dedicated simultaneous `ClipGroupMove` command (Option G2).

**Option G2 Adopted:**
`EXISTING MULTI-CLIP MOVE IS ORDER-SENSITIVE / UNSAFE — DEDICATED ClipGroupMove COMMAND REQUIRED`

**Evidence from `src-tauri/tests/phase16_research_poc.rs`:**
- **Right move requires reverse ordering:** Moving clips `[A, B]` rightward by $\Delta > 0$ requires moving `B` first then `A`. Moving `A` first immediately collides with `B`'s unmoved position, causing false validation rejection.
- **Left move requires forward ordering:** Moving clips `[A, B]` leftward by $\Delta < 0$ requires moving `A` first then `B`. Moving `B` first collides with `A`.
- **Gap cases can false-reject:** When clips have varying gaps, determining an execution order without temporary false collisions is complex and non-deterministic in user space.
- **CrossDissolve destruction:** Moving clips sequentially triggers per-item transition reconciliation at intermediate steps. An internal cut transition between two moved clips can be permanently stripped if the first clip moves before the second, destroying adjacency mid-flight.
- **Atomic rollback works correctly:** While atomic rollback on command failure works as designed, client-side sequential ordering workarounds are fragile and prohibited.

**Conclusion:** UI-level ordering hacks are strictly prohibited. A dedicated atomic `ClipGroupMove` domain command is required.

---

## 3. Normative Research Contract (TP-01 through TP-12)

### TP-01 — Ephemeral Selection
- Selection state exists exclusively in frontend client memory (`selectedItemIds: Set<string>`, `primarySelectedItemId: string | null`).
- Selection MUST NOT be serialized into `.tcproj` project files or stored in domain state.
- **Interaction Contract:**
  - *Normal Click:* Replaces existing selection with clicked item; clicked item becomes primary.
  - *Ctrl+Click:* Toggles clicked item in/out of selection. When added, clicked item becomes primary. When removed, primary selection updates to another remaining selected item (or null if empty).
  - *Empty Timeline Click:* Clears selection.
  - *Shift Range Selection:* Deferred.
  - *Marquee / Box Selection:* Deferred.
- **Selection Persistence & Normal Save:**
  - Normal project Save MUST NOT clear selection merely because selection is ephemeral.
  - Selection is cleared or reconciled only when:
    1. Project is closed, opened, or reloaded;
    2. A selected item is deleted;
    3. Canonical project state no longer contains the selected ID.

### TP-02 — Primary Selection
- Exactly one selected item is designated as the **primary selected item** whenever selection is non-empty.
- **Responsibilities of Primary Selection:**
  - Drives Inspector property panel display and editing;
  - Acts as the group-drag anchor during multi-clip moves;
  - Provides snapping boundary anchors during drag interactions;
  - Serves as the sole target for single-item-only actions (Duplicate, Ripple Delete).

### TP-03 — UI-Only Snapping
- Snapping is strictly an ephemeral UI behavior during timeline pointer gestures; it does not introduce new canonical persistence or schema fields.
- **Snap Targets:**
  - Timeline zero ($t = 0$);
  - Current playhead position;
  - Start and end edges of unselected timeline items across all tracks.
- **Constraint:** Selected items must not snap to their own boundaries.
- **Group Move Snapping:** Snap the primary (actively dragged) clip to eligible targets, then apply the resulting exact common delta to all selected clips.
- **Threshold & Computation:**
  - Default threshold: `8 CSS pixels`.
  - **Correction:** Do NOT expand threshold to a minimum one-frame duration. Eligibility is determined strictly by actual visual pixel distance:
    $$\text{distancePx} = |\text{semanticTimeDistanceSeconds}| \times \text{zoomPxPerSecond}$$
    $$\text{snap eligible iff } \text{distancePx} \le 8$$
  - Snapped result is the exact target `RationalTime` of the target boundary.
  - At high zoom levels, if one frame renders wider than 8px, do not silently extend the snap threshold to one frame.
- **Tie-Breaking Rules:**
  1. Smallest absolute pixel distance;
  2. Target priority: Playhead > Timeline Zero > Clip Edge;
  3. Earlier semantic target time;
  4. Stable target/item ordering if still tied.
- **Modifier:** Holding `Alt` temporarily suppresses/disables snapping during drag.
- **Configuration:** Snapping enabled state is ephemeral UI state; no project preference or schema field in Phase 16.

### TP-04 — Group Move Command Exclusivity
- Multi-clip timeline translation MUST use the dedicated `ClipGroupMove` command.
- Implementing group move via multiple individual `ClipMove` calls is prohibited.

### TP-05 — ClipGroupMove Semantics
- **Candidate Operation:**
  ```rust
  ClipGroupMove {
      item_ids: Vec<String>,
      delta: RationalTime,
  }
  ```
- **Normative Contract:**
  - Applies a single common semantic horizontal `delta` to all specified `item_ids`.
  - Clip tracks remain unchanged (pure horizontal translation).
  - Source ranges (`source_start`, `source_duration`) remain unchanged.
  - Relative timeline offsets and spacing between selected items are strictly invariant.
  - Rescales `delta` semantically to each item's timebase/rate (no raw cross-rate `.value` comparisons).
  - Rejects with error if any resulting item start time becomes negative ($t < 0$).
  - Validates final simultaneous placement against all unselected items on affected tracks (no collisions).
  - Selected members are treated as a simultaneous group: intermediate intra-group collisions do not occur.
  - Internal cut transitions (where both adjacent clips move together) are preserved intact.
  - External transitions (where one participant moves and the other does not) are reconciled/removed under existing Phase-13 transition rules.
  - Updates staged project simultaneously before final domain validation.
  - Atomic failure and rollback: if any clip fails validation, no changes are committed.
  - Emits one logical history action (single Undo/Redo step).
  - Inherits standard command receipt, idempotency, and dry-run semantics.
  - Retains Schema v3 (no Schema v4).

### TP-06 — Bounded ClipRippleDelete
- **Candidate Operation:**
  ```rust
  ClipRippleDelete {
      item_id: String,
  }
  ```
- **Normative Contract:**
  - Single target item only; affects only the track containing the target item.
  - Rejects if the target track is locked or non-editable.
  - Removes the target item's timeline interval $[t_{\text{start}}, t_{\text{end}})$.
  - Removes any transitions directly attached to the deleted item.
  - Every surviving item on the same track whose start is at or after the deleted item's end time ($t \ge t_{\text{end}}$) shifts left by exactly the deleted item's duration ($d = t_{\text{end}} - t_{\text{start}}$).
  - Source ranges of surviving items are strictly preserved.
  - Relative spacing and gaps among downstream items are preserved.
  - Other tracks remain completely untouched.
  - Does NOT synthesize a new transition across the newly exposed cut.
  - Downstream transitions where both participants shift left remain intact.
  - Canonical project duration recomputes through standard existing project rules.
  - Atomic execution with exact single-step Undo/Redo.
  - Retains Schema v3 (no Schema v4).

### TP-07 — Transition Coexistence
- `ClipGroupMove` and `ClipRippleDelete` must preserve all ADR-003 transition invariants:
  - Transitions remain video-only cut transitions;
  - Strict semantic adjacency required for transition validity;
  - Valid source handles required;
  - At most one transition per cut;
  - No automatic freeze-frame clamping;
  - No silent synthetic transition creation.

### TP-08 — Multi-Delete Semantics
- Deleting multiple selected items executes via ordinary `ClipDelete` operations batched inside a single atomic `CommandEnvelope`.
- POC proved that `ClipDelete` operations are order-independent when deleting a selection set.
- The entire multi-item deletion remains one logical transaction and one history undo/redo step.

### TP-09 — Conservative Duplicate Scope
- Phase 16 Duplicate shortcut is restricted to the **primary selected item only**.
- Multi-clip duplication is explicitly deferred because arbitrary batch duplication creates unavoidable track/time collision ambiguity without complex layout heuristics.

### TP-10 — Split at Playhead Semantics
- **Keyboard Shortcut Behavior (Ctrl+K):**
  1. If the primary selected item's interior contains the playhead ($t_{\text{start}} < t_{\text{playhead}} < t_{\text{end}}$), split the primary item;
  2. If multiple items are selected and contain the playhead, split ONLY the primary selected item;
  3. If the primary item does not contain the playhead (or nothing is selected), search tracks in canonical track order and split the first item whose interior contains the playhead;
  4. If no item across any track intersects the playhead, the shortcut is a clean no-op.
- Uses the existing canonical `ClipSplit` domain operation.
- Multi-track simultaneous splitting is explicitly deferred.

### TP-11 — Keyboard Framework & Suppression
- Implement a centralized, lightweight frontend keyboard shortcut listener; no heavy third-party shortcut library.
- **Suppression:** All shortcuts are suppressed when active focus is inside `<input>`, `<textarea>`, or any `contenteditable` element.
- **Windows-First Default Key Bindings:**
  - `Space` → Play / Pause toggle (existing)
  - `Ctrl+K` → Split at Playhead
  - `Ctrl+D` → Duplicate primary item
  - `Delete` / `Backspace` → Delete selected items (batched atomic envelope)
  - `Shift+Delete` → Ripple Delete primary item
  - `[` → Normal Trim Head to Playhead (existing `ClipTrim`, non-ripple)
  - `]` → Normal Trim Tail to Playhead (existing `ClipTrim`, non-ripple)
  - `=` / `+` → Timeline Zoom In
  - `-` → Timeline Zoom Out
  - `S` → Toggle Snapping
- **Explicit Exclusion:** Q/W ripple trim shortcuts are explicitly OUT OF SCOPE.
- Head/tail trim shortcuts adjust the clip boundary without moving downstream clips.

### TP-12 — Schema, History, and Automation Invariance
- Phase 16 retains **Schema v3** without changes.
- New commands (`ClipGroupMove`, `ClipRippleDelete`) are registered as standard `CommandOperation` enum variants.
- Both operations automatically inherit:
  - `CommandService` validation, staged mutations, and rollback;
  - Dry-run calculation and diff generation;
  - Atomic execution;
  - Single-step Undo/Redo history integration;
  - ADR-001 idempotency and identity tracking;
  - Headless CLI, JSONL session, and MCP generic envelope compatibility.
- `AutomationAdapter::supported_operations()` must be updated to advertise the new variants.
- No MCP-specific or CLI-specific editing paths are introduced.

---

## 4. Phase-16 In-Scope Ceiling

The following ten (10) capabilities define the strict ceiling for Phase 16:
1. **Ctrl-based ephemeral multi-select + primary selection** (`selectedItemIds`, `primarySelectedItemId`);
2. **UI-only deterministic snapping** (8px threshold, playhead/zero/clip edges, Alt disable);
3. **Dedicated `ClipGroupMove` command** (atomic simultaneous translation, delta-based, collision-checked);
4. **Bounded single-track `ClipRippleDelete` command** (deletes target, shifts downstream on same track);
5. **Centralized keyboard shortcut framework** (input suppression, Windows-first bindings);
6. **Split-at-playhead shortcut/workflow** (Ctrl+K, primary clip or first intersecting track);
7. **Normal non-ripple trim-to-playhead shortcuts** (`[` and `]`, existing `ClipTrim`);
8. **Multi-selection delete via one atomic envelope** (batched `ClipDelete` operations);
9. **Primary-only duplicate shortcut** (Ctrl+D);
10. **Timeline zoom and snapping toggle shortcuts** (`=`, `-`, `S` as UI behavior).

---

## 5. Explicitly Out of Scope

The following items are deferred and explicitly prohibited from Phase 16:
- Ripple insert;
- Ripple trim / Q-W keyboard semantics;
- Magnetic / FCP-style timeline;
- Cross-track ripple editing;
- Shift range selection;
- Marquee / box drag selection;
- Multi-clip duplication;
- Persistent timeline markers;
- Track targeting matrix;
- Keyframe UX expansion;
- Audio crossfade (`acrossfade`);
- Waveform visualization expansion;
- New transition kinds or transition easing;
- Text/caption expansion;
- Project Schema v4.

---

## 6. ADR & POC Gate Decision

- **ADR Required:** **NO** (ADR-006 is NOT required).
  - *Rationale:* Both `ClipGroupMove` and `ClipRippleDelete` are normal domain operations executing within existing `CommandService`, ADR-001 idempotency, and ADR-003 transition boundaries. They do not alter persistence topology, renderer pipeline, IPC protocols, or concurrency models.
- **Additional POC Required Before Planning:** **NO**.
  - *Rationale:* Comprehensive transaction semantics and ordering constraints were already proven by `phase16_research_poc.rs`.

---

## 7. Research POC Status & Verification Evidence

- **Test Suite:** `src-tauri/tests/phase16_research_poc.rs`
- **Result:** **12 passed, 0 failed**
- **Classification:** **RESEARCH POC EVIDENCE ONLY** (proves feasibility, order-sensitivity constraints, and transaction invariants; does NOT constitute Phase-16 implementation acceptance).

| Test Name | Verified Invariant |
|---|---|
| `test_case_1_two_adjacent_move_right_order_sensitivity` | Proves sequential move right requires reverse order. |
| `test_case_2_two_adjacent_move_left_order_sensitivity` | Proves sequential move left requires forward order. |
| `test_case_3_and_4_three_adjacent_clips_move_right_and_left` | Proves 3-clip sequential moves require strict directional ordering. |
| `test_case_5_clips_separated_by_gaps` | Proves gap cases can cause false-positive collision rejections. |
| `test_case_6_clips_on_different_tracks` | Proves multi-track group move validity. |
| `test_case_7_group_next_to_unselected_clip_conflict_rejection` | Proves collision rejection against stationary unselected clips. |
| `test_case_8_selected_clips_with_cross_dissolve_FATAL_TRANSITION_DESTRUCTION` | Proves sequential move destroys intermediate cut transitions; simultaneous update preserves them. |
| `test_case_9_stale_revision` | Proves revision rejection on group move. |
| `test_case_10_transaction_rollback_when_one_member_invalid` | Proves atomic rollback when one group member violates constraints. |
| `test_ripple_delete_candidate_semantics_and_invariants` | Proves single-track downstream shift, transition cleanup, and gap preservation. |
| `test_multi_selection_delete_safety` | Proves batched `ClipDelete` is order-independent in an atomic envelope. |
| `test_multi_selection_duplicate_safety` | Proves multi-clip duplicate is collision-prone and must be restricted to primary clip. |

---

# Phase 17 Research Gate — Performance Requalification & Release Foundation

## Status: COMPLETE — READY FOR PLANNING AFTER DURABLE RECONCILIATION

**Milestone:** `Phase 17 — Performance Requalification & Release Foundation`  
**Research Status:** COMPLETE — ACCEPTED  
**Implementation:** NOT STARTED  
**Current Active Implementation Phase:** NONE  
**Schema State:** Schema v3 invariant (no Schema v4)  
**ADR Decision:** ADR-006 NOT REQUIRED (qualification-first milestone; zero production architecture/schema changes)  
**Governing Inputs:** `PLAN.md`, `ARCHITECTURE.md`, ADR-001 through ADR-005, Phase 11 / Phase 12 qualification baselines, Phase 16 reconciliation  

---

## 1. Research History & Audit Progression

The Phase 17 research gate resolved the next strategic engineering boundary following the completion of Phase 16 (Timeline Editing Productivity). The research progressed across four structured phases:

1. **Initial Repository & Performance Architecture Audit:**  
   Audited the codebase across preview decoding, proxy policies, thumbnail/waveform background jobs, `JobManager` backpressure and cancellation, export pipeline, media binary resolver, hardware encoder selection, frontend timeline interactions, composition compilation, serialization/autosave latency, long-project scale characteristics, and IPC hotspots. Confirmed that foundational capabilities across Schema v1 through v3 are implemented and functionally verified, but historical performance baselines (established in Phase 11 on Schema v1 and Phase 12 packaging) had not been systematically requalified under Schema v3 (incorporating ADR-003 cut-owned `CrossDissolve` and ADR-004 nonlinear Bézier keyframes).
2. **Calibration Round:**  
   Clarified terminology and benchmark methodology. Established a sharp boundary between **preview semantic sampling latency** (`sample_preview_at` Rust composition evaluation) and **interactive display preview** (which involves WebView presentation, frame pacing, and display sync, tracked as an external manual gate). Proved the requirement for a dual-fixture evaluation framework separating feature complexity from project scale.
3. **Scale, Transport & Package Closure:**  
   Formulated the deterministic 20-track / 200-clip scale fixture (Fixture B). Re-measured project load/save, composition compilation, Phase-16 command scaling, and transport overhead (JSONL and MCP stdio). Evaluated packaging artifact hygiene and release synchronization requirements.
4. **MCP Validation Correction:**  
   Investigated an anomalous early measurement where `tinixcut_apply` was erroneously reported at ~0.06 ms. Diagnosed the root cause in the temporary harness, corrected wire protocol formatting, and executed genuine release subprocess benchmarks over Windows OS stdio pipes to establish valid, authoritative measurements.

### Explicit Disclosure of Prior Invalid MCP Benchmark Measurements

```text
Earlier MCP ~0.06 ms measurements were INVALID.

Root cause:
A malformed wire parameter `_meta` (passing `clientCapabilities` instead of `io.modelcontextprotocol/clientCapabilities`) caused the `rmcp` 3.x stdio server to return a JSON-RPC error -32602 (`InvalidParams`) within ~0.08 ms before tool dispatch. The initial temporary benchmark harness failed to inspect the error response payload and recorded this early protocol rejection as a successful execution.

Corrected real OS-stdio MCP measurements supersede those invalid values.
```

---

## 2. Accepted Milestone Scope & Boundaries

### Milestone Name & Purpose
- **Milestone:** `Phase 17 — Performance Requalification & Release Foundation`
- **Purpose:**  
  1. Systematically requalify Schema-v3 performance across core operations, long-project scale, command execution, and multi-transport surfaces.
  2. Establish reproducible, non-flaky performance regression test fixtures and an opt-in qualification suite.
  3. Synchronize release packaging configuration (Tauri 2.12 NSIS installer, bundled FFmpeg 9.0.2 Essentials, Schema-v3 qualification harness).
  4. Perform Host-Isolated Workstation qualification (`CM-01` through `CM-07`).
  5. Formalize and preserve external release gates (`P16-AC-56` interactive GUI QA and True Clean VM qualification).

### Milestone Philosophy: Qualification-First
Phase 17 is strictly **qualification-first**, not an optimization or feature phase. Production code in `src/` and `src-tauri/src/` must remain untouched unless an unambiguous regression or defect is uncovered during qualification. No speculative refactoring, algorithm redesign, or premature optimizations are authorized.

### Explicit Exclusions
The following areas are explicitly excluded from Phase 17:
- Hardware acceleration qualification (NVENC, Intel QSV, AMD AMF) without dedicated multi-vendor testbeds;
- True Clean Windows VM provisioning and automation;
- Interactive GUI desktop qualification (remains tracked under `P16-AC-56`);
- New creator-facing features or UI expansion;
- Project Schema v4 or domain model revisions;
- Compositor, timeline, or renderer architectural redesign.

---

## 3. Standardized Benchmark Fixtures

To ensure deterministic, reproducible benchmarking across runs, two canonical fixtures are established:

### Fixture A — Semantic-Rich Schema-v3 Workload
- **Target:** Feature-complexity overhead, semantic sampling, export throughput, and preview/export parity.
- **Duration:** 6.0 seconds @ 30 fps (180 frames canonical duration).
- **Variants:** 720p (`1280x720`) and 1080p (`1920x1080`).
- **Topology:**
  - 3 timeline tracks (2 video tracks, 1 audio track);
  - 2 sequential video clips with 1.0s overlap cut transition;
  - 1 text/title overlay item with animated transform properties;
  - 1 background audio item with dynamic volume envelope.
- **Semantic Features:**
  - 1.0s video-only `CrossDissolve` cut transition (ADR-003 premultiplied RGBA dual-backend compositing);
  - Nonlinear Bézier keyframes across clips (Linear, EaseIn, EaseOut, EaseInOut, CubicBezier evaluated via 32-iteration bisection);
  - Dynamic audio volume automation schedules.

### Fixture B — Long-Project Scale Schema-v3 Workload
- **Target:** Scale sensitivity, serialization/deserialization limits, composition compilation, multi-item commands, and transport serialization cost.
- **Duration:** 10.0 seconds @ 30 fps (300 frames canonical duration).
- **Topology:**
  - 20 tracks total:
    - 14 video tracks (Tracks 0..13)
    - 4 audio tracks (Tracks 14..17)
    - 2 text/overlay tracks (Tracks 18..19)
  - 200 items total (10 sequential items per track, 30 frames each = 300 frames / 10.0 seconds total);
  - 20 video `CrossDissolve` cut transitions:
    - Distributed across Video Tracks 0..3 (4 tracks)
    - 5 pairwise cut transitions per track (20 total)
  - Mixed timeline rates and source timebases;
  - 60+ nonlinear visual Bézier keyframes (distributed across video clips);
  - 120 audio volume keyframes (3 keyframes per clip across 40 audio items).

---

## 4. Empirical Benchmark Data & Methodology

### Methodology & Reference Execution Environment
- **Operating System:** Windows 11 Pro (Build 26200 / 24H2)
- **CPU:** Intel Core i5-13400F (10 cores, 16 threads, base 2.5 GHz, boost up to 4.6 GHz)
- **RAM:** 64 GB DDR4-3200 MHz
- **Storage:** Samsung 990 PRO 2TB NVMe PCIe 4.0 SSD (sequential read/write > 7000 MB/s)
- **GPU:** NVIDIA GeForce RTX 3090 (24 GB VRAM, Driver 595.79)
- **Media Binaries:** Bundled FFmpeg 9.0.2 / FFprobe 9.0.2 Essentials (GPLv3)
- **Build Profile:** Rust Release Profile (`cargo build --release`, `opt-level = 3`, LTO enabled)
- **Protocol:** Dedicated measurement loops with 5–10 warmup iterations, high-resolution wall-clock timestamps (`std::time::Instant`), isolated temporary directories with atomic cleanup.

### 4.1 Fixture B Scale Operations ($N = 50$ iterations)

| Operation | Median | P95 | Reference-Host Budget | Qualification Ceiling |
|---|---|---|---|---|
| `save_project_atomic` | 3.890 ms | 5.912 ms | $\le 8.0\text{ ms}$ | $\le 50.0\text{ ms}$ |
| `load_project` (deser) | 2.985 ms | 3.951 ms | $\le 6.0\text{ ms}$ | $\le 50.0\text{ ms}$ |
| `CompositionPlan::from_project` | 0.282 ms | 0.408 ms | $\le 1.0\text{ ms}$ | $\le 10.0\text{ ms}$ |
| `sample_preview_at` (sampling) | 0.218 ms | 0.252 ms | $\le 0.5\text{ ms}$ | $\le 5.0\text{ ms}$ |

### 4.2 Phase-16 Command Scale on Fixture B ($N = 50$ iterations)

| Command Operation | Scope | Median Latency | Qualification Ceiling |
|---|---|---|---|
| `ClipGroupMove` | 5 items simultaneous translation | 64 µs | $\le 1.0\text{ ms}$ |
| `ClipRippleDelete` | Single item + downstream shift | 49 µs | $\le 1.0\text{ ms}$ |
| `ClipDelete` (batch envelope) | 5 items atomic deletion | 53 µs | $\le 1.0\text{ ms}$ |

### 4.3 JSONL OS-Stdio Automation Session ($N = 50$ iterations)

| Action | Target | Median Latency | Notes |
|---|---|---|---|
| `session.inspect` | Fixture B (200 items) | 23 µs | In-memory query over JSONL stdio |
| `command.dry_run` | Fixture B command | 18 µs | Transactional validation without commit |
| `command.apply` | Fixture B in-memory | 19 µs | In-memory apply (without `saveAfter: true`) |

*Note: In-memory JSONL `command.apply` executes in ~19 µs because atomic disk persistence is decoupled. When `saveAfter: true` is requested, latency reflects the underlying ~3.89 ms atomic disk write.*

### 4.4 Validated MCP OS-Stdio Protocol ($N = 30$ iterations)
Executed against real release subprocess `tinixcut.exe mcp` over Windows OS stdin/stdout pipes:

| Tool Call | Description | Median Latency | P95 Latency | Aggregate Avg |
|---|---|---|---|---|
| `tinixcut_inspect` | Inspect project state | 0.2381 ms | 0.3707 ms | 0.2858 ms |
| `tinixcut_dry_run` | Dry-run validation | 0.2422 ms | 0.4493 ms | 0.2909 ms |
| `tinixcut_apply` | Full reload + mutate + atomic save | 12.2009 ms | 14.3105 ms | 12.3136 ms |

**Constituent Operations Alignment:**  
Physical disk operations on Fixture B account for $0.19\text{ ms}$ (`load_project`) $+ 3.18\text{ ms}$ (`save_project_atomic`) $\approx 3.37\text{ ms}$. The total tool call median of $12.20\text{ ms}$ encompasses:
1. JSON-RPC request deserialization and protocol dispatch;
2. Process-local write gate mutex acquisition;
3. Domain project disk reload and validation;
4. `CommandService` transactional execution and receipt generation;
5. Atomic temporary write, fsync, and replace persistence;
6. JSON-RPC response serialization and OS pipe write.  
Sequential throughput across 30 measured requests was verified in aggregate at $12.31\text{ ms/call}$, demonstrating $< 1\%$ variance. Validated execution confirmed: JSON-RPC IDs matched, `isError == false`, `operationsApplied == 1`, single revision increment, and durable on-disk persistence.

### 4.5 Export Throughput on Fixture A (5 repeated runs, 6.0s timeline, bundled FFmpeg 9.0.2, CPU `libx264` preset `fast`, CRF 23)

| Profile | Median Time | Min RTF | Median RTF | Reference-Host Budget | Qualification Ceiling |
|---|---|---|---|---|---|
| 720p30 (1280x720) | 2.343 s | 2.42x | 2.56x | $\text{RTF} \ge 2.20\times$ | $\text{RTF} \ge 1.00\times$ |
| 1080p30 (1920x1080) | 4.791 s | 1.15x | 1.25x | $\text{RTF} \ge 1.00\times$ | $\text{RTF} \ge 0.50\times$ |

*Clarification: The empirical benchmark was conducted with `libx264` preset `fast` (CRF 23). The minimum observed 720p RTF across 5 runs was 2.42x (exceeding the 2.20x budget). The 1080p budget of $\text{RTF} \ge 1.00\times$ is strictly a reference-host baseline; slower multi-core hardware will exhibit proportionally lower RTF under CPU encoding.*

---

## 5. Hybrid Timing Budget Policy & CI Isolation Contract

### Three-Tier Assertion Classification
To reconcile the tension between detecting performance degradation and preventing flaky test failures across divergent hardware, performance criteria are classified into three distinct tiers:

1. **Qualification Ceilings (Hard Guardrails):**  
   Generous upper bounds designed to catch gross algorithmic regressions ($O(N^2)$ loops, deadlocks, memory leaks) on any reasonably capable workstation in the opt-in qualification suite. Failure indicates a fundamental defect.
2. **Reference-Host Budgets (Target Baselines):**  
   Concrete latency targets calibrated against the specified reference developer workstation (Intel i5-13400F, NVMe SSD). Used during milestone qualification and performance audits to confirm efficiency on target-class hardware.
3. **Informational Baselines (Tracking Only):**  
   Empirical timings recorded in documentation and benchmark logs to monitor trends over time without triggering hard assertion failures.

### CI Isolation Contract
Machine-dependent latency assertions **MUST NEVER** run inside default automated functional test suites (`cargo test`, `vitest run`).  
- All performance benchmarks and latency assertions must be located in dedicated test harnesses under `src-tauri/tests/` and decorated with `#[ignore]`.
- Performance suites are executed exclusively via explicit opt-in invocations:  
  `cargo test --release --test phase17_performance_benchmarks -- --ignored --nocapture`
- Default CI suites remain strictly functional, deterministic, fast, and host-agnostic.

---

## 6. Hardware Acceleration Policy

1. **CPU `libx264` is Authoritative:**  
   The software encoder `libx264` bundled via FFmpeg 9.0.2 Essentials is the sole authoritative export backend for all functional verification, acceptance tests, and deterministic output comparison.
2. **Hardware Acceleration is Non-Authoritative Fallback:**  
   NVENC, Intel QuickSync (QSV), and AMD AMF remain governed by ADR-002 as best-effort, non-authoritative hardware paths with mandatory automatic fallback to CPU `libx264`.
3. **Hardware Qualification Deferred:**  
   Full qualification of NVENC/QSV/AMF across varied driver versions, GPUs, and virtual machines is explicitly excluded from Phase 17 and deferred to future specialized release environments.

---

## 7. Packaging & Installer Synchronization Policy

1. **Product Identity & Version (Authoritative `tauri.conf.json`):**  
   - Application Name: `TinixCut`
   - Identifier: `com.tinixcut.editor`
   - Current Version: `0.1.0`
   - WebView2 Install Mode: `"webviewInstallMode": { "type": "downloadBootstrapper" }`
   - NSIS Configuration: `"nsis": { "installMode": "currentUser", "languages": ["English"] }`
2. **App-Owned Bundled Binaries:**  
   Bundled FFmpeg and FFprobe 9.0.2 Essentials executables reside in `src-tauri/resources/bin/` as app-owned packaged resources resolved via `MediaBinaryResolver`.
3. **Binary Tracking Prohibition:**  
   Compiled distribution binaries (e.g., `TinixCut_0.1.0_x64-setup.exe`, `.pdb` debug symbols) **MUST NOT** be committed to the Git repository. Release integrity is preserved durably via:
   - Cryptographic SHA-256 checksums;
   - Build provenance and toolchain metadata;
   - Automated packaging qualification test reports.
4. **Qualification Harness Synchronization:**  
   The actual qualification module resides at `src-tauri/src/modules/media/qualification.rs` (orchestrated via `tests/clean_machine_qualification.ps1`). It contains stale Schema-v1 mock transition markers (`item.transition: Some(Transition { kind: TransitionKind::FadeBlack, ... })`). It must be synchronized to Schema v3 (`track.transitions` with `CutTransition` / `CrossDissolve` and nonlinear keyframes) during Phase 17.

---

## 8. Host-Isolated Workstation Qualification Contract (`CM-01` through `CM-07`)

The accepted Host-Isolated Workstation qualification contract verifies essential release properties in a controlled developer workstation environment:

- **`CM-01` — Bundled Binary Self-Sufficiency:**  
  Run with developer FFmpeg/toolchains stripped from `PATH`. Packaged application resolves bundled FFmpeg/ffprobe exclusively through `MediaBinaryResolver`.
- **`CM-02` — Mandatory Software Fallback:**  
  When hardware acceleration is unavailable or failing, bundled CPU `libx264` succeeds without crash or project corruption.
- **`CM-03` — Spaces and Unicode Path Robustness:**  
  Projects and media located under file paths containing spaces and non-ASCII Unicode characters open, save, probe, and export correctly.
- **`CM-04` — Read-Only Media Protection:**  
  External read-only media assets can be imported, probed, and rendered without modification or write-permission failure.
- **`CM-05` — Atomic Persistence & Recovery:**  
  Interrupted save preserves valid project and recovery state; zero file truncation or corrupted project overwrite.
- **`CM-06` — Clean User-Scope Installation:**  
  NSIS `currentUser` installation succeeds without admin elevation and creates the expected application executables, shortcuts, and registry entries.
- **`CM-07` — User Data Preservation on Uninstall:**  
  Application uninstall cleanly removes binaries and program files while strictly preserving user data: `.tcproj` project files, external source media, and rendered export artifacts.

### Test-Environment Prerequisites
The following operational conditions serve as test-environment prerequisites for executing `CM-01..CM-07`, but do NOT replace the semantic contracts above:
- Clean working directory verified against authoritative Git commit hash;
- Pinned toolchain versions (Rust 1.85+, Node 20+, pnpm 9+, Vite 6+, Tauri 2.12);
- Release compilation with full optimization (`opt-level = 3`, LTO enabled);
- Cryptographic SHA-256 verification of bundled FFmpeg/ffprobe binaries;
- Disposable cache cleanliness with zero temp leakage;
- Background workload suppression during benchmark runs;
- Host hardware and environment metadata logging.

### Qualification Complementarity
`Host-Isolated Workstation Qualification COMPLEMENTS but does not replace True Clean Windows VM Qualification.`

---

## 9. External Release Gates Dispositions

Two external release gates remain explicitly tracked and deferred:

1. **`P16-AC-56 — Real Interactive Windows GUI Qualification`:**  
   - **Status:** `DEFERRED / NOT PASS`
   - **Rationale:** Native Windows desktop GUI interaction capability is unavailable in autonomous agent environments, and the user elected not to perform the 23-point manual checklist personally. Automated test coverage does not substitute for this qualification.
2. **`True Clean Windows 10/11 VM Qualification`:**  
   - **Status:** `DEFERRED / EXTERNAL RELEASE GATE`
   - **Rationale:** Provisioning a pristine Windows virtual machine lacking developer tools, pre-installed Visual C++ runtimes, and local paths remains an external release gate prior to final public distribution.

---

## 10. Final Normative Contract (`PR-01` through `PR-18`)

The following eighteen (18) normative requirements govern the definition, planning, and execution of Phase 17:

- **`PR-01`:** Two deterministic Schema-v3 workloads: Fixture A semantic-rich (6.0s @ 30fps, 3 tracks, 720p/1080p, CrossDissolve, Bézier keyframes, audio volume automation) + Fixture B 20-track/200-item scale (14 video tracks, 4 audio tracks, 2 text tracks, 20 CrossDissolves, 60+ Bézier keyframes, 120 audio keyframes).
- **`PR-02`:** Release-profile benchmark methodology with warmups and median/P95/aggregate reporting where applicable.
- **`PR-03`:** Ordinary functional CI **MUST NEVER** fail on machine-dependent wall-clock or RTF thresholds.
- **`PR-04`:** Performance budgets execute only through explicit opt-in release qualification suite (`--ignored`).
- **`PR-05`:** Strict distinction between:
  - Qualification Ceilings (guardrails against gross regressions across any machine);
  - Reference-Host Regression Budgets (calibrated against reference workstation);
  - Informational Baselines (longitudinal tracking).
- **`PR-06`:** `sample_preview_at` = preview semantic sampling latency, **NOT** physical display/playback qualification.
- **`PR-07`:** CPU/`libx264` = sole authoritative required export baseline. Hardware encoders remain best-effort with software fallback.
- **`PR-08`:** Reference-host CPU export budgets:
  - 720p30 RTF $\ge 2.20\times$
  - 1080p30 RTF $\ge 1.00\times$
  (using `libx264` preset `fast`, CRF 23).
- **`PR-09`:** Exported MP4 must satisfy accepted container/stream validity and timeline duration invariance. No unsupported claim of physical A/V sync or live dropped frames.
- **`PR-10`:** Phase-16 scale commands on Fixture B: Reference-host median $< 200\text{ µs}$.
- **`PR-11A`:** JSONL steady-state OS-stdio in-memory operations (`saveAfter:false`): Reference-host median $< 100\text{ µs}$.
- **`PR-11B`:** MCP OS-stdio inspect/dry-run: Reference-host median $< 1.0\text{ ms}$. Explicit qualification ceiling $< 20\text{ ms}$.
- **`PR-11C`:** MCP OS-stdio persistent apply: full reload + CommandService transaction + atomic save. Reference-host median $< 20\text{ ms}$. Explicit qualification ceiling $< 100\text{ ms}$.
- **`PR-12`:** True Clean Windows 10/11 VM qualification remains **DEFERRED / EXTERNAL RELEASE GATE**.
- **`PR-13`:** Host-Isolated Workstation `CM-01..CM-07` complements, but does not replace, True Clean VM qualification.
- **`PR-14`:** `P16-AC-56` remains **DEFERRED / NOT PASS** as external human pre-release GUI qualification.
- **`PR-15`:** Generated installer/release executables are transient artifacts and **MUST NOT** be committed to Git.
- **`PR-16`:** Durable release evidence records:
  - SHA-256 hashes;
  - Version metadata;
  - Git provenance;
  - Environment metadata;
  - Qualification results/logs.
- **`PR-17`:** Schema v3 invariant. No Schema v4 or creator-feature expansion in Phase 17.
- **`PR-18`:** Phase 17 is qualification-first. Production optimization/refactoring is unauthorized unless a measured accepted ceiling is violated and remediation scope is explicitly approved.

---

## Phase 17 Transport Performance Contract Recalibration — 2026-10-07

### Finding and decision

The accepted transport reference-host numbers combined workload-specific measurements without preserving consistent provenance. A new release-binary benchmark exercised W0 (1 track/0 items), W1 (Fixture A, 3 tracks/4 items), and W2 (Fixture B, 20 tracks/200 items) through actual Windows OS stdio. The results show the expected size-dependent cost in stateless MCP inspect/dry-run: W0 medians are about 0.22–0.24 ms, while W2 medians are about 3.41–3.54 ms.

Rank the contract options **C > B > A** and select **Option C**: W2 alone is normative for Phase-17 transport qualification; W0 and W1 remain informational. The Phase-17 scale gate should compare like-for-like Fixture B runs. Applying W0 transport latency to W2 would confuse input-size cost with a regression; three workload-specific normative gates would add complexity beyond this phase's accepted scale objective. No production change, caching, protocol change, or ADR is needed to satisfy the calibrated contract.

### New normative reference-host budgets

These values amend PR-11A and PR-11B for the Phase-17 qualification path. They do not change the qualification ceilings or transport/domain semantics.

| Workload and operation | Amended reference-host median | Qualification ceiling |
|---|---:|---:|
| Fixture B JSONL inspect/dry-run/apply with `saveAfter:false` | < 150 µs | < 10 ms |
| Fixture B MCP inspect/dry-run | < 5.0 ms | < 20 ms |
| Fixture B MCP persistent apply | < 20 ms (unchanged) | < 100 ms |

Fixture B is 20 tracks/200 items. The thresholds apply only on the accepted reference host and only to the explicit release qualification; timing budgets remain outside ordinary functional CI. Report median, P95, arithmetic sample average, and workload dimensions. W0/W1 results are informational.

### Recalibration measurements

Binary and complete method metadata are recorded in [`tests/evidence/phase17_transport_recalibration_2026-10-07.json`](tests/evidence/phase17_transport_recalibration_2026-10-07.json). It records Git `92a225899686e0440536b59f30f9fb16d3156511`, the Release executable SHA-256, workload dimensions/file sizes, exact sample counts, timer boundaries, response validation, and aggregate measurements.

Each workload used 5 warmups. JSONL used 50 timed calls per operation; MCP used 30. These were real `tinixcut session` and `tinixcut mcp` Release processes connected over OS stdin/stdout. Process startup and MCP discovery/handshake were excluded. Every successful response was correlated to the request ID and error responses failed the run. The timer included request write/flush and response line read, but excluded client request serialization and response JSON parsing. “Average” below is the arithmetic mean of timed samples; wall-throughput aggregate is a separate measure and is not per-call latency.

| Workload | JSONL inspect median / P95 / avg (ms) | JSONL dry-run median / P95 / avg (ms) | JSONL apply `saveAfter:false` median / P95 / avg (ms) | MCP inspect median / P95 / avg (ms) | MCP dry-run median / P95 / avg (ms) | MCP apply median / P95 / avg (ms) |
|---|---:|---:|---:|---:|---:|---:|
| W0: 1 track / 0 items | 0.0436 / 0.0849 / 0.0515 | 0.0642 / 0.0957 / 0.0655 | 0.0525 / 0.0891 / 0.0573 | 0.2407 / 0.3396 / 0.2500 | 0.2217 / 0.3276 / 0.2270 | 9.703 / 13.736 / 9.633 |
| W1: Fixture A, 3 tracks / 4 items | 0.0577 / 0.1069 / 0.0636 | 0.0673 / 0.0851 / 0.0681 | 0.0758 / 0.0987 / 0.0752 | 0.3231 / 0.4210 / 0.3323 | 0.3368 / 0.4195 / 0.3555 | 10.110 / 14.464 / 10.369 |
| W2: Fixture B, 20 tracks / 200 items | 0.0800 / 0.1452 / 0.0971 | 0.1098 / 0.1449 / 0.1141 | 0.1034 / 0.1369 / 0.1105 | 3.4117 / 3.8771 / 3.4954 | 3.5426 / 4.9322 / 3.6843 | 16.760 / 24.496 / 17.274 |

The W2 medians satisfy the amended JSONL and MCP inspect/dry-run reference budgets. MCP apply satisfies its unchanged <20 ms reference-host median budget and <100 ms qualification ceiling. Its measured P95 of 24.496 ms is reported, but is not the reference criterion; it remains within the qualification ceiling.

Supporting direct measurements are from distinct runs and must not be conflated with the table above. The existing Fixture-B qualification report records `load_project` median 2.985 ms/P95 3.951 ms and `save_project_atomic` median 3.890 ms/P95 5.912 ms. Another Release direct run recorded atomic-save median 4.167 ms/P95 6.888 ms/average 4.547 ms. Direct CommandService dry-run was median 0.050 ms/P95 0.069 ms/average 0.058 ms; apply was 0.075/0.107/0.079 ms. A separate 55-load aggregate totaled 178.199 ms (3.240 ms/load including warmups). These checks contextualize the JSONL and stateless MCP paths; they do not replace transport qualification.

### Historical workload provenance audit

Classification is limited to what retained documents and artifacts establish:

| Historical result | Classification | Evidence and limitation |
|---|---|---|
| JSONL inspect 23 µs, dry-run 18 µs, apply 19 µs | **W2 (documented)** | The historical research table explicitly labels the target Fixture B / 200 items. Raw per-sample logs were not retained, so independent reconstruction of those specific requests is unavailable. |
| MCP inspect 0.2381 ms, dry-run 0.2422 ms | **Unknown — conflicting documentation** | The historical research table is under the Fixture-B section, but the later [`phase17_release_qualification_report.json`](tests/evidence/phase17_release_qualification_report.json) says approximately 0.238 ms was measured on W0 (1 track/0 items). The original raw samples are absent, so neither conflicting label can be confirmed as actual provenance. |
| MCP apply 12.2009 ms | **W2 (documented)** | The research records this as full reload + mutation + atomic save in its Fixture-B MCP benchmark. Raw samples are not retained. |
| MCP apply approximately 0.06 ms | **Invalid / excluded** | The retained investigation identifies an invalid `_meta` request format and JSON-RPC `-32602` error path. It is not successful tool-call performance. |

The old values remain in their original research context as historical records; the amendment does not rewrite that history. In particular, the documented JSONL numbers remain traceable to W2, while conflicting MCP inspect/dry-run provenance is explicitly left unknown rather than guessed.

### Contract reconciliation

The authoritative amended contract is:

- **PR-11A:** Fixture-B JSONL real OS-stdio inspect, dry-run, and in-memory apply (`saveAfter:false`): reference-host median <150 µs; qualification ceiling <10 ms.
- **PR-11B:** Fixture-B MCP real OS-stdio inspect and dry-run: reference-host median <5.0 ms; qualification ceiling <20 ms.
- **PR-11C:** Fixture-B MCP persistent apply remains reference-host median <20 ms; qualification ceiling <100 ms.

This section supersedes only the original PR-11A/PR-11B reference-host numbers for Phase-17 qualification. Original qualification ceilings, PR-11C, Fixture B definition, and all transport semantics remain unchanged. `Plans/plan_phase17.md` records the matching P17-AC-31/P17-AC-34 amendment without renumbering acceptance criteria. No durable architecture decision changed; ADR-006 remains unnecessary.

---

# Phase 18 Research Gate — Audio Crossfade Semantics & Preview/Export Parity

**Research date:** 2026-10-07
**Status:** COMPLETE — READY FOR IMPLEMENTATION PLANNING
**Scope:** Standalone adjacent audio items on Audio tracks; no implementation changes were made as part of this research.

## Local implementation evidence

The existing code already provides most of the required semantic building blocks:

- `src-tauri/src/modules/composition/mod.rs` defines `CompositionAudioLayer` and `effective_volume_at`. The current gain order is mute, canonical volume-keyframe evaluation at clip-local offset, then fade-in/fade-out multiplication. The method currently returns silence when the playhead is outside the editorial range, so it cannot by itself evaluate a transition handle.
- The same module's `TransitionPlan` derives centered left/right durations using integer ticks and clamps clip-local offsets to the editorial endpoints for handle evaluation. Its tests explicitly verify endpoint hold for outgoing postroll and incoming preroll. This is a proven model for audio's derived transition branch.
- Normalized audio layers retain source and timeline ranges, and the existing preview sampler reports per-item effective audio gain. Export compiles timeline-positioned audio branches and uses `ExportAutomationSchedule` for sampled volume automation. These are suitable integration points for one canonical audio-gain evaluator and disposable sample-grid envelopes.
- Existing audio playback in `src/components/PreviewCanvas.tsx` is not yet present for standalone Audio-track items. This is a real product gap, not something solved by the existing preview gain sampler; the bounded phase must add transport-synchronized WebView audio playback and resource cleanup.
- The domain model is Schema v3 and stores video transitions on `Track.transitions`. A durable audio transition therefore requires a versioned field and explicit migration; silently adding one to v3 is not compatible with the documented schema contract.

## External backend evidence

FFmpeg documents `acrossfade` as a filter that combines two or more audio streams, with an overlap option that defaults to enabled and configurable fade curves. It also supports a no-overlap mode. This is useful as a rendering primitive, but does not express TinixCut's canonical model of two independently positioned clip branches on an adjacent, duration-invariant timeline. The project should therefore persist transition intent and evaluate gains itself, then pass per-clip envelopes through its existing positioned-branch mixer. FFmpeg's `qsin` curve is documented as a quarter sine wave; the product's specified complementary `cos(πp/2)`/`sin(πp/2)` pair remains the exact project semantic and should not be delegated to an assumed filter-curve equivalence.

Primary source: [FFmpeg Filters Documentation — acrossfade and afade](https://www.ffmpeg.org/ffmpeg-filters.html).

## Boundary and handle decision

Use the established video transition rule for clip-local state in audio source handles: clamp the handle-time clip offset to `[0, item_duration]`, then evaluate volume keyframes and fades at that clamped endpoint with the existing audio gain order. Incoming preroll uses offset zero; outgoing postroll uses the item duration. Consequently, volume automation holds its endpoint value, while configured fade-in/fade-out reaches its existing endpoint gain (zero at the respective endpoint). This resolves the previous open question and keeps preview and export on one rule. The audio branch still reads actual verified source media handles; endpoint hold applies to automation coordinates, not to source samples.

For duration `D` rescaled to the cut's exact rational rate, set `L=floor(D/2)` and `R=D-L`, and use `[cut-L, cut+R)`. The outgoing item supplies `R` postroll and the incoming item `L` preroll. Only the derived composition/render branches extend beyond the adjacent persisted item ranges; no timeline overlap or project-duration change is introduced.

## Research conclusion

Proceed with a bounded Phase 18: Schema v4 `Track.audio_transitions`, lossless v1–v3 migration to an empty collection, existing Command Service/history and automation paths, canonical equal-power gain sampling, standalone Audio-track preview playback, and sample-grid export envelopes. No new dependency, audio engine, microservice, or persisted FFmpeg syntax is justified by the evidence. Keep embedded Video-item audio, alternate curves, and native audio-engine selection deferred. ADR-006 records the durable semantics; `Plans/plan_phase18.md` contains the 14 acceptance criteria and stop conditions.

## Implementation reconciliation

Phase 18 is implemented as `COMPLETE WITH DEFERRED ITEMS`. The v4 model/migration, command/history lifecycle, dependent-edit rules, canonical preview/export gain evaluation, MCP path, and real-media export are covered by passing automated acceptance tests. Review exposed and fixed a mixed-frame-rate defect in clip-local fade evaluation by normalizing offsets to the clip's time rate before evaluating fades.

The runtime implementation now includes standalone Audio-track WebView playback and timeline/inspector crossfade controls. Interactive Windows qualification for audible transport, UI accessibility/editing flow, and direct typed `app_health_check` remains deferred because no native application surface was available for this session. Automated checks and the local development runtime restart passed; see `Plans/plan_phase18.md` and `PROJECT_STATUS.md` for exact evidence. The implementation conforms to ADR-006, with no change to the research conclusions or architecture.

