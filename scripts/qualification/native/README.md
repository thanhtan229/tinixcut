# Native qualification harness (Windows)

Scripts used to qualify G54 on 2026-11-12. They drive the real TinixCut window through
Windows UI Automation (native input path) and read in-page state through the WebView2
DevTools protocol (CDP). Evidence from these runs lives under `artifacts/` (untracked).

## Start the app with the debug port

The port is local-only and is passed as a one-off config override; never commit it into
`src-tauri/tauri.conf.json`.

```bash
pnpm tauri dev --config scripts/qualification/native/cdp-config-override.json
```

For measurements that must use the production frontend bundle (UI response, RAM):

```bash
pnpm tauri build --debug --no-bundle --config scripts/qualification/native/cdp-config-override.json
```

Requirements: Python 3 with `websockets`; Windows PowerShell 5. Run the scripts from this
directory (`python -I <script>`; they call the `.ps1` helpers by relative path) and pass
output paths under `artifacts/`.

## Helpers

| Script | Purpose |
|---|---|
| `cdp.py "<js>"` | Evaluate JavaScript in the app page and print the JSON result. |
| `restore_window.ps1` | Restore the window if minimized (a minimized WebView throttles timers). |
| `open_fixture.ps1 -Path <p>` then `fill_dialog.ps1 -Path <p>` | Invoke Open and submit the native file dialog (`WM_SETTEXT` on the file-name box; use `-EditId 1001` for Save dialogs). |
| `invoke_button.ps1 -Name <exact>` / `invoke_button_suffix.ps1 -Suffix <text>` | Invoke a uniquely named button (suffix form for emoji-prefixed labels). |
| `seek_uia.ps1 -Frame <n>` | Focus the Go-to-frame field, set the value, read it back, invoke Go. |
| `seek2_uia.ps1 -First <a> -Second <b>` | Two seeks back to back (superseded-seek / cancel step). |
| `ui_action.ps1 -Action play\|pause\|step\|select [-ClipName <name>]` | Transport and clip selection. `select` sends Enter only when TinixCut owns the foreground window. |
| `toggle_proxy.ps1 -Want On\|Off` | Preview proxy checkbox (Media cache dialog must be open). |
| `proc_tree_mem.ps1` | Private/working-set memory summed over `tinixcut.exe` and descendants. |
| `set_range.ps1 -Name <slider> -Value <v>` | Set a named slider (e.g. Inspector Brightness) through UIA RangeValue. |
| `select_option.ps1 -Option <text>` | Choose an option in a `<select>` (e.g. Output → High quality). |

## Measurements

| Script | What it measures |
|---|---|
| `run30.py <out.json> [frames...]` | Seek set: Go invoke to last video layer `seeked`. Needs the dev build and the recorder: `sessionStorage.setItem("tinix.previewVideoDiagnostics","1")` then reload. |
| `summarize.py <run.json>...` | Sync count, p50/p95/max for seek runs. |
| `ui_response.py <out.json> <clip names,csv> <frames,csv>` | Input event to painted result for Play/Pause/Step/Go/Select (`UI_SAMPLES`, default 20). |
| `ram_cycles.py <out.json> <frames,csv>` | 30 cycles of seek/play/pause/superseded seek with process-tree memory sampling. Check that every cycle's steps succeeded before accepting a run. |
| `profile_seek.py <frame> <out.cpuprofile>` | CPU profile of one seek; prints top self/total functions. |
| `capture_preview.py <dir> <frames...>` | Screenshot exactly the preview canvas at each frame after layers settle; hides HUD text and selection handles, aborts if anything covers the canvas. |
| `compare_parity.py <dir> <frames,csv> [--isolation <dir>]` | G52 metrics: luma SSIM and per-layer centroid error between `preview_fN.png` and `export_fN.png`. |

Budget definitions and accepted results: `Plans/phase54_preview_performance.md`.

## Known pitfalls

- UIA calls right after launch or page reload can find no controls; wait a few seconds and retry.
- A recovery prompt may appear after opening a fixture; choose **Open saved project** to leave recovery copies untouched.
- Opening a project whose media was never chosen through a dialog shows "Media access required"; use **Relink…** to the same file.
- Do not pool runs or count cycles whose UIA steps failed.
- The recovery prompt that can appear after opening a project is not `role=dialog`; `capture_preview.py` samples points over the canvas to detect it. Dismiss with **Later**.
- Emulating a higher device-pixel ratio over CDP does not make WebView2 re-render the canvas at that density; captures are only valid at the window's real DPR.
