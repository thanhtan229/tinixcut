# ADR-002: Windows FFmpeg Distribution, Process Isolation, and Hardware Fallback Strategy

## Status

**ACCEPTED & IMPLEMENTED (FINAL RECONCILIATION)** — Phase 12 Baseline (`COMPLETE WITH DEFERRED ITEMS`)  
**Date:** 2026-10-04  
**Deciders:** Core Architecture Team  
**Scope:** Personal-Use Desktop Video Editor (Non-Commercial / Non-Public Distribution)  
**Consulted:** Phase 12 External Research, Phase 4 Media Foundation, Phase 8 Export Engine, Gate F / POC 5 Specifications

---

## 1. Context and Problem Statement

TinixCut relies on FFmpeg and ffprobe for media probing, waveform extraction, thumbnail extraction, proxy generation, and deterministic project export. During Phases 0 through 11, the development runtime relied on ambient system `PATH` resolution via `std::process::Command::new("ffmpeg")` and `Command::new("ffprobe")`.

Repo verification confirms:
1. **Clean Installation Failure**: On a standard Windows machine without FFmpeg in system `PATH`, all media operations fail with OS Error 2 (`FileNotFound`).
2. **Bundler Gaps**: `src-tauri/tauri.conf.json` does not configure `bundle.resources`, or `bundle.windows` (NSIS/WebView2 settings).
3. **Hardcoded Process Resolution**: Rust modules (`ffmpeg.rs`, `export/mod.rs`, `parity.rs`, `preview/mod.rs`) instantiate `Command::new("ffmpeg")` directly with no application-owned binary discovery, resource location resolution, or environment override.
4. **Coarse Fallback**: `ExportEngine::render_snapshot` implements a reactive fallback (`is_hw -> libx264`), but lacks pre-flight capability probing, GPU ladder selection (`nvenc` -> `qsv` -> `amf`), or discrimination between encoder failure and input I/O errors.
5. **Product Boundary**: TinixCut is targeted strictly for **personal use, without public or commercial distribution**. Therefore, commercial distribution compliance, legal patent licensing, and EULA formalities are not release blockers, but technical provenance and debugging reproducibility remain vital.

Phase 12 transitions TinixCut from an ambient developer environment into a self-contained, reproducible, installable Windows desktop product.

---

## 2. Decision Drivers

- **Zero External Prerequisites**: The user must be able to install and run TinixCut on a clean Windows 10/11 system without manually installing FFmpeg, Python, C++ runtimes, or setting environment variables.
- **Architectural Process Boundary**: FFmpeg/ffprobe must remain an external process sidecar invoked via `std::process::Command` with typed arguments and structured stdio. No FFI dynamic or static linking against `libavcodec`/`libavformat` is introduced into the core Rust binary.
- **Personal-Use Licensing Scope**: Formal legal/compliance licensing, commercial redistribution obligations, GPL/LGPL EULA reviews, and commercial patent indemnifications are explicitly **OUT OF SCOPE**. Artifact provenance, version pinning, and build reproducibility are preserved solely for technical integrity and debugging.
- **Deterministic & Reproducible Media Engine**: The bundled FFmpeg distribution is pinned to `FFmpeg 9.0.2` Gyan Release Essentials, verified by cryptographic SHA-256 digests.
- **Single Authoritative Packaging Mechanism**: Tauri `bundle.resources` paired with an application-owned `MediaBinaryResolver`. Packaged release builds must use app-owned pinned binaries. System `PATH` is strictly disallowed in packaged release builds.
- **WebView2 Evergreen Strategy**: NSIS installer uses `downloadBootstrapper`, balancing minimal installer payload with automated fallback.
- **Reliable Hardware Acceleration & Universal Fallback**: Export supports NVIDIA (`h264_nvenc`), Intel (`h264_qsv`), and AMD (`h264_amf`) hardware acceleration, while guaranteeing seamless, automatic fallback to software `libx264` whenever hardware encoders are unavailable or fail.

---

## 3. Pinned Binary Distribution Specification

### 3.1 Distribution Channel & Build Selection
- **Upstream Distribution**: Gyan.dev FFmpeg Windows Builds.
- **Target Version**: `FFmpeg 9.0.2` (git release build).
- **Target Variant**: **Release Essentials** (`ffmpeg-9.0.2-essentials_build.zip`).
- **Binary Footprint**:
  - `ffmpeg.exe`: ~85 MB
  - `ffprobe.exe`: ~35 MB
  - Total uncompressed footprint: ~120 MB (~45 MB compressed inside NSIS LZMA stream).
- **Rationale for Essentials Build**:
  - Contains all required decoders, demuxers, and filters (`scale`, `overlay`, `aresample`, `color`, `sine`).
  - Contains `libx264` software reference baseline.
  - Contains hardware encoder headers and API bindings (`h264_nvenc`, `h264_qsv`, `h264_amf`) for DirectX/DXVA2/D3D11/NVENC/AMF/QSV runtime dispatch without external GPU SDK compile-time dependencies.
  - Omits large, unneeded external libraries (Whisper, AviSynth, LibVMAF, etc.) that would inflate binary size beyond 250 MB.

### 3.2 Artifact Provenance & Verification
For personal use, legal compliance auditing is excluded, but build provenance is maintained in a checked-in metadata manifest (`ffmpeg_provenance.json`) to guarantee reproducibility and ease bug reports:
```json
{
  "version": "9.0.2",
  "distribution": "gyan_release_essentials",
  "upstream_url": "https://www.gyan.dev/ffmpeg/builds/",
  "platform": "x86_64-pc-windows-msvc",
  "binaries": {
    "ffmpeg.exe": {
      "sha256": "EXPECTED_FFMPEG_SHA256_HEX"
    },
    "ffprobe.exe": {
      "sha256": "EXPECTED_FFPROBE_SHA256_HEX"
    }
  }
}
```

---

## 4. Authoritative Packaging & Binary Resolution

### 4.1 Packaging Mechanism: Tauri `bundle.resources`
The packaging mechanism is finalized as **Tauri `bundle.resources`**:
- Pinned binaries are stored under `src-tauri/resources/bin/ffmpeg.exe` and `src-tauri/resources/bin/ffprobe.exe`.
- Configured in `src-tauri/tauri.conf.json`:
  ```json
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "resources": [
      "resources/bin/ffmpeg.exe",
      "resources/bin/ffprobe.exe"
    ]
  }
  ```
- NSIS bundles these files into `<install_dir>/resources/bin/`.

### 4.2 Centralized Native Resolver (`MediaBinaryResolver`)
All direct `Command::new("ffmpeg")` and `Command::new("ffprobe")` calls are replaced with `MediaBinaryResolver::resolve("ffmpeg")` and `MediaBinaryResolver::resolve("ffprobe")`.

```rust
pub struct MediaBinaryResolver;

impl MediaBinaryResolver {
    pub fn resolve(binary_name: &str) -> Result<PathBuf, MediaError> {
        // 1. Packaged App Resource Path (Authoritative for production)
        if let Some(app_resource_path) = Self::resolve_packaged_resource(binary_name) {
            if app_resource_path.exists() {
                return Ok(app_resource_path);
            }
        }

        // 2. Explicit Environment Variable Override (for CI/automated test harnesses)
        let env_var = format!("TINIX_{}_PATH", binary_name.to_uppercase());
        if let Ok(env_path) = std::env::var(&env_var) {
            let path = PathBuf::from(env_path);
            if path.exists() {
                return Ok(path);
            }
        }

        // 3. System PATH fallback ONLY in explicit dev/test mode
        if cfg!(debug_assertions) || std::env::var("TINIX_DEV_ALLOW_SYSTEM_PATH").as_deref() == Ok("1") {
            if let Ok(path) = which::which(binary_name) {
                return Ok(path);
            }
        }

        // 4. Strict Failure in Packaged Builds
        Err(MediaError::BundledBinaryMissing(binary_name.to_string()))
    }
}
```

### 4.3 Production vs Development Boundary Rule
- **Packaged Production Build**: Must use the app-owned pinned binary from `resource_dir`. Falling back to ambient system `PATH` is **strictly prohibited**, guaranteeing deterministic execution regardless of host machine configuration.
- **Development/Test Mode**: System `PATH` or `TINIX_*_PATH` overrides are allowed only when compiling in debug mode (`cfg!(debug_assertions)`) or when `TINIX_DEV_ALLOW_SYSTEM_PATH=1` is explicitly set, allowing quick checkout tests without downloading the full 120 MB binary payload.

---

## 5. WebView2 Strategy: `downloadBootstrapper`

### 5.1 Chosen Strategy
The authoritative WebView2 distribution strategy for TinixCut on Windows is **`downloadBootstrapper`**.

```json
"bundle": {
  "windows": {
    "webviewInstallMode": {
      "type": "downloadBootstrapper"
    },
    "nsis": {
      "installMode": "currentUser",
      "deleteDataOnUninstall": false,
      "languages": ["en-US"]
    }
  }
}
```

### 5.2 Rationale & Trade-off Analysis
- **Selected: `downloadBootstrapper`**:
  - *Payload*: Adds minimal overhead (~1.8 MB Microsoft bootstrapper binary).
  - *Target Environment*: TinixCut targets personal use on modern Windows 10 (updated) and Windows 11 systems, which come with Microsoft Edge WebView2 Evergreen Runtime pre-installed system-wide out of the box.
  - *Reliability*: In the edge case that WebView2 has been uninstalled or corrupted, the bootstrapper automatically fetches and installs the latest Evergreen runtime during setup.
- **Rejected: `offlineInstaller`**:
  - *Trade-off*: Bundling the standalone offline installer adds ~160 MB of dead weight to every installer release, inflating the download size from ~150 MB to >310 MB.
  - *Assessment*: Offline installation is beneficial for air-gapped enterprise environments, which is irrelevant for a personal-use desktop tool.

---

## 6. Hardware Acceleration & Fallback Semantics

### 6.1 Supported Video Codec Matrix
| Codec Identifier | Engine | Platform / Vendor | Role / Qualification Scope |
| :--- | :--- | :--- | :--- |
| `libx264` | Software (CPU) | Universal (All x86_64) | **Required Qualified Export Path (Release-Blocking Baseline)** |
| `h264_nvenc` | Hardware (GPU) | NVIDIA GeForce / RTX | **Best-effort / Deferred Hardware Acceleration (Driver >= 610.00 required)** |
| `h264_qsv` | Hardware (GPU) | Intel HD / UHD / Iris / Arc | **Best-effort / Deferred Hardware Acceleration (Unqualified without host HW)** |
| `h264_amf` | Hardware (GPU) | AMD Radeon | **Best-effort / Deferred Hardware Acceleration (Unqualified without host HW)** |

### 6.2 Auto-Detection & Fallback Protocol
1. **Auto Mode (`video_codec = "auto"`)**:
   - The engine checks hardware acceleration availability via lightweight encoder probe.
   - Priority evaluation ladder: `h264_nvenc` -> `h264_qsv` -> `h264_amf` -> `libx264`.
2. **Direct Hardware Request (`video_codec = "h264_nvenc" | "h264_qsv" | "h264_amf"`)**:
   - The engine attempts the render using the requested hardware encoder.
   - **Error Discrimination**:
     - The engine inspects FFmpeg exit code and stderr.
     - If the failure is hardware/driver related (e.g. `Cannot load nvcuda.dll`, `Device creation failed`, `Driver version insufficient`, `Function not implemented`):
       - Emits a structured log: `[ExportEngine] Hardware encoder '{codec}' unavailable or failed; falling back to software reference 'libx264'`.
       - Automatically retries render using `libx264`.
     - If the failure is an I/O, source media missing, or invalid filter graph error, the engine **fails immediately** without redundant retries.
3. **Deterministic Output Parity Invariant**:
   - Renders generated via hardware acceleration MUST maintain composition parity with software renders (matching duration, audio sync, frame count, and spatial geometry).

---

## 7. Licensing & Compliance Boundaries (Personal Use)

1. **Non-Commercial / Personal Use Scope**:
   - TinixCut is maintained for personal use and developer workflows.
   - Commercial redistribution, GPL/LGPL EULA declarations, and commercial patent reviews are explicitly **OUT OF SCOPE** and do not gate Phase 12.
2. **Process Boundary Integrity**:
   - Architectural modularity is maintained: FFmpeg executes as an independent child process via `std::process::Command` communicating via pipes.
   - No direct C FFI bindings are linked into `tinixcut_lib`.
3. **Internal Provenance Documentation**:
   - Pinned version (9.0.2), build provider (gyan.dev), and SHA-256 hashes are tracked in `ffmpeg_provenance.json` for technical debugging and support.

---

## 8. Implementation Outcome & Qualification Reconciliation

| Architecture Decision | Authoritative Resolution | Status |
| :--- | :--- | :--- |
| **FFmpeg Distribution Source** | Gyan Release Essentials 9.0.2 (x86_64) | **IMPLEMENTED** |
| **Packaging Mechanism** | Tauri `bundle.resources` (`resources/bin/*`) | **IMPLEMENTED** |
| **Binary Resolution Hierarchy** | App `resource_dir` (prod) -> Env -> PATH (dev only) | **IMPLEMENTED** |
| **WebView2 Strategy** | `downloadBootstrapper` (NSIS currentUser) | **IMPLEMENTED** |
| **Hardware Fallback Matrix** | Auto/Requested HW -> Stderr check -> `libx264` | **IMPLEMENTED** |
| **Licensing / Compliance Scope** | Personal use (Legal/EULA/Patents OUT OF SCOPE) | **RECONCILED** |

### 8.1 Final Qualification Reclassification & Deferred Items
1. **Isolated Workstation Qualification**:
   - The CM-01..CM-05 verification harness executed with a sanitized `PATH` where repository and developer toolchains were stripped from the execution path.
   - Because execution occurred on the host Windows environment, this is formally classified as **Isolated Workstation Qualification**, not true clean-machine/clean-VM qualification.
2. **Deferred Items**:
   - **True clean Windows 10/11 VM qualification**: DEFERRED / non-blocking for personal-use v1.
   - **Hardware acceleration NVENC/QSV/AMF qualification**: DEFERRED / best-effort.
   - **Required Qualified Export Path**: `CPU/libx264` remains the only required qualified export path.
3. **Phase 12 & Roadmap Status**:
   - Phase 12 final status is **`COMPLETE WITH DEFERRED ITEMS`**.
   - Phase 12 is the final phase of the current TinixCut roadmap; no Phase 13 is opened.
