# ADR-004: Canonical Keyframe Easing, Clip-Local Lifecycle, and Sampled Export Automation Semantics

## Status

**ACCEPTED & IMPLEMENTED — Phase 14**  
**Date:** 2026-10-05  
**Deciders:** Core Architecture Team  
**Scope:** Keyframe Easing, Clip-Local Keyframe Address & Lifecycle (Split/Trim), Schema v3 Migration, Full Export Automation Schedule  
**Consulted:** Phase 14 Technical Research Gate, Gate D/E Parity Specifications, FFmpeg runtime filtering internals, CSS Transitions / Easing Level 1, de Casteljau Subdivision Algorithms  
**Implementation State:** Fully implemented, verified green across 129 Rust tests (106 unit, 8 POC, 15 acceptance) and 14 Vitest tests. See Section 14 for verification note.


---

## 1. Context and Problem Statement

During Phase 9, single-clip keyframing was introduced for volume and transform properties, followed by preview-canvas scrubbing and single-frame snapshot evaluation. However, research and repo auditing revealed three structural deficiencies:

1. **Phase 9 Full-Export Discrepancy (`P14-PREREQ-01`)**: While preview (`PreviewCanvas`) and single-frame snapshot export evaluate dynamic linear keyframes, full video export (`CompositionPlan::build_export_filtergraph`) was implemented using static item properties. Full export completely ignored dynamic keyframe timelines, causing full exports of animated projects to drop keyframes.
2. **Keyframe Ownership and Timeline Lifecycle Gaps**:
   - Split operations (`split_item`) naively cloned all keyframes to both fragments without rebasing offsets relative to the cut boundary, corrupting animation timing on the right fragment.
   - Trim operations lacked clear semantics for hidden keyframes outside the visible clip duration, risking data loss or desynchronization when clips are re-extended.
   - Keyframe lookups and equality checks in several modules relied on raw integer tick comparisons (`.offset.value == .offset.value`) rather than canonical semantic `RationalTime` equality, creating subtle bugs when mixed time rates were present.
3. **Absence of Non-Linear Easing**: Keyframe interpolation was hard-coded to linear interpolation. In professional workflows (and CapCut parity), non-linear easing (`EaseIn`, `EaseOut`, `EaseInOut`, and custom cubic Bézier curves) is standard. Introducing easing without an authoritative mathematical specification risks divergences between preview (browser/CSS/JS) and export (FFmpeg).

Phase 14 defines the canonical mathematical contract, domain ownership, command mutations, clip lifecycle (split/trim), versioned persistence schema (Schema v3), and export automation architecture.

---

## 2. Decision Drivers

- **Single Canonical Evaluator in Rust/Core**: All keyframe curves must be evaluated by a single, deterministic Rust mathematical evaluator. No renderer, browser CSS engine, or FFmpeg filter expression may become the canonical source of truth for interpolation.
- **Strict Clip-Local Address & Ownership**: Keyframes belong strictly to their containing `TimelineItem`. Stable keyframe IDs are explicitly rejected for Phase 14; keyframes are addressed exclusively by `(property, semantic RationalTime offset)`.
- **Mathematical Invariant & Subdivision Determinism**: Splitting an animated clip across an interpolation segment must preserve the exact continuous curve function on both fragments via deterministic bisection and de Casteljau Bézier subdivision.
- **Hidden Keyframe Retention on Trim**: Trimming a clip head or tail must never silently delete keyframes. Head trims shift keyframes into valid canonical negative offsets; tail trims preserve post-duration keyframes. Extending clips restores animation losslessly.
- **Schema v3 Promotion**: Introducing easing requires promoting the project format to Schema v3. Keeping Schema v2 would allow older editors to deserialize projects, drop unknown easing fields, and silently resave corrupted linear animations.
- **Decoupled Export Automation via `ExportAutomationSchedule`**: Full export evaluates keyframes in Rust at the target frame/sample grid and compiles an ephemeral `ExportAutomationSchedule`. FFmpeg executes literal runtime commands and receives sample-accurate audio envelopes; FFmpeg executes zero interpolation math.
- **Coexistence with ADR-003 CrossDissolve**: Dynamic keyframe animation and ADR-003 transition branches must coexist seamlessly without altering cut-owned topology or premultiplied cross-dissolve invariants.
- **Strict Scope Discipline**: Overshoot Bézier curves, physics/springs, hold/step interpolation, motion paths, shortest-path rotation wrapping, general curve/graph editors, transition easing, audio crossfades, and MCP remain deferred.

---

## 3. Mathematical Specification: Canonical Cubic Bézier & Solver

### 3.1 Supported Easing Varieties

Phase 14 keyframe interpolation supports exactly five easing variants on the outgoing keyframe:

| Easing Kind | Classification | Control Points $(x_1, y_1, x_2, y_2)$ | Description |
|---|---|---|---|
| `Linear` | Identity | N/A | Standard linear progress: $e(u) = u$ |
| `EaseIn` | Preset Cubic | $(0.42, 0.0, 1.0, 1.0)$ | Accelerating from zero velocity |
| `EaseOut` | Preset Cubic | $(0.0, 0.0, 0.58, 1.0)$ | Decelerating to zero velocity |
| `EaseInOut` | Preset Cubic | $(0.42, 0.0, 0.58, 1.0)$ | Acceleration then deceleration |
| `CubicBezier` | Parametric Cubic | $(x_1, y_1, x_2, y_2)$ | Arbitrary user-defined cubic Bézier |

### 3.2 Control Point Constraints

For any `CubicBezier { x1, y1, x2, y2 }`:
1. All four coordinates must be finite `f64` floats (no `NaN`, `+Inf`, or `-Inf`).
2. Coordinates are bounded to unit range:
   $$0.0 \le x_1, y_1, x_2, y_2 \le 1.0$$
3. **No Overshoot in Phase 14**: Because $y_1, y_2 \in [0.0, 1.0]$, the eased progress $e(u)$ is strictly bounded in $[0.0, 1.0]$. Anticipation/overshoot ($y < 0$ or $y > 1$) is deferred.
4. **No Ordering Constraint**: There is no requirement that $x_1 \le x_2$. Any pair $(x_1, x_2) \in [0, 1]^2$ produces a strictly monotonic $x(q)$ mapping.

### 3.3 Parametric Formulation

A cubic Bézier segment with endpoints $P_0 = (0, 0)$ and $P_3 = (1, 1)$ and control points $P_1 = (x_1, y_1)$ and $P_2 = (x_2, y_2)$ is parameterized by $q \in [0, 1]$:

$$x(q) = 3(1-q)^2 q \, x_1 + 3(1-q) q^2 \, x_2 + q^3$$
$$y(q) = 3(1-q)^2 q \, y_1 + 3(1-q) q^2 \, y_2 + q^3$$

Given normalized time progress $u = \frac{t - t_{\text{left}}}{t_{\text{right}} - t_{\text{left}}} \in [0, 1]$:
1. Solve for parameter $q \in [0, 1]$ such that $x(q) = u$.
2. Compute eased progress $e = y(q) \in [0, 1]$.
3. Compute the interpolated scalar property value:
   $$V(t) = V_{\text{left}} + e \cdot (V_{\text{right}} - V_{\text{left}})$$

### 3.4 Canonical 32-Iteration Bisection Solver

To guarantee 100% bit-exact parity across preview, export, automated tests, and platforms without numerical divergence:

1. **Endpoint Bypass**:
   - If $u \le 0.0$, return $V_{\text{left}}$ exactly (bypass solver).
   - If $u \ge 1.0$, return $V_{\text{right}}$ exactly (bypass solver).
2. **Fixed 32-Iteration Bisection**:
   - Initialize search bracket $[q_{\min}, q_{\max}] = [0.0, 1.0]$.
   - For exactly 32 iterations:
     $$q_{\text{mid}} = \frac{q_{\min} + q_{\max}}{2}$$
     $$x_{\text{test}} = x(q_{\text{mid}})$$
     $$\text{if } x_{\text{test}} < u \text{ then } q_{\min} = q_{\text{mid}} \text{ else } q_{\max} = q_{\text{mid}}$$
   - Precision: $2^{-32} \approx 2.33 \times 10^{-10}$ in parameter space $q$.
3. **Evaluation**:
   - Evaluate $e = y\left(\frac{q_{\min} + q_{\max}}{2}\right)$.

No third-party numerical library, CSS bezier timing function, or FFmpeg internal expression solver is authorized as canonical truth.

---

## 4. Property Domain Semantics

### 4.1 Supported Channels

Phase 14 retains exactly the 7 existing keyframe-capable properties:

1. `PositionX` (pixels, canvas coordinate space)
2. `PositionY` (pixels, canvas coordinate space)
3. `ScaleX` (scalar multiplier, $1.0 = 100\%$)
4. `ScaleY` (scalar multiplier, $1.0 = 100\%$)
5. `Rotation` (degrees)
6. `Opacity` (scalar, $0.0 \dots 1.0$)
7. `Volume` (linear gain multiplier, $0.0 \dots 2.0$)

No new keyframe properties are introduced in Phase 14.

### 4.2 Interpolation Behavior Rules

- **Universal Scalar Evaluator**: The same canonical scalar evaluator evaluates all 7 channels.
- **Rotation Numeric Interpolation**: Rotation interpolates strictly as raw numeric degrees (e.g., $350^\circ \to 10^\circ$ interpolates downwards through $180^\circ$, not shortest-path $+20^\circ$). Shortest-path angle wrapping is deferred.
- **No Implicit Clamping in Interpolator**: The interpolation layer does not clamp intermediate values (e.g. opacity outside $[0, 1]$ or negative scale). Validation of initial keyframe values remains the responsibility of the domain/command boundary.

---

## 5. Canonical Time Semantics & Keyframe Address

### 5.1 Semantic Address Invariant

Keyframes are clip-local and lack stable unique identifiers. A keyframe is uniquely and semantically addressed by the tuple:

$$\text{Address} = (\text{Property}, \text{Semantic } \text{RationalTime } \text{Offset})$$

### 5.2 Mixed-Rate Semantic Equality

- Comparing two keyframe offsets $T_A = (v_A, r_A)$ and $T_B = (v_B, r_B)$ across potentially different timebases MUST evaluate cross-multiplied semantic equality:
  $$v_A \cdot r_B == v_B \cdot r_A$$
- Keyframe lookup, ordering, insertion, and mutation code MUST NEVER compare raw `.offset.value` unless both timestamps are already proven or rescaled to the identical timebase.
- Global `RationalTime::eq` / `PartialEq` represents representation equality and remains unchanged by design. All keyframe operations must use semantic comparison helpers (`is_same_time`, `cmp_semantic`).
- Keyframes with identical semantic offsets under differing rates represent the same keyframe address.

---

## 6. Command Mutation Semantics

All keyframe operations route exclusively through the authoritative `CommandService`. Automation and GUI clients use identical command channels.

### 6.1 `ClipKeyframeAdd` (Upsert Semantics)

Existing user-facing upsert behavior is strictly preserved:
1. **New Offset**: If no keyframe exists at the semantic offset for that property, insert the keyframe. If `outgoing_easing` is omitted by the caller, it defaults to `KeyframeEasing::Linear`.
2. **Existing Offset**: If a keyframe already exists at that semantic offset, update its value. If the caller omits easing in the replacement command, **preserve the existing keyframe's easing**.
3. **Last Keyframe Invariant**: If the added/updated keyframe is the rightmost keyframe for that property, its `outgoing_easing` must be `KeyframeEasing::Linear`.

### 6.2 `ClipKeyframeMove`

1. Looks up keyframe at source semantic offset.
2. If source and destination offsets are semantically identical, operation is a no-op.
3. The moving payload carries `(value, outgoing_easing)`.
4. If a keyframe already exists at the destination semantic offset, it is replaced by the moving keyframe.
5. Re-sorts keyframe array by semantic time.

### 6.3 `ClipKeyframeEasingSet`

Explicit command to update easing:
1. Target keyframe is identified by `(item_id, property, semantic_offset)`.
2. If targeted keyframe is the last (rightmost) keyframe of the property, setting any non-linear easing rejects with `LAST_KEYFRAME_EASING_MUST_BE_LINEAR`.
3. If control points violate $[0.0, 1.0]$ or contain non-finite numbers, command rejects with `INVALID_KEYFRAME_EASING_PARAMETERS`.

---

## 7. Clip Lifecycle Semantics: Split and Trim

### 7.1 Split Semantics (Subdivision & Rebase)

When a clip is split at clip-local offset $S$:
1. **Left Fragment ($t \in [0, S]$)**:
   - Retains original `TimelineItem.id`.
   - Keyframes with $K < S$ remain on the left fragment with unchanged offsets.
2. **Right Fragment ($t \in [S, \text{duration}]$)**:
   - Receives a freshly generated `TimelineItem.id`.
   - Keyframes with $K > S$ migrate to the right fragment with offsets rebased:
     $$K_{\text{new}} = K - S$$
3. **Exact Cut Boundary ($K == S$)**:
   - Boundary keyframe is represented on both fragments:
     - Left fragment: keyframe at offset $S$ with value $V(S)$ and `outgoing_easing = Linear`.
     - Right fragment: keyframe at offset $0$ with value $V(S)$ and outgoing easing inherited from the original keyframe.
4. **Segment Crossing (Subdivision via de Casteljau)**:
   If split point $S$ falls strictly inside an interpolation interval $(K_i, K_{i+1})$:
   - Sample exact boundary value $V(S)$ using the canonical 32-iteration solver.
   - Synthesize a boundary keyframe at offset $S$ on the left fragment and offset $0$ on the right fragment, both with value $V(S)$.
   - **Easing Subdivision**:
     - If original segment is `Linear`: both left and right outgoing segments remain `Linear`.
     - If original segment is `EaseIn`, `EaseOut`, `EaseInOut`, or `CubicBezier`:
       1. Compute split parameter $q_S \in (0, 1)$ such that $x(q_S) = u_S = \frac{S - K_i}{K_{i+1} - K_i}$ using the 32-iteration bisection solver.
       2. Apply de Casteljau algorithm at $q_S$ to split the Bézier curve into two subcurves $C_{\text{left}}$ and $C_{\text{right}}$.
       3. Normalize subcurve control points into the unit square $[0, 1] \times [0, 1]$.
       4. Left boundary key inherits $C_{\text{left}}$ as `KeyframeEasing::CubicBezier`.
       5. Right keyframe at offset $0$ inherits $C_{\text{right}}$ as `KeyframeEasing::CubicBezier`.
       *(Note: Preset curves naturally subdivide into general `CubicBezier` instances).*

### 7.2 Trim Semantics (Hidden State Retention)

Trimming must never silently discard user animation work.

1. **Tail Trim by $\Delta L$**:
   - Clip duration is reduced: $D_{\text{new}} = D - \Delta L$.
   - All keyframes remain on the clip with unchanged offsets.
   - Keyframes with offset $> D_{\text{new}}$ become **hidden keyframes** (outside visible playback range).
   - Extending the tail rightward restores these keyframes to visibility and evaluation without data loss.
2. **Head Trim by $H$**:
   - Clip source start advances: $\text{source\_start}_{\text{new}} = \text{source\_start} + H$.
   - Clip duration is reduced: $D_{\text{new}} = D - H$.
   - Every keyframe offset is shifted by $-H$:
     $$K_{\text{new}} = K - H$$
   - Keyframes with $K_{\text{new}} < 0$ become **hidden preroll keyframes** (canonical negative offsets are valid persisted domain state).
   - Extending the head leftward shifts offsets back by $+H$, restoring hidden keyframes.
   - This ensures visible motion paths on the timeline do not slip relative to source media frames during trim.
3. **Transaction Atomicity**:
   - Trim operation and dependent keyframe offset updates commit atomically in a single command history transaction.

---

## 8. Schema v3 Promotion & Migration Contract

### 8.1 Necessity of Schema v3

Persisting easing metadata on keyframes necessitates upgrading the project schema to version 3.

```rust
// Keyframe Model in Schema v3
pub struct Keyframe {
    pub property: KeyframeProperty,
    pub offset: RationalTime,
    pub value: f64,
    pub outgoing_easing: KeyframeEasing, // New in Schema v3
}
```

**Reasoning**: If easing fields were added under `schema_version = 2`, an older editor binary (from Phase 13) could open the file, drop unknown fields during deserialization, and upon save, silently overwrite the project with all easing permanently flattened to linear. Bumping to Schema v3 forces older binaries to reject the file with `UnsupportedSchemaVersion(3)`, protecting user project integrity.

### 8.2 Migration from Schema v2 to Schema v3

Migration is deterministic and lossless:
1. Validate `schema_version == 2`.
2. Preserve all project metadata, assets, track topology, and timeline items.
3. Preserve all ADR-003 `Track.transitions` (`CutTransition`) losslessly.
4. Preserve all item-level `TimelineItem.transition` (`FadeIn`, `FadeOut`, `Both`) losslessly.
5. For every keyframe across all timeline items:
   - Retain `property`, `offset`, and `value`.
   - Set `outgoing_easing = KeyframeEasing::Linear`.
6. Set `schema_version = 3`.
7. Existing projects open with zero visual change or curve drift.

---

## 9. Full Export Architecture: `ExportAutomationSchedule`

### 9.1 Data Flow Architecture

Export animation adheres strictly to the decoupled evaluation pattern:

```text
Project / Composition Keyframes
              ↓
  Canonical Rust Evaluator (32-iter bisection)
              ↓
   ExportAutomationSchedule (in-memory / ephemeral)
              ↓
FFmpeg Runtime Command Dispatch (literal values only)
```

`ExportAutomationSchedule` is a build-time execution artifact. It is never serialized into saved project JSON and contains zero interpolation formulas.

### 9.2 Video Literal Command Application

1. **Evaluation Grid**: Rust evaluates animated visual properties at every output frame RationalTime:
   $$t_n = \frac{n}{\text{export\_fps}}$$
2. **Initial Frame (Frame 0)**:
   - Initial values for Position, Scale, Rotation, and Opacity become static initial parameters in the FFmpeg filter graph string.
3. **Runtime Dispatch ($n \ge 1$)**:
   - When dynamic keyframes exist, Rust writes a sendcmd script file.
   - Midpoint Dispatch Strategy: Commands for frame $n$ are scheduled at $t_{\text{mid}} = \frac{t_{n-1} + t_n}{2}$ to prevent edge-timestamp jitter.
   - Supported Filter Mappings:
     - `PositionX`, `PositionY` $\to$ `overlay` filter runtime commands (`x`, `y`).
     - `ScaleX`, `ScaleY` $\to$ `scale` filter runtime commands (`w`, `h`).
     - `Rotation` $\to$ `rotate` filter runtime commands (`a` in radians).
     - `Opacity` $\to$ `colorchannelmixer` filter runtime commands (`aa`).
4. **No FFmpeg Solvers**: FFmpeg filter expressions do not evaluate cubic curves, Béziers, or linear segments. FFmpeg only executes literal value updates.

### 9.3 Audio Sample-Accurate Volume Envelope

1. **Evaluation Grid**: Rust evaluates volume keyframes across the exact audio sample rate grid (e.g., 48,000 Hz).
2. **Streamed Binary Envelope**:
   - Volume envelope is written to a temporary binary file as raw 32-bit floating point PCM (`f32le`, 1 channel, 48 kHz).
   - Envelope generation streams in $O(1)$ memory buffer chunks.
3. **Filter Application**:
   - FFmpeg ingests the binary envelope via `eval=all` or audio `amultiply` filter (`[audio_in][envelope]amultiply[out]`).
4. **Lifecycle & Disk Preflight**:
   - Export preflights available disk space before generating envelopes (e.g. 60-minute 48kHz mono f32le envelope requires $\approx 691.2 \text{ MB}$).
   - Envelope file is automatically deleted upon export success, failure, or cancellation.

---

## 10. Coexistence with ADR-003 CrossDissolve

Full export automation must preserve all ADR-003 adjacent-cut transition semantics:

1. **Topology Preservation**: Adjacent cut cross-dissolves remain cut-owned in `Track.transitions` with zero timeline duration mutation.
2. **Filtergraph Routing**:
   - Outside the transition window $[T_0, T_1)$, dynamic keyframe commands route to ordinary clip streams.
   - Inside the transition window $[T_0, T_1)$, dynamic keyframe commands route to the corresponding transition participant streams entering the premultiplied `xfade` block:
     $$\text{Participant Streams} \to \text{Keyframe Transform} \to \text{Premultiply} \to \text{xfade=fade:offset=0} \to \text{Unpremultiply}$$
   - Ordinary participant streams remain suppressed during $[T_0, T_1)$.
3. **Parity**: Cross-dissolve premultiplied blend invariant $(1-p)A^* + pB^*$ is preserved identically.

---

## 11. Architectural Validation & Proof of Concept Evidence

The Phase 14 export automation architecture was proven through a dedicated test harness (`src-tauri/tests/export_automation_poc.rs`) targeting the app-owned bundled FFmpeg 9.0.2 binary.

| Test ID | Gate Verification Requirement | Result | Evidence / Measurement |
|---|---|---|---|
| **POC-Preflight** | Bundled FFmpeg 9.0.2 resolution | **PASS** | Resolves `MediaBinaryResolver` packaged binary |
| **POC-V1** | Runtime command capability on FFmpeg 9.0.2 | **PASS** | Verified `sendcmd` runtime updates on `scale`, `overlay`, `rotate` |
| **POC-V2** | Frame-exact dispatch timing | **PASS** | Midpoint dispatch verified at 30 fps and fractional 30000/1001 fps |
| **POC-V3** | Visual correctness & negative control | **PASS** | MAE $\le 2.0$ against reference; negative control detects intentional 1-frame offset |
| **POC-V4** | ADR-003 CrossDissolve coexistence | **PASS** | Animated clips dissolve cleanly across cut with zero filter graph conflict |
| **POC-A1** | Sample-accurate audio envelope | **PASS** | `amultiply` audio gain verified; negative control detects intentional discrepancy |
| **POC-A2** | Envelope resource lifecycle | **PASS** | Preflight space validation verified; deterministic cleanup on cancel/error |
| **POC-P1** | Performance sanity & overhead | **PASS** | 2.0s fixture rendered in 265 ms (1.74x vs static); 60-min envelope preflight proven |

---

## 12. Scope Boundaries & Deferred Items

The following capabilities are explicitly deferred and remain outside Phase 14:

- **Overshoot / Anticipation Béziers**: Curves with $y < 0.0$ or $y > 1.0$ (deferred to future animation curve updates).
- **Physics / Springs / Gravity Simulation**: Deferred.
- **Hold / Step Keyframe Mode**: Deferred.
- **Motion Paths / Spatial Tangents**: Multi-dimensional curve paths deferred.
- **Shortest-Path Angle Rotation**: Deferred; rotation remains raw scalar degree interpolation.
- **Expressions & Automation Scripting**: Deferred.
- **General Graph / Curve Editor UI**: Phase 14 UI is restricted to inspector preset pickers and cubic coordinate inputs.
- **Transition Easing Curves**: Easing on `CutTransition` (ADR-003) remains linear progress $p(t) = \frac{t-T_0}{D}$.
- **Audio Crossfades**: Audio across cuts remains butt-cut; `acrossfade` remains deferred.
- **Model Context Protocol (MCP)**: Deferred until after core editing stability.
- **GPU Shader Compositor**: Tauri DOM preview and FFmpeg CPU/GPU export remain standard.
- **Stable Keyframe IDs**: Keyframes remain addressed by property and semantic offset.

---

## 13. Architecture Conformance & Implementation Gates

Before Phase 14 implementation can be accepted into production, code changes must fulfill:

1. **P14-PREREQ-01 Remediation**: Full export must consume dynamic linear keyframes before advanced easing is merged.
2. **Zero Renderer Solver Invariant**: Neither preview frontend nor FFmpeg export may implement proprietary Bézier formulas; both must consume canonical Rust evaluations.
3. **Schema v3 Conformance**: All persistence tests must verify v2 $\to$ v3 lossless forward migration and older binary rejection.
4. **All Tests Green**: Parity fixtures, automated unit tests, clippy, formatting, and build checks must pass cleanly.

---

## 14. Implementation Verification Note

Phase 14 has implemented and verified all decisions in this ADR:

1. **Canonical Evaluator**: Deterministic 32-iteration bisection cubic Bézier solver implemented in `src-tauri/src/modules/domain/evaluator.rs` with exact endpoint handling and bounded unit control points $[0.0, 1.0]^4$. Serves as the single source of truth for both preview and export.
2. **Schema v3**: Project format promoted to Schema v3 (`schema_version = 3`) with `outgoing_easing: KeyframeEasing` on `Keyframe`. Lossless forward migration from Schema v2 implemented and tested (`v2_to_v3`). Older Schema v2 binaries explicitly reject v3 projects.
3. **Split & Trim Lifecycle**: `split_item` implements cut boundary value synthesis, right-fragment rebase by $-S$, and de Casteljau Bézier curve subdivision. Non-destructive head and tail trim preserves keyframe offsets outside visible duration (head trim shifts offsets into canonical negative space; tail trim preserves post-duration keys).
4. **ExportAutomationSchedule**: Decoupled in-memory schedule compiled in `src-tauri/src/modules/export/automation.rs`. FFmpeg filters receive pre-calculated values and execute zero Bézier math.
5. **Video sendcmd Transport**: Visual properties (`PositionX`, `PositionY`, `ScaleX`, `ScaleY`, `Rotation`, `Opacity`) are dispatched via literal frame-midpoint `sendcmd` scripts targeting `overlay`, `scale`, `rotate`, and `colorchannelmixer`.
6. **Audio Sample Envelope**: Volume keyframes compile to an ephemeral sample-accurate mono `f32le` binary gain envelope file modulated via `amultiply`, with preflight disk check, chunked streaming, and deterministic temp cleanup. Supported export sample rates are 48,000 Hz and 44,100 Hz.
7. **CrossDissolve Coexistence**: Automated tests verify that animated clips dissolve cleanly across ADR-003 adjacent cuts with runtime commands routing to participant streams entering the premultiplied `xfade` block without filtergraph conflict.
8. **Semantic RationalTime Remediation**: Keyframe addresses, lookups, and upsert operations evaluate cross-multiplied canonical rational equality, completely eliminating tick-comparison mismatches across different timebases.
