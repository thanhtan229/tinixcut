import { describe, expect, it } from "vitest";
import { VisualSceneAnalysisReport } from "../services/editorApi";
import { applySceneRangeInputs, initialSceneRangeInputs } from "./sceneRangeDraft";

const report: VisualSceneAnalysisReport = {
  schemaVersion: 1,
  algorithmVersion: "test",
  provider: "local",
  model: "test",
  query: "walk",
  sourceFingerprint: "source",
  configFingerprint: "config",
  truncated: false,
  warnings: [],
  scenes: [{
    sceneId: "scene-1", assetId: "asset-1", assetName: "video.mp4",
    sourceStartMicros: 1_000_000, sourceEndMicros: 5_000_000, sampledAtMicros: 3_000_000,
    caption: "walking", tags: [], relevanceScore: 0.9, rationale: "clear",
  }],
};

describe("scene source range draft", () => {
  it("defaults to full analyzed ranges and applies an in-bounds trim without mutating the report", () => {
    const inputs = initialSceneRangeInputs(report);
    expect(inputs["scene-1"]).toEqual({ startSeconds: "1.000000", endSeconds: "5.000000" });
    expect(applySceneRangeInputs(report, ["scene-1"], inputs).scenes[0].sourceEndMicros).toBe(5_000_000);
    inputs["scene-1"] = { startSeconds: "2.250", endSeconds: "4.500" };

    const trimmed = applySceneRangeInputs(report, ["scene-1"], inputs);

    expect(trimmed.scenes[0]).toMatchObject({
      sourceStartMicros: 2_250_000,
      sourceEndMicros: 4_500_000,
      sampledAtMicros: 3_000_000,
    });
    expect(report.scenes[0].sourceStartMicros).toBe(1_000_000);
  });

  it("rejects ranges outside the analyzed interval, empty ranges, and invalid numbers", () => {
    const invalid = [
      { startSeconds: "0", endSeconds: "4" },
      { startSeconds: "4", endSeconds: "4" },
      { startSeconds: "nope", endSeconds: "5" },
    ];
    for (const input of invalid) {
      expect(() => applySceneRangeInputs(report, ["scene-1"], { "scene-1": input })).toThrow();
    }
  });
});
