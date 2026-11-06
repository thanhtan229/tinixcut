# Phase 55 editorial quality rubric

**Purpose:** assess editorial usefulness separately from whether the plan validates, applies, saves, or exports. Scores describe one review of the Phase 45 disposable fixture; they are not a general AI accuracy claim or a substitute for the still-blocked Phase 55 native workflows.

## Scoring

Score each dimension from 1 to 5 using visible source frames, the stated brief, and the exact selected order. Do not infer events between sampled frames.

| Score | Relevance to brief | Continuity/order | Text readability |
|---|---|---|---|
| 1 | Mostly unrelated scenes | Sequence contradicts itself or is unusable | Text is unreadable or obscures essential content |
| 2 | Weak relation; key brief intent is missed | Noticeable jump with no narrative reason | Frequent contrast, crop, timing, or safe-area problems |
| 3 | Generally on topic with a clear omission | Understandable but uneven progression | Readable with a visible consistency issue |
| 4 | Strong match; minor omission | Progression is plausible with one minor jump | Consistent, legible, and inside safe area |
| 5 | Directly satisfies the brief with no unsupported claims | Deliberate and coherent progression | Clear across sampled frames and target canvas |

For an exploratory sample, report each score and evidence. Do not combine the editorial score with technical validation. A product-quality acceptance target is at least 4 in each dimension across three separately reviewed project briefs; this single-fixture evaluation cannot satisfy that target.

## Exploratory review: existing Phase 45 export

- **Artifact:** `artifacts/phase45/disposable/phase45_scene_guided_draft.mp4`; three inspected frames at 1.5, 7.5, and 13.5 seconds; source report query: “Describe the visible Hoi An street and people”.
- **Selection order:** `scene-02`, `scene-01`, `scene-03` (MCP/native evidence manifest).
- **Relevance: 4/5.** Inspected frames show a person/river scene, a Hoi An market street with people, and a lantern-lit street. These are broadly on brief. The model's first source caption incorrectly called an early-morning frame “sunset”; that error is documented in the Phase 44 human check.
- **Continuity/order: 2/5.** The sampled order moves from a sunset river view to an early-morning market, then to a night street. The time-of-day change between the first two scenes is conspicuous and not explained by the brief. The model caption error increases this risk.
- **Readability: 4/5.** Existing Vietnamese subtitles are legible in all three sampled frames and remain near the lower safe area. This is only a still-frame check; subtitle timing and motion were not assessed here.
- **Result:** 10/15; revise order or add an explicit non-chronological/travel montage intent before applying. This is an exploratory reviewer score, not an independent panel decision.
- **Technical evidence is separate:** the Phase 45 evidence records 1920×1080 H.264/AAC, 30 fps, 15 seconds, ffprobe exit 0, and full FFmpeg decode exit 0. Those facts do not improve the editorial scores.

## Remaining evidence

A second reviewer and two additional briefs/projects are required to satisfy the three-project acceptance target. Phase 55 native Apply/Undo/save/reopen/export workflows also remain open; the rubric does not close them.

### G55 material availability reconciliation — 2026-11-05

Only one underlying Hoi An media corpus is present among the workspace's non-disposable sample projects; the landscape and portrait project variants share the same still/audio hashes, and the 15-second video variants share one SHA-256. The sample inventory is recorded at `../artifacts/phase55/phase55_sample_availability_audit_20261105.json`. They are not counted as independent additional project samples. G55 remains open pending two additional approved real project/brief samples and an independent qualified reviewer.
