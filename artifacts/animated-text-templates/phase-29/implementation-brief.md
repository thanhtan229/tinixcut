# Phase 29 — Locked Implementation Brief (Rich Text Domain, Migration v10, Commands)

**Owner:** 01 Boss (contract) · 02 Backend (Rust) · 03 Frontend (TS types)
**Status:** IMPLEMENTING
**Authority:** `Plans/plan_phase29_rich_text_domain.md`, `artifacts/animated-text-templates/phase-27/contract-semantics.md`, `artifacts/animated-text-templates/phase-27/compatibility-matrix.json`

This brief is the interface contract. Do not invent parallel types, renames, or a second
mutation path. If a rule here is wrong, stop and report with evidence before deviating.

Scope discipline:
- No UI components (Phase 33), no renderer/rasterizer changes (Phase 30/31), no template
  packs or CapCut importer (Phase 32/34).
- No new crates. `unicode-segmentation` and `unicode-normalization` are already dependencies.
- No comments in code unless they explain a non-obvious invariant.
- Preserve all uncommitted working-tree work; never revert unrelated files.
- Schema stays additive and lossless. No ADR is required for Phase 29 (additive v10 only).

---

## 1. Domain types (`src-tauri/src/modules/domain/text.rs`, new module)

Serialization is Rust default snake_case, matching the existing project model.
Constants: `MAX_TEXT_GRAPHEMES = 512`, `MAX_TEXT_STROKES = 4`, `MAX_TEXT_SHADOWS = 4`,
`MAX_GRADIENT_STOPS = 8`.

```rust
pub fn grapheme_count(text: &str) -> usize; // UnicodeSegmentation::graphemes(text, true).count()

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FontStyle { #[default] Normal, Italic, Oblique }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontReference {
    pub family: String,
    pub weight: u16,
    #[serde(default)] pub style: FontStyle,
    #[serde(default)] pub asset_id: Option<String>,
}
// Default: family "Inter", weight 400, Normal, None

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextWrapMode { #[default] None, Wrap, AutoFit }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextLayout {
    #[serde(default)] pub max_width: Option<f32>,
    #[serde(default)] pub alignment: TextAlignment, // reuse existing enum
    #[serde(default = "default_line_height")] pub line_height: f32, // 1.0
    #[serde(default)] pub letter_spacing: f32,
    #[serde(default)] pub wrap_mode: TextWrapMode,
    #[serde(default)] pub min_font_size: Option<f32>,
    #[serde(default)] pub max_font_size: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GradientKind { #[default] Linear, Radial }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GradientSpace { #[default] All, Character }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStop { pub offset: f32, pub color: String }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind")]
pub enum TextFill {
    Solid { color: String },
    Gradient {
        gradient_kind: GradientKind,
        angle_deg: f32,
        #[serde(default)] radius: Option<f32>,
        #[serde(default)] space: GradientSpace,
        stops: Vec<GradientStop>,
    },
}
// Default: Solid { color: "#FFFFFF" }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StrokeAlign { #[default] Outside, Center, Inside }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStroke { pub color: String, pub width: f32, #[serde(default)] pub align: StrokeAlign }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextShadow {
    pub color: String,
    #[serde(default)] pub offset_x: f32,
    #[serde(default)] pub offset_y: f32,
    #[serde(default)] pub blur: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBackgroundBox {
    #[serde(default)] pub fill: TextFill,
    #[serde(default)] pub padding_x: f32,
    #[serde(default)] pub padding_y: f32,
    #[serde(default)] pub corner_radius: f32,
    #[serde(default = "default_unit_opacity")] pub opacity: f32, // 1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextSpanStyle {
    #[serde(default)] pub fill: Option<TextFill>,
    #[serde(default)] pub font: Option<FontReference>,
    #[serde(default)] pub font_size: Option<f32>,
    #[serde(default)] pub letter_spacing: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichTextSpan { pub start: u32, pub end: u32, #[serde(default)] pub style: TextSpanStyle }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextRichStyle {
    #[serde(default)] pub font: Option<FontReference>,
    #[serde(default)] pub font_size: Option<f32>,
    #[serde(default)] pub layout: TextLayout,
    #[serde(default)] pub fill: TextFill,
    #[serde(default)] pub strokes: Vec<TextStroke>,
    #[serde(default)] pub shadows: Vec<TextShadow>,
    #[serde(default)] pub background: Option<TextBackgroundBox>,
    #[serde(default)] pub spans: Vec<RichTextSpan>,
}
```

Animation definition (storage only; evaluator is Phase 31):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AnimationUnit { #[default] All, Grapheme, Word, Line }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StaggerOrder { #[default] Forward, Backward, CenterOut, Random }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoopMode { #[default] None, Repeat, PingPong }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DurationConflictPolicy { #[default] Reject, ProportionalScale, ClampOut }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimationTransform {
    pub translate_x: f32, pub translate_y: f32,
    pub scale_x: f32, pub scale_y: f32,
    pub rotation: f32, pub opacity: f32,
}
// Default: 0,0,1,1,0,1

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAnimationSegment {
    pub duration: RationalTime,
    #[serde(default)] pub easing: KeyframeEasing,
    #[serde(default)] pub stagger_ratio: f32,
    #[serde(default)] pub from: AnimationTransform,
    #[serde(default)] pub to: AnimationTransform,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextAnimation {
    #[serde(default)] pub unit: AnimationUnit,
    #[serde(default)] pub order: StaggerOrder,
    #[serde(default)] pub seed: Option<u64>,
    #[serde(default)] pub pivot_x: f32, // 0.5 default, range [0,1]
    #[serde(default)] pub pivot_y: f32, // 0.5 default, range [0,1]
    #[serde(default)] pub in_anim: Option<TextAnimationSegment>,
    #[serde(default)] pub loop_anim: Option<TextAnimationSegment>,
    #[serde(default)] pub out_anim: Option<TextAnimationSegment>,
    #[serde(default)] pub loop_mode: LoopMode,
    #[serde(default)] pub conflict_policy: DurationConflictPolicy,
}
```
Use explicit `Default` impl when a non-zero/0.5 default is required (`pivot_x/pivot_y = 0.5`).

### 1.1 Validation rules (return `DomainError`)

- `grapheme_count(text) <= MAX_TEXT_GRAPHEMES` for the owning item's text; exceeding is rejected,
  never truncated.
- Colors: `#RRGGBB` or `#RRGGBBAA` only (reuse the existing helper).
- All numeric values finite. `font_size > 0`; `line_height > 0`; `letter_spacing` finite.
  `stagger_ratio` in `[0,1]`; `pivot_x/pivot_y` in `[0,1]`; opacity in `[0,1]`;
  `stroke.width > 0`; `shadow.blur >= 0`; background padding/radius `>= 0`.
- Font weight in `[100,900]`; `family` non-empty; `asset_id` (when present) must pass `is_valid_id`.
- Max 4 strokes, 4 shadows, 8 gradient stops. Gradient stops: `offset` in `[0,1]`,
  strictly increasing, last stop must be exactly `1.0` unless `stops.is_empty()`.
- Spans use **grapheme indices** `[start, end)`. Reject `start >= end`, `end > grapheme_count(text)`,
  overlapping spans, unsorted spans. Validate every span style field.
- `StaggerOrder::Random` requires `seed.is_some()`.
- `TextAnimation::validate_for_clip_duration(clip_duration)`: rescale every segment duration to the
  clip rate; require positive durations; if `in + out > clip`:
  `Reject` → error, `ProportionalScale` / `ClampOut` → accepted (resolution math is Phase 31).

`TimelineItem::validate` must call rich validation with `text_content.as_deref().unwrap_or("")`
and `validate_for_clip_duration(self.timeline_range.duration)`, and reject rich/animation fields on
non-Text items. This guarantees no invalid rich-text state can be persisted at rest.

Add `DomainError` variants: `TextTooLong { max, found }`,
`InvalidTextSpan { start, end, reason }`,
`AnimationDurationConflict { in_duration, out_duration, clip_duration }` (strings),
`UnsupportedTextFeature(String)`.

Reuse, do not duplicate: existing `TextAlignment`, `TextAnchor`, `TextStyle`,
`KeyframeEasing`, `RationalTime`, `is_valid_id`, `is_hex_color`.
Required minimal edits to `domain/mod.rs`: `pub mod text;`, `pub use text::*;` (or explicit
re-exports), `is_hex_color` becomes `pub(crate)`, and add `#[derive(Default)]` with
`#[default] Center` to `TextAlignment`.

## 2. Persistence — Schema v10

- `CURRENT_SCHEMA_VERSION = 10`.
- `TimelineItem` gains:
  `#[serde(default)] pub text_rich: Option<TextRichStyle>` and
  `#[serde(default)] pub text_animation: Option<TextAnimation>`.
  Both default `None` in `TimelineItem::new` (no animation is invented for legacy templates).
- `run_migrations_if_needed`: add `if version <= 9 && CURRENT_SCHEMA_VERSION >= 10`.
- `migrate_v9_to_v10`: version bump only — do not invent fields, do not rewrite text, preserve
  `text_style`, `text_template`, `text_content`, keyframes, transitions and command ledger
  byte-for-byte.
- Existing atomic save / recovery / future-schema rejection behavior must stay unchanged.

## 3. Command Service (`src-tauri/src/modules/command/mod.rs`)

Add these `CommandOperation` variants (payload fields snake_case):

```rust
ClipTextRichSet { item_id: String, rich: TextRichStyle },
ClipTextSpansSet { item_id: String, spans: Vec<RichTextSpan> },
ClipTextLayoutSet { item_id: String, layout: TextLayout },
ClipTextAppearanceSet {
    item_id: String,
    fill: TextFill,
    strokes: Vec<TextStroke>,
    shadows: Vec<TextShadow>,
    background: Option<TextBackgroundBox>,
},
ClipTextFontSet { item_id: String, font: FontReference, font_size: Option<f32> },
ClipTextAnimationSet { item_id: String, animation: Option<TextAnimation> },
ClipTextRichReset { item_id: String },
```

Semantics:
- Every operation rejects non-Text items and unknown items with `OperationValidationFailed`.
- Validation happens before mutation; a failed operation in a transaction rolls back all
  operations (existing engine behavior — do not add a second path).
- Granular ops (spans/layout/appearance/font/animation) start from
  `item.text_rich.clone().unwrap_or_default()` and update only their own fields.
- `ClipTextRichSet` replaces the whole rich style.
- `ClipTextRichReset` clears `text_rich` only (animation is cleared via
  `ClipTextAnimationSet { animation: None }`).
- `ClipTextAnimationSet` validates against the item's current `timeline_range.duration`.
- All operations return `DiffChange::ClipModified`.
- Affected scope/revision/idempotency receipts/undo-redo come from the existing engine.

`ClipTextSet` rebinding (grapheme indices must never go stale):
- Compute `old_count` before assignment and `new_count` after.
- A span exactly `[0, old_count)` expands to `[0, new_count)`.
- Otherwise clamp `start = min(start, new_count)`, `end = min(end, new_count)`, then drop spans
  where `start >= end`. Never keep out-of-bounds indices.
- Re-validate the resulting rich style against the new text; rejection must fail the whole
  operation.

`TextTrackStyleSet`/`TextTrackTemplateApply`/template reset keep legacy `TextStyle` behavior
unchanged; they do not mutate rich text or animation.

## 4. Composition & automation

`CompositionVisualLayer` and `PreviewSampledLayer` gain
`text_rich: Option<TextRichStyle>` and `text_animation: Option<TextAnimation>`.
Populate for `ItemKind::Text` from the item; `None` for video/image layers.
`sample_visual_layer_at` copies both fields. FFmpeg drawtext fallback and existing preview
behavior stay untouched (renderer work is Phase 30/31).

`AutomationAdapter::supported_operations()` gains the 7 new names. Add read-only capability
helpers returning the new enums (`AnimationUnit`, `StaggerOrder`, `LoopMode`,
`DurationConflictPolicy`) and include them in the capability-discovery payload where the other
capability lists are assembled. No new transport or server runtime.

## 5. Frontend types (TypeScript must mirror Rust exactly)

`src/types/project.ts`:

```ts
export type FontStyle = "Normal" | "Italic" | "Oblique";
export interface FontReference { family: string; weight: number; style?: FontStyle; asset_id?: string | null; }
export type TextWrapMode = "None" | "Wrap" | "AutoFit";
export interface TextLayout {
  max_width?: number | null; alignment?: TextAlignment; line_height?: number;
  letter_spacing?: number; wrap_mode?: TextWrapMode;
  min_font_size?: number | null; max_font_size?: number | null;
}
export type GradientKind = "Linear" | "Radial";
export type GradientSpace = "All" | "Character";
export interface GradientStop { offset: number; color: string; }
export type TextFill =
  | { kind: "Solid"; color: string }
  | { kind: "Gradient"; gradient_kind: GradientKind; angle_deg: number; radius?: number | null; space?: GradientSpace; stops: GradientStop[] };
export type StrokeAlign = "Outside" | "Center" | "Inside";
export interface TextStroke { color: string; width: number; align?: StrokeAlign; }
export interface TextShadow { color: string; offset_x?: number; offset_y?: number; blur?: number; }
export interface TextBackgroundBox { fill?: TextFill; padding_x?: number; padding_y?: number; corner_radius?: number; opacity?: number; }
export interface TextSpanStyle { fill?: TextFill | null; font?: FontReference | null; font_size?: number | null; letter_spacing?: number | null; }
export interface RichTextSpan { start: number; end: number; style?: TextSpanStyle; }
export interface TextRichStyle {
  font?: FontReference | null; font_size?: number | null; layout?: TextLayout; fill?: TextFill;
  strokes?: TextStroke[]; shadows?: TextShadow[]; background?: TextBackgroundBox | null; spans?: RichTextSpan[];
}
export type AnimationUnit = "All" | "Grapheme" | "Word" | "Line";
export type StaggerOrder = "Forward" | "Backward" | "CenterOut" | "Random";
export type LoopMode = "None" | "Repeat" | "PingPong";
export type DurationConflictPolicy = "Reject" | "ProportionalScale" | "ClampOut";
export interface AnimationTransform { translate_x: number; translate_y: number; scale_x: number; scale_y: number; rotation: number; opacity: number; }
export interface TextAnimationSegment { duration: RationalTime; easing?: KeyframeEasing; stagger_ratio?: number; from?: AnimationTransform; to?: AnimationTransform; }
export interface TextAnimation {
  unit?: AnimationUnit; order?: StaggerOrder; seed?: number | null; pivot_x?: number; pivot_y?: number;
  in_anim?: TextAnimationSegment | null; loop_anim?: TextAnimationSegment | null;
  out_anim?: TextAnimationSegment | null; loop_mode?: LoopMode; conflict_policy?: DurationConflictPolicy;
}
```

`TimelineItem` adds `text_rich?: TextRichStyle | null;` and `text_animation?: TextAnimation | null;`.
`src/types/preview.ts` `PreviewSampledLayer` adds the same two fields.
`src/types/command.ts` adds the 7 operations with snake_case payload fields matching Rust.

## 6. Required evidence / tests

Must all pass before this slice is reportable:
1. Domain unit tests (`domain/text.rs`): grapheme count for Vietnamese NFC and NFD, ZWJ emoji;
   span bounds/overlap/order failures; limits (512 graphemes, 4 strokes, 4 shadows, 8 stops);
   invalid colors; Random without seed; duration conflict policies.
2. Persistence: v9 fixture with text item + keyframes + ledger + transition loads as v10 and
   preserves non-text data; `text_rich`/`text_animation` are `None`; schema 11 rejected.
3. Commands: each new op applies and round-trips undo/redo; `ClipTextSet` full-span expansion,
   clamp and prune; invalid second operation rolls back the whole transaction; stale revision
   rejected; replay after save/reopen does not duplicate mutation; non-Text item rejected;
   animation duration conflict rejected under `Reject`.
4. Composition: rich text + animation carried into `CompositionVisualLayer` and
   `PreviewSampledLayer` for Text and `None` for video.
5. Automation: capability discovery reports the 7 operations and new enum values.
6. `cargo fmt --check`, focused `cargo test` (new tests + persistence/command/composition
   suites), `pnpm run typecheck`, `pnpm test`, `pnpm run build`.

Known environment policy: the POC5 qualification matrix may not terminate; do not use it as a
gate and do not report timeout as PASS.

Report back: files changed, exact test names/counts actually run, and any deviation from this
brief with evidence.
