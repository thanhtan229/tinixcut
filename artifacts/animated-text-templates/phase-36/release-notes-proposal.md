# Release Notes Proposal — TinixCut v0.1.0 (Milestone T5: Rich Animated Text Templates)

**Version:** 0.1.0  
**Release Target:** Windows 10 / Windows 11 (x64)  
**Package Artifact:** `TinixCut_0.1.0_x64-setup.exe` (57.54 MiB)  
**Status:** IMPLEMENTED WITH DEFERRED QUALIFICATION (Clean-VM and hardware encoder qualification open)  

---

## 1. What's New in This Release

### 1.1. Pure Rust Typography & Software Rasterizer Engine (Phase 28 & 30)
- **Zero-Dependency Architecture:** Custom high-performance 2D typography and rasterization engine implemented entirely in pure Rust using `ab_glyph`, `unicode-normalization`, and `unicode-segmentation`. Requires zero external C++/LLVM/Python toolchains.
- **Vietnamese Language Perfection:** Unicode NFC precomposition and UAX #29 Extended Grapheme Cluster segmentation ensure Vietnamese tone marks and diacritics (`ử`, `ệ`, `ế`) remain atomically bound during stagger animations, scaling, and 2D rotations without visual splitting or clipping.
- **Rich Visual Styling:** Supports static-bounds-anchored multi-stop linear gradients (custom angles), radial gradients, up to 4 dilated outer strokes (circular dilation, widest-to-narrowest), up to 4 separable gaussian drop shadows, and rounded background boxes.
- **Bit-for-Bit Determinism:** Preview and headless export share the exact same rasterizer code path, guaranteeing 100% overlay parity ($\text{SSIM} = 1.000000$ on lossless overlays, $\text{SSIM} \ge 0.997$ on encoded MP4).

### 1.2. Expressive Text Animation Engine (Phase 31)
- Character-level, word-level, line-level, and whole-layer animation units.
- Built-in stagger cascade orders (`Forward`, `Backward`, `CenterOut`, `Random`) and loop modes (`None`, `Repeat`, `PingPong`).
- Motion presets: `FadeIn`, `FadeOut`, `SlideInLeft`, `SlideInRight`, `PopIn`, `Typewriter`, `Wiggle`, `Pulse`.
- Explicit duration conflict policies (`Reject`, `ProportionalScale`, `ClampOut`) prevent timeline overlaps during trimming.

### 1.3. Multi-Layer Templates & Template Packs (Phase 32 & 33)
- Support for complex template compositions up to 16 visual layers (Text, Rounded Shapes, Decorative Shapes, Images).
- Parameter binding system: expose template controls (e.g. title text, accent color, background opacity) to the editor Inspector without breaking underlying layout.
- Portable `.tinixtpl` format for importing and exporting template packs with full asset containment, checksum verification, and anti-path-traversal security hardening.
- 12 built-in curated templates ready to use from the Library panel.

### 1.4. CapCut Template Import Adapter (Phase 34)
- Import CapCut draft folders and ZIP archives directly into TinixCut.
- **Scope Option 1 Fidelity:** Full 1:1 visual and motion parity for the evidenced `StaggeredCharacterIn` family (vertical slide + bounce/rotation).
- Graceful degradation: Unsupported animation curves or 3D text meshes are transparently identified in an itemized `ConversionReport` and imported as editable approximations or baked reference video layers.

### 1.5. Autonomous Agent Generation Surface (Phase 35)
- Full programmatic template creation, inspection, refinement, preview, and application via Command Line Interface (`tinixcut template ...`) and Model Context Protocol (`tinixcut mcp`).
- 12 authoritative MCP tools operating over standard stdio JSON-RPC.
- Strict input validation against JSON Schema 2020-12, bounded concurrency (max 4 background render workers), 15s timeout protection, and atomic transaction rollback.

### 1.6. Offline-First Desktop Distribution (Phase 36)
- Self-contained NSIS installer packages everything needed to run:
  - Pinned Gyan Release Essentials FFmpeg 9.0.2 (`ffmpeg.exe` and `ffprobe.exe`)
  - Bundled Apache-2.0 `Roboto-Bold.ttf` font embedded directly in the binary
  - Zero requirement for active internet connectivity or external developer toolchains.

---

## 2. Performance Benchmarks on Windows Reference Host

Tested on Windows 11 Pro 24H2, i5-13400F, 64 GB RAM, RTX 3090:

- **Workload W1 (120-grapheme animated title):** Warm p95 = **6.65 ms** (Budget $\le 16.0\text{ ms}$ — capable of sustained 60 fps preview).
- **Workload W2 (4 concurrent animated titles):** Warm p95 = **11.40 ms** (Budget $\le 33.3\text{ ms}$).
- **Workload W3 (16-layer density stress):** 16:9 warm p95 = **19.13 ms**, 9:16 warm p95 = **19.36 ms** (Budget $\le 50.0\text{ ms}$).
- **Preview IPC Delivery:** Warm p95 = **13.53 ms** (Budget $\le 16.0\text{ ms}$).
- **Export Throughput:** **4.92× Real-Time** (0.406s for 2.0s 60-frame video, Budget $\ge 1.0\times$ RTF).
- **Glyph & Mask Cache:** 0.38 MiB footprint (Ceiling $\le 256\text{ MiB}$).

---

## 3. Known Limitations & Deferred Items

1. **Clean-VM & GPU Hardware Encoder Gates:** Execution on clean, non-developer Windows virtual machines and native hardware GPU encoding (NVENC/AMF/QSV) remain documented as deferred qualification items. Production exports currently utilize software `libx264`.
2. **CapCut 3D & Advanced Effects:** 3D text extrusions and proprietary shaders cannot be edited natively and are preserved as baked video references.
3. **Transparent Video Export:** MP4 (H.264/HEVC) exports are composited over an opaque canvas. True transparent alpha export requires ProRes 4444 or PNG frame sequence export.
4. **Embedded AI API Provider:** TinixCut does not embed private API keys or make external network calls. AI generation relies on external clients orchestrating TinixCut via CLI or MCP.
