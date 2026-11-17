import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve } from "node:path";

const binary = process.env.TINIXCUT_TEST_BINARY ?? resolve("src-tauri/target/debug/tinixcut.exe");
if (!existsSync(binary)) throw new Error(`Native CLI missing: ${binary}. Run cargo build --manifest-path src-tauri/Cargo.toml first.`);
const steps = [
  ["CDP continuation and report guards", process.execPath, ["--test", "scripts/qualification/background/cdp-report-policy.node-test.mjs"]],
  ["Caption component + CLI command paths", process.execPath, ["scripts/qualification/background/headless-caption.mjs"]],
  ["App mocked IPC + native save/reopen", process.execPath, ["scripts/qualification/background/headless-app.mjs"]],
  ["Draft delayed response guards", process.execPath, ["scripts/qualification/background/headless-draft-stale.mjs"]],
  ["Planner delayed response guards", process.execPath, ["scripts/qualification/background/headless-planner-stale.mjs"]],
  ["Audio mixer calculations", process.execPath, ["node_modules/vitest/vitest.mjs", "run", "src/components/audioMeter.test.ts", "src/components/audioGain.test.ts", "src/components/audioMixPlan.test.ts"]],
  ["Audio meter graph sum and silent output", process.execPath, ["scripts/qualification/background/headless-audio-meter.mjs"]],
  ["Native track gain and summed audio export", "cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--test", "phase51_audio_mix_acceptance"]],
  ["Native caption history and timeline persistence", "cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "background_caption_history_tests::"]],
  ["Native mixed media probes", "cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "media_import_mixed_real_probe_classifies_valid_and_corrupt_files"]],
  ["Native corrupt recovery", "cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "corrupt_recovery_is_reported_without_blocking_saved_project_or_changing_file"]],
  ["Tauri mock IPC scope and history", "powershell.exe", ["-NoProfile", "-File", "scripts/qualification/background/run-mock-ipc.ps1"]],
];
const report = { startedAt: new Date().toISOString(), revision: spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8", windowsHide: true }).stdout?.trim(), dirty: Boolean(spawnSync("git", ["status", "--porcelain", "--untracked-files=no"], { encoding: "utf8", windowsHide: true }).stdout?.trim()), fixtureSha256: createHash("sha256").update(readFileSync("scripts/qualification/background/caption-merge.tcproj")).digest("hex"), scope: "Headless CLI and MockRuntime; native UI gates remain open", steps: [] };
mkdirSync("artifacts/background", { recursive: true });
try {
for (const [label, program, args] of steps) {
  console.log(`\n--- ${label} ---`);
  const step = { label, status: "running", startedAt: new Date().toISOString() };
  report.steps.push(step);
  try {
  await new Promise((resolveStep, rejectStep) => {
    const child = spawn(program, args, { stdio: "inherit", windowsHide: true });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      if (process.platform === "win32" && child.pid) spawnSync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], { windowsHide: true });
      else child.kill("SIGKILL");
    }, 180000);
    child.once("error", error => { clearTimeout(timer); rejectStep(error); });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      if (timedOut || code !== 0) rejectStep(new Error(`${label} failed: ${timedOut ? "timeout" : `exit ${code ?? signal}`}`));
      else resolveStep();
    });
  });
  step.status = "passed";
  } catch (error) { step.status = "failed"; step.error = String(error); throw error; }
}
console.log("\nPASS: background checks completed; native UI acceptance remains separate");
} finally {
  report.finishedAt = new Date().toISOString();
  writeFileSync("artifacts/background/latest-report.json", JSON.stringify(report, null, 2) + "\n");
}
