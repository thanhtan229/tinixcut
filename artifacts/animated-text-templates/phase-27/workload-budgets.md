# Reference Host, Workload Definitions and Performance Budgets

**Program:** Tinix Animated Text Templates (Phases 27–36)  
**Deliverable:** Phase 27 Acceptance Requirement `P27-AC-06`  
**Status:** Baseline Specification (Empirical measurements to be locked in Phase 28)

---

## 1. Reference Windows Host Specification

All performance evaluations, rendering benchmarks, and latency measurements for Phases 27–36 target and document the primary reference Windows development host environment:

- **OS:** Windows 11 Pro 24H2 (`Windows-10-10.0.26200-SP0` / Build 26200)
- **CPU:** Intel Core i5-13400F (10 cores: 6 P-cores + 4 E-cores, 16 hardware threads)
- **RAM:** 64 GB DDR4/DDR5 system memory
- **GPU:** NVIDIA GeForce RTX 3090 (24 GB VRAM)
- **Storage:** Samsung 990 PRO NVMe SSD
- **Display Resolution:** Primary 1920×1080 @ 60 Hz or higher
- **Media Pipeline:** Local Gyan FFmpeg 9.0.2 pinned binaries with software `libx264` baseline and DXVA2/D3D11VA / NVENC hardware detection

### Baseline Metrics (Pre-Phase 27 Baseline Snapshot)
- **Rust Tauri Binary Size (Debug):** `24,498,688 bytes` (23.36 MiB / 24.50 MB)
- **Rust Tauri Binary Size (Release):** `7,381,504 bytes` (7.04 MiB / 7.38 MB)
- **Windows NSIS Installer Package Size:** `59,085,922 bytes` (56.35 MiB / 59.09 MB)
- **Verified Rust Tests Suite:** 222 tests passing in ~7.3s
- **Verified Frontend Vitest Suite:** 44 tests passing in ~0.7s
- **Phase 17 Baseline Latencies (Documented in `RESEARCH.md`):**
  - `load_project` median: `2.985 ms` (P95: `3.951 ms`)
  - `save_project_atomic` median: `3.890 ms` (P95: `5.912 ms`)
  - Fixture B JSONL apply (`saveAfter:false`) median: `0.1034 ms` (P95: `0.1369 ms`)
  - Fixture B MCP inspect median: `3.4117 ms` (P95: `3.8771 ms`)
  - Fixture B MCP dry-run median: `3.5426 ms` (P95: `4.9322 ms`)
  - Fixture B MCP persistent apply median: `16.760 ms` (P95: `24.496 ms`)
  - CPU `libx264` Export Throughput: 1080p30 RTF $\ge 1.00\times$, 720p30 RTF $\ge 2.20\times$

---

## 2. Standard Workload Definitions

To ensure apples-to-apples performance comparisons across candidate renderers in Phase 28 and subsequent engine optimizations, the following four workloads are standardized:

### Workload W1 — Single Animated Rich Title
- **Canvas:** 1920×1080 @ 30 fps (and tested at 60 fps)
- **Content:** Single text slot, 120 grapheme clusters
- **Appearance:** Linear gradient (2 colors, angle 118°), 2 outer strokes, 2 drop shadows
- **Motion:** In-animation with per-character stagger over 1.0 second (30 frames)
- **Evaluation Points:** $t = 0.0\text{s}$, $t = 0.5\text{s}$ (mid-stagger), $t = 1.0\text{s}$ (completion), $t = 3.0\text{s}$ (hold phase)

### Workload W2 — Multi-Title Concurrency
- **Canvas:** 1920×1080 @ 30 fps
- **Content:** 4 concurrent, non-overlapping text titles active simultaneously on separate tracks
- **Appearance:** Mixed rich styles (gradient, outline, background box, multi-shadow)
- **Motion:** Staggered in-animation active on all 4 instances

### Workload W3 — Maximum Layer Density
- **Canvas:** 1920×1080 @ 30 fps (and 1080×1920 9:16)
- **Content:** Single template instance with maximum allowed complexity:
  - 8 text slots (each 32 graphemes)
  - 8 shape / background / decorative layers
  - Total: 16 layers with z-index ordering and distinct anchor points
- **Motion:** Synchronized layer-level entrance and exit animations

### Workload W4 — Continuous Export Throughput
- **Duration:** 60.0 continuous seconds (1800 frames @ 30 fps)
- **Content:** Alternating W1 and W2 title overlays over 1080p source video
- **Measurement:** Total export elapsed time, average fps, frame delivery latency into FFmpeg stdin pipe, memory peak during export run

---

## 3. Performance Budget Targets (To Be Validated in Phase 28)

The following initial target budgets are established as design criteria for candidate renderers. They are explicitly marked as **targets**, not measured facts, and MUST be validated on the reference host during Phase 28:

| Metric | Target Budget | Rationale |
| :--- | :--- | :--- |
| **W1 Raster + Delivery Latency (p95)** | $\le 16.0\text{ ms}$ | Enables smooth 60 fps preview playback |
| **W2 Concurrency Latency (p95)** | $\le 33.3\text{ ms}$ | Maintains minimum 30 fps interactive preview |
| **W3 16-Layer Raster Latency (p95)** | $\le 50.0\text{ ms}$ | Avoids preview hitching during complex scenes |
| **W4 Export Throughput Speedup** | $\ge 1.5\times$ real-time | Ensures export is bottlenecked by video encoder, not text overlay generation |
| **Bounded Overlay Cache Ceiling** | $\le 256\text{ MiB}$ | Prevents memory exhaustion during prolonged editing sessions |
| **Application Installer Growth** | $\le 35\text{ MB}$ additional | Restricts native dependencies from ballooning package footprint |
| **Cold Startup Latency Overhead** | $\le 150\text{ ms}$ additional | Ensures app launch responsiveness is preserved |

---

## 4. Verification and Tolerance Policies

1. **Overlay Comparison**:
   - Preview frame vs. Headless export frame comparison MUST evaluate the extracted RGBA overlay buffer before color-space conversion or lossy video encoding.
   - Text bounding box SSIM MUST exceed $0.985$ across all text ROI regions.
   - Layout metrics (glyph bounds, baseline alignment, total line width) MUST match within $\le 1.0\text{ pixel}$.

2. **Encoding Artifact Separation**:
   - Encoded H.264 MP4 outputs are evaluated separately for perceptual quality. Codec compression artifacts MUST NOT be used to loosen renderer core tolerances.
