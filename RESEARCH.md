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
8. **Commercial distribution/license model:** needed to finalize FFmpeg/codec/Remotion decisions.
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
