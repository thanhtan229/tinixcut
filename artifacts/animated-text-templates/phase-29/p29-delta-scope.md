# Phase 29 — Delta Scope for QA and Review

The working tree contains large pre-existing uncommitted work from Phases 5–28.
`git diff` against HEAD therefore includes unrelated changes. Review ONLY the items below.

## New files (entire file is P29)
- `src-tauri/src/modules/domain/text.rs`
- `src-tauri/tests/phase29_rich_text_domain.rs`

## Modified files — P29 hunks are those touching these identifiers only
- `src-tauri/src/modules/domain/mod.rs`
  - `pub mod text;`, `pub use text::*;`
  - `CURRENT_SCHEMA_VERSION = 10`
  - `DomainError::{TextTooLong, InvalidTextSpan, AnimationDurationConflict, UnsupportedTextFeature}`
  - `#[derive(Default)]` on `TextAlignment`
  - `pub(crate) fn is_hex_color`
  - `TimelineItem::{text_rich, text_animation}` (fields, `new`, `validate`)
- `src-tauri/src/modules/persistence/mod.rs`
  - `migrate_v9_to_v10`, migration dispatch at `version <= 9`, new tests
- `src-tauri/src/modules/command/mod.rs`
  - `CommandOperation::{ClipTextRichSet, ClipTextSpansSet, ClipTextLayoutSet, ClipTextAppearanceSet, ClipTextFontSet, ClipTextAnimationSet, ClipTextRichReset}`
  - `ClipTextSet` grapheme-span rebinding branch
  - the seven new `apply_operation` arms
- `src-tauri/src/modules/composition/mod.rs`
  - `CompositionVisualLayer::{text_rich, text_animation}`, `PreviewSampledLayer::{text_rich, text_animation}`
  - population in `from_project` and `sample_visual_layer_at`
- `src-tauri/src/modules/automation/mod.rs`
  - `ProjectInspectReport` four new capability lists
  - `supported_operations()` new names
  - `supported_animation_units/stagger_orders/loop_modes/duration_conflict_policies`
  - capability assembly in `inspect_project`
- `src/types/project.ts`, `src/types/preview.ts`, `src/types/command.ts`
  - Only the rich-text/animation types and the seven new operation entries.

## Reference contract
`artifacts/animated-text-templates/phase-29/implementation-brief.md`

## Recorded verification by Boss (2026-10-25)
- `cargo fmt --check` PASS
- `cargo test --lib -- --skip test_gate_f_poc5_matrix_execution` → 152 passed, 0 failed, 1 filtered
- `cargo test --test phase29_rich_text_domain` → 8 passed
- phase14 15 PASS; phase15 27 PASS; phase16 12 PASS; phase16-timeline 30 PASS;
  phase17 perf 1 PASS + 5 ignored; phase17 transport-seed 1 PASS; phase18 1 PASS;
  export_automation_poc 8 PASS
- `cargo test --test phase28_text_renderer_poc` → 4 passed, 2 FAILED (performance-only:
  IPC delivery 47–55 ms and W1 warm p95 27–29 ms vs locked budgets). Phase 28 test imports
  only `media::text_renderer_poc`; P29 does not touch the renderer. Treated as host
  performance variance, not a P29 regression, and NOT claimed as PASS.
- `pnpm run typecheck` PASS; `pnpm test` 44 passed; `pnpm run build` PASS
- `cargo build` PASS
- `cargo clippy --lib -D warnings` FAILS on 3 pre-existing findings
  (`export/mod.rs` derivable Default, `media/mod.rs::register_asset`,
  `lib.rs::run_auto_caption_job`), all recorded in the Phase 27 baseline inventory.
