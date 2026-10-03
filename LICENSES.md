# LICENSES.md

> **TinixCut Dependency & Third-Party Licensing Inventory**
>
> Phase 12 Qualification Record

---

## 1. Application Core License

TinixCut is an automation-first desktop video editor developed under the Tinix standard.

---

## 2. Bundled & Third-Party Native Components

### 2.1 FFmpeg & FFprobe

- **License:** LGPL v2.1+ (or GPL v3 for builds enabling GPL-licensed external filters/codecs).
- **Source Code:** [https://ffmpeg.org](https://ffmpeg.org)
- **Usage Boundary:** TinixCut interacts with FFmpeg/FFprobe exclusively via process boundary (`std::process::Command` / IPC). TinixCut does not statically or dynamically link against `libavcodec`/`libavformat` directly in proprietary distributions, preserving full LGPL/GPL license compliance.
- **Hardware Fallback:** Default release profile uses software encoding (`libx264`) with optional hardware acceleration flags (`h264_nvenc`, `h264_qsv`, `h264_amf`) when supported drivers are detected.

---

## 3. Rust Dependencies (`src-tauri/Cargo.toml`)

| Crate | Version | License | Purpose |
| :--- | :--- | :--- | :--- |
| `tauri` | 2.12.x | MIT / Apache-2.0 | Native application framework |
| `tauri-build` | 2.12.x | MIT / Apache-2.0 | Tauri build scripting |
| `serde` | 1.0.x | MIT / Apache-2.0 | Serialization / Deserialization |
| `serde_json` | 1.0.x | MIT / Apache-2.0 | Canonical JSON persistence & IPC |
| `uuid` | 1.11.x | MIT / Apache-2.0 | Stable prefixed identifier generation |
| `chrono` | 0.4.x | MIT / Apache-2.0 | UTC timestamps for persistence metadata |
| `thiserror` | 2.0.x | MIT / Apache-2.0 | Typed, ergonomic domain error handling |

---

## 4. Frontend Dependencies (`package.json`)

| Package | License | Purpose |
| :--- | :--- | :--- |
| `react`, `react-dom` | MIT | UI component hierarchy and reconciliation |
| `@tauri-apps/api` | MIT / Apache-2.0 | Strongly typed frontend-to-native IPC |
| `lucide-react` | ISC | Accessible UI iconography |
| `vite` | MIT | Development server and bundling pipeline |
| `vitest` | MIT | Unit and integration test runner |
| `typescript` | Apache-2.0 | Compile-time static type verification |

---

## 5. Assets & Fonts

- **Inter Font Family:** SIL Open Font License 1.1 (OFL-1.1). Bundled for deterministic cross-platform text and caption layout parity.
