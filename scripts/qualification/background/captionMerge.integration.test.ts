import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCaptionMergeOperations } from "../../../src/components/captionEditor";
import type { Project } from "../../../src/types/project";

describe.skipIf(process.env.TINIXCUT_RUN_NATIVE_TESTS !== "1")("background caption merge through the CLI", () => {
  it("applies the UI's merge transaction and persists the result", () => {
    const dir = mkdtempSync(join(tmpdir(), "tinixcut-caption-merge-"));
    try {
      const projectPath = join(dir, "project.tcproj");
      const envelopePath = join(dir, "merge.json");
      const project = JSON.parse(readFileSync(resolve("scripts/qualification/background/caption-merge.tcproj"), "utf8")) as Project;
      writeFileSync(projectPath, JSON.stringify(project));
      writeFileSync(envelopePath, JSON.stringify({
        schema_version: project.schema_version,
        project_id: project.project_id,
        base_revision: project.revision,
        dry_run: false,
        transaction_id: "tx_caption_merge_background",
        operations: buildCaptionMergeOperations(project, "caption_left", "caption_right", 1),
      }));
      const binary = process.env.TINIXCUT_TEST_BINARY ?? resolve("src-tauri/target/debug/tinixcut.exe");
      execFileSync(binary, ["apply", projectPath, envelopePath], { encoding: "utf8", timeout: 30000, windowsHide: true });
      const saved = JSON.parse(readFileSync(projectPath, "utf8")) as Project;
      expect(saved.revision).toBe(project.revision + 1);
      expect(saved.tracks[0].items).toHaveLength(1);
      expect(saved.tracks[0].items[0]).toMatchObject({
        item_id: "caption_left",
        text_content: "Xin chao\nViet Nam",
        timeline_range: { start: { value: 0 }, duration: { value: 60 } },
        source_range: { duration: { value: 60 } },
      });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("applies the same transaction through the CLI JSON call", () => {
    const binary = process.env.TINIXCUT_TEST_BINARY ?? resolve("src-tauri/target/debug/tinixcut.exe");
    const dir = mkdtempSync(join(tmpdir(), "tinixcut-caption-cli-json-"));
    try {
      const projectPath = join(dir, "project.tcproj");
      const project = JSON.parse(readFileSync(resolve("scripts/qualification/background/caption-merge.tcproj"), "utf8")) as Project;
      writeFileSync(projectPath, JSON.stringify(project));
      const envelope = {
        schema_version: project.schema_version, project_id: project.project_id, base_revision: project.revision,
        client_id: "caption-cli-test", client_epoch: 1, sequence_number: 1,
        dry_run: false, transaction_id: "tx_caption_merge_cli_json",
        operations: buildCaptionMergeOperations(project, "caption_left", "caption_right", 1),
      };
      const dryRun = JSON.parse(execFileSync(binary, ["call", "dry_run", "-"], {
        input: JSON.stringify({ projectPath, envelope }), encoding: "utf8", timeout: 30000, windowsHide: true,
      }));
      expect(dryRun.result?.isError, JSON.stringify(dryRun)).toBe(false);
      expect(JSON.parse(readFileSync(projectPath, "utf8")).revision).toBe(project.revision);
      const applied = JSON.parse(execFileSync(binary, ["call", "apply", "-"], {
        input: JSON.stringify({ projectPath, envelope }), encoding: "utf8", timeout: 30000, windowsHide: true,
      }));
      expect(applied.result?.isError, JSON.stringify(applied)).toBe(false);
      const receipt = applied.result.structuredContent;
      expect(receipt.newRevision).toBe(project.revision + 1);
      const saved = JSON.parse(readFileSync(projectPath, "utf8")) as Project;
      expect(saved.tracks[0].items).toHaveLength(1);
      expect(saved.tracks[0].items[0]).toMatchObject({ item_id: "caption_left", text_content: "Xin chao\nViet Nam",
        timeline_range: { start: { value: 0 }, duration: { value: 60 } }, source_range: { duration: { value: 60 } } });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
