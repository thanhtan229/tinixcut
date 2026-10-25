# Animated Text Templates — Canonical Contract Semantics

**Document Version:** 1.2 (Phase 27 Deliverable — Complete Normative Contract)  
**Program:** Tinix Animated Text Templates (Phases 27–36)  
**Authority:** Companion specification to `ARCHITECTURE.md` and `Plans/plan_phase27_template_contract.md`.

---

## 1. Units, Coordinate Systems & Transform Hierarchy

1. **Time Domain**:
   - Canonical timeline time MUST strictly use `RationalTime { value: i64, rate: FrameRate }`, where `FrameRate { numerator: u32, denominator: u32 }` (e.g. 30/1, 24/1, 60/1, 24000/1001, 30000/1001).
   - Animation phase/progress is evaluated in continuous local rational time: `t_local = t_timeline - clip_start`.
   - Never use arbitrary floating-point seconds as canonical project keys or serialized time values.

2. **Spatial Coordinate System & Units**:
   - Canvas coordinate space: Top-left origin `(0, 0)`, positive X rightward, positive Y downward.
   - Reference canvas: Standard coordinates and dimensions are expressed in nominal project canvas pixels (e.g. 1920×1080, 1080×1920, 1080×1080, 1080×1350).
   - Font sizes, stroke widths, shadow distances, blur radii, background padding, and corner radii are specified in nominal reference canvas pixels `px`.
   - Letter-spacing (tracking) is specified as an offset in font `em` ratio or canvas `px`.
   - Line-height is specified as a multiplier relative to nominal font size (e.g. 1.2).
   - Responsive scaling: When a template instance is applied across different project canvas dimensions or aspect ratios, layer bounds map through normalized coordinate anchors `(anchor_x: [0.0..1.0], anchor_y: [0.0..1.0])` with pivot offsets.
   - Conversion rule for CapCut normalized units (e.g. stroke width `0.03808219`): CapCut stroke widths are relative to font canvas height (`nominal_font_size * 0.03808219 * scale`).

3. **Three-Tier Transform Hierarchy & Multiplication Order**:
   - To satisfy `plan_phase29_rich_text_domain.md` Section 29.1, composite glyph transform in world/canvas coordinates is evaluated through three strictly ordered transform layers:
     $$\mathbf{M}_{\text{final}}(t, i) = \mathbf{M}_{\text{item}}(t) \times \mathbf{M}_{\text{layer}}(t) \times \mathbf{M}_{\text{unit}}(t, i)$$
     where:
     - $\mathbf{M}_{\text{item}}(t)$: Timeline item transform (editorial track position, uniform scale, rotation, whole-item keyframes).
     - $\mathbf{M}_{\text{layer}}(t)$: Template layer-level transform (relative placement, slot anchor, responsive offset).
     - $\mathbf{M}_{\text{unit}}(t, i)$: Per-animation-unit transform (per-grapheme/word/line stagger, bounce, rotation, opacity at index $i$).
   - Within each 2D affine matrix tier, transformation order is:
     $$\mathbf{M} = \mathbf{T}(\text{Position}) \times \mathbf{T}(\text{Pivot}) \times \mathbf{R}(\text{Rotation}) \times \mathbf{S}(\text{Scale}) \times \mathbf{T}(-\text{Pivot})$$
   - Angles are specified in degrees clockwise (with positive rotation matching screen coordinate orientation).
   - Scale factor 1.0 = 100% nominal dimensions.

4. **Colors, Alpha & Blending**:
   - Canonical color representation: Normalized sRGB with linear alpha `[R, G, B, A]` where each channel is in `[0.0, 1.0]`, serializable to standard hex `#RRGGBBAA`.
   - Alpha compositing: Preview and export overlays MUST specify explicit alpha channel semantics (premultiplied alpha for hardware/Skia rasterization; un-premultiplied straight alpha for PNG/filtergraph interchange). No black halos around soft edges.

---

## 2. Text Style, Painter's Order & Appearance Hierarchy

1. **Painter's Rendering Order (Back-to-Front)**:
   Within any text layer, visual primitives are rasterized in strict bottom-to-top painter's order:
   1. **Background Box**: Rendered lowest, covering the text layout bounds expanded by `padding_x`, `padding_y`, with corner radius `border_radius` and fill (solid or gradient).
   2. **Drop Shadows & Glows**: Rendered in array order `[0..k-1]`, positioned behind glyph strokes and fills. Outer bounds MUST include shadow blur radius and offset distance to prevent premature bounding box clipping.
   3. **Outer Strokes**: Layered strokes rendered in array order `[0..m-1]`. Strokes are drawn behind the glyph fill so thick outlines expand outward without shrinking the readable letterform fill.
   4. **Glyph Fill**: Rendered above strokes. Supports:
      - Solid color fill (`color_rgba`).
      - Linear / Radial gradient fill (up to 8 stops with angle/radius). Coordinate space mode supports:
        - `mode: "all"`: Gradient spans the entire bounding box of the text container.
        - `mode: "character"`: Gradient is evaluated locally across each individual glyph's bounding box.
   5. **Inner Strokes / Inner Shadows**: If configured, applied within glyph interior mask.

2. **Character and Text Span Semantics**:
   - A "character" for layout, indexing, and animation stagger is strictly defined as an **Extended Grapheme Cluster** (Unicode Standard Annex #29).
   - Multi-codepoint sequences (such as Vietnamese base + combining tone marks `ê` + `\u0301`, emoji zero-width-joiner sequences `👨‍👩‍👧‍👦`, and regional indicator flags) MUST remain a single indivisible unit during character-level animation. Tone marks MUST NOT detach or lag behind the vowel base.
   - Normalization: Input text is normalized via Unicode NFC prior to cluster segmentation.

3. **Rich Text Span Indexing & Rebinding Invariant**:
   - All rich text span intervals `[start, end)` MUST use **Grapheme Indices** (0-based count of extended grapheme clusters), NOT UTF-8 byte offsets and NOT UTF-16 code units.
   - Rust and TypeScript domain representations MUST share this exact same grapheme-count invariant.
   - **Rebinding Policy on Text Edit (`ClipTextSet`)**: When text content is replaced ($N_{\text{old}} \to N_{\text{new}}$ graphemes):
     - Spans are deterministically clamped to `[0, N_new]`.
     - Spans that were full-text `[0, N_old)` automatically expand to `[0, N_new)`.
     - Spans whose start index exceeds $N_{\text{new}}$ or collapses to zero width `[k, k)` outside user intent are safely pruned or adjusted. No stale or out-of-bounds indices are ever retained.

4. **Font Resolution and Fallback**:
   - Primary font identity is resolved via explicit font asset ID or family name with weight/style specifiers.
   - Missing font policy: If a specified font is unavailable on the local system, the engine:
     a) flags the missing asset in project diagnostic state;
     b) falls back to a deterministic bundled system font (e.g. Noto Sans / Arial) for preview;
     c) MUST NOT silently export altered typography without explicit user confirmation or automated fallback command.

---

## 3. Layout and Boundary Behavior

1. **Empty and Whitespace Strings**:
   - An empty string `""` or whitespace-only string produces zero rendered glyphs, zero bounding box, and does not fail or panic the layout pipeline.
   - Stagger evaluation over empty text safely evaluates to an empty timeline segment.

2. **Overflow, Wrapping, and Auto-Fit**:
   - Mode `Wrap`: Text breaks at whitespace or grapheme cluster boundaries when exceeding the designated bounding box width (`line_max_width`).
   - Mode `AutoFit`: Font size dynamically scales down between `min_font_size` and `max_font_size` until the text fits within the designated boundary. If text still exceeds at `min_font_size`, wrapping or clipping is applied based on clip configuration.
   - Text alignment: Left, Center, Right, Justified within the computed layout container.

3. **Shadow and Glow Bounds**:
   - Computed layer bounding boxes MUST account for stroke width, shadow blur radii, and offset distances to ensure overlay buffers are not prematurely clipped at the tight text boundary.

---

## 4. Animation Lifecycle, Units & Lifecycle Rules

1. **Segment Model**:
   - Each animated template instance defines up to three animation segments:
     - **In-Animation** (duration $D_{\text{in}}$)
     - **Loop-Animation** (duration $D_{\text{loop}}$, recurring during the middle phase)
     - **Out-Animation** (duration $D_{\text{out}}$)
     - **Hold / Steady Phase**: The interval between In-Animation completion and Out-Animation start.

2. **Animation Units & Stagger Evaluation**:
   - Supported segmentation units (`AnimationUnit`):
     - `All`: Whole text layer moves synchronously.
     - `Grapheme`: Extended grapheme clusters animate sequentially (per-character).
     - `Word`: Whitespace-separated words animate sequentially.
     - `Line`: Wrapped or newline-separated lines animate sequentially.
   - Stagger order modes:
     - `Forward`: Index $0 \to N-1$.
     - `Backward`: Index $N-1 \to 0$.
     - `CenterOut`: Middle index out to ends.
     - `Random`: Deterministic pseudo-random permutation governed strictly by a required `seed: u64` stored in domain state (ensuring 100% reproducibility across preview and export).

3. **Loop Animation Modes (`LoopMode`)**:
   - `None`: No animation during steady phase.
   - `Repeat`: Progress restarts at boundary: $p(t) = (t_{\text{local}} - D_{\text{in}}) \bmod D_{\text{loop}} / D_{\text{loop}}$.
   - `PingPong`: Alternates forward and backward cycles with continuous velocity.

4. **Duration Collision Policy**:
   - Let total clip duration be $D_{\text{clip}}$.
   - If $D_{\text{in}} + D_{\text{out}} \le D_{\text{clip}}$:
     - In-Animation plays at natural tempo $[0, D_{\text{in}}]$.
     - Steady/Loop phase plays during $[D_{\text{in}}, D_{\text{clip}} - D_{\text{out}}]$.
     - Out-Animation plays during $[D_{\text{clip}} - D_{\text{out}}, D_{\text{clip}}]$.
   - If $D_{\text{in}} + D_{\text{out}} > D_{\text{clip}}$:
     - **Default Policy (`Reject`)**: The Command Service **rejects** the command with a validation error (`DurationConflict: InDuration + OutDuration exceeds ClipDuration`). The engine MUST NOT invent arbitrary hidden tempo changes or silently warp playback rate.
     - **Explicit Policy (`ProportionalScale`)**: When caller explicitly passes `conflict_policy: "ProportionalScale"`, the available duration $D_{\text{clip}}$ is proportionally divided:
       $$D'_{\text{in}} = D_{\text{clip}} \times \left(\frac{D_{\text{in}}}{D_{\text{in}} + D_{\text{out}}}\right), \quad D'_{\text{out}} = D_{\text{clip}} - D'_{\text{in}}$$
       Steady phase becomes zero.
     - **Explicit Policy (`ClampOut`)**: When caller explicitly passes `conflict_policy: "ClampOut"`, $D_{\text{in}}$ is preserved and $D_{\text{out}} = \max(0, D_{\text{clip}} - D_{\text{in}})$.

5. **Timeline Editorial Invariants (Move, Trim, Split, Stretch)**:
   - **Move**: Translating a clip on the timeline shifts absolute start/end; clip-local animation evaluation $t_{\text{local}} = t - t_{\text{start}}$ is invariant.
   - **Trim Left / Right**: Trimming an edge adjusts the visible editorial window. Animation evaluation preserves visible phase via explicit `animation_time_offset`.
   - **Split**: Splitting a clip clones the template instance and adjusts segment enablement or offsets deterministically. Splitting does NOT restart entrance stagger on the second half.
   - **Stretch Duration**: Extending clip duration expands the steady/loop phase, maintaining original authored $D_{\text{in}}$ and $D_{\text{out}}$ tempos.

---

## 5. Domain Identity, Pack Security & Validation Bounds

1. **Template Definition vs. Instance vs. Overrides**:
   - **Definition (Immutable / Versioned)**: Stored in library registry (`(template_id, version)`). Defines default layers, parameter slots, and motion curves.
   - **Instance (Project-Local)**: Stored in `TimelineItem.text_template: Option<TextTemplateReference>`. Owns a snapshot of resolved layer properties and explicit parameter bindings. Renders identically even if the template pack is uninstalled.
   - **User Overrides**: Stored directly on instance fields. Command `ClipTextTemplateReset` restores modified values back to definition defaults.

2. **Typed Parameter Bindings Whitelist**:
   Parameter bindings exposed to UI/automation are strictly whitelisted to typed paths:
   - `TextContent`, `FontFamily`, `FontSize`, `FontWeight`, `FillColor`, `GradientStops`, `StrokeWidth`, `StrokeColor`, `ShadowColor`, `ShadowDistance`, `AnimationInDuration`, `AnimationOutDuration`.
   - Arbitrary script expressions or untyped path traversal are strictly prohibited.

3. **Template Pack Archive (`.tinixtpl`) Safety Limits**:
   To prevent zip-bombs, path traversal attacks, and resource exhaustion:
   - Max archive file size: 100 MiB.
   - Max decompressed size: 250 MiB.
   - Max files per pack: 128.
   - Path traversal rejection: Any entry with `..`, absolute paths (`/` or `C:\`), or symbolic links is immediately rejected.
   - All bundled fonts and images MUST match SHA-256 digests declared in `manifest.json`.

4. **Structural Validation Limits (Command Service Hard Guardrails)**:

| Resource Dimension | Hard Limit | Enforcement Policy |
| :--- | :--- | :--- |
| Max Layers per Template Instance | 16 | Command rejected if exceeded |
| Max Text Slots per Instance | 8 | Command rejected if exceeded |
| Max Grapheme Clusters per Text Slot | 512 | Command rejected if exceeded |
| Max Strokes per Text Slot | 4 | Command rejected if exceeded |
| Max Shadows / Glows per Text Slot | 4 | Command rejected if exceeded |
| Max Gradient Color Stops | 8 | Command rejected if exceeded |
| Max Nesting Depth of Layer Groups | 1 (Flat) | Tree structures prohibited |

*Note: The Command Service rejects inputs exceeding these limits with descriptive error messages; it NEVER silently truncates user text.*
