import { execFileSync, spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { createInterface } from "node:readline";
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

  it("applies the same transaction through the MCP stdio server", async () => {
    const dir = mkdtempSync(join(tmpdir(), "tinixcut-caption-mcp-"));
    const binary = process.env.TINIXCUT_TEST_BINARY ?? resolve("src-tauri/target/debug/tinixcut.exe");
    const child = spawn(binary, ["mcp"], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
    let stderr = "";
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", chunk => { stderr += chunk; });
    const lines = createInterface({ input: child.stdout });
    const responses = lines[Symbol.asyncIterator]();
    const call = async (id: number, name: string, args: object) => {
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method: "tools/call", params: {
        _meta: { "io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {} },
        name, arguments: args,
      } }) + "\n");
      const next = await responses.next();
      if (next.done) throw new Error(`MCP exited before responding: ${stderr}`);
      const response = JSON.parse(next.value);
      expect(response.id).toBe(id);
      expect(response.result?.isError, JSON.stringify(response)).toBe(false);
      return response.result.structuredContent;
    };
    try {
      const projectPath = join(dir, "project.tcproj");
      const project = JSON.parse(readFileSync(resolve("scripts/qualification/background/caption-merge.tcproj"), "utf8")) as Project;
      writeFileSync(projectPath, JSON.stringify(project));
      const envelope = {
        schema_version: project.schema_version, project_id: project.project_id, base_revision: project.revision,
        client_id: "caption-mcp-test", client_epoch: 1, sequence_number: 1,
        dry_run: false, transaction_id: "tx_caption_merge_mcp",
        operations: buildCaptionMergeOperations(project, "caption_left", "caption_right", 1),
      };
      await call(1, "tinixcut_dry_run", { projectPath, envelope });
      expect(JSON.parse(readFileSync(projectPath, "utf8")).revision).toBe(project.revision);
      const receipt = await call(2, "tinixcut_apply", { projectPath, envelope });
      expect(receipt.newRevision).toBe(project.revision + 1);
      const saved = JSON.parse(readFileSync(projectPath, "utf8")) as Project;
      expect(saved.tracks[0].items).toHaveLength(1);
      expect(saved.tracks[0].items[0]).toMatchObject({ item_id: "caption_left", text_content: "Xin chao\nViet Nam",
        timeline_range: { start: { value: 0 }, duration: { value: 60 } }, source_range: { duration: { value: 60 } } });
    } finally {
      child.kill();
      lines.close();
      rmSync(dir, { recursive: true, force: true });
    }
  }, 30000);
});
