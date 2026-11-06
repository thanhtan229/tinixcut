import { VisualSceneAnalysisReport } from "../services/editorApi";

export interface SceneRangeInput {
  startSeconds: string;
  endSeconds: string;
}

export type SceneRangeInputs = Record<string, SceneRangeInput>;

export function initialSceneRangeInputs(report: VisualSceneAnalysisReport): SceneRangeInputs {
  return Object.fromEntries(report.scenes.map((scene) => [scene.sceneId, {
    startSeconds: (scene.sourceStartMicros / 1_000_000).toFixed(6),
    endSeconds: (scene.sourceEndMicros / 1_000_000).toFixed(6),
  }]));
}

export function applySceneRangeInputs(
  report: VisualSceneAnalysisReport,
  selectedSceneIds: string[],
  inputs: SceneRangeInputs,
): VisualSceneAnalysisReport {
  const selected = new Set(selectedSceneIds);
  if (selected.size !== selectedSceneIds.length) throw new Error("Scene selection must not contain duplicates.");

  return {
    ...report,
    scenes: report.scenes.map((scene) => {
      if (!selected.has(scene.sceneId)) return scene;
      const input = inputs[scene.sceneId];
      const start = Number(input?.startSeconds);
      const end = Number(input?.endSeconds);
      if (!Number.isFinite(start) || !Number.isFinite(end)) {
        throw new Error(`Enter valid source in/out times for ${scene.sceneId}.`);
      }
      const startMicros = Math.round(start * 1_000_000);
      const endMicros = Math.round(end * 1_000_000);
      if (startMicros < scene.sourceStartMicros || endMicros > scene.sourceEndMicros || endMicros <= startMicros) {
        throw new Error(`Source range for ${scene.sceneId} must remain inside the analyzed scene and have positive duration.`);
      }
      return {
        ...scene,
        sourceStartMicros: startMicros,
        sourceEndMicros: endMicros,
        sampledAtMicros: Math.min(Math.max(scene.sampledAtMicros, startMicros), endMicros - 1),
      };
    }),
  };
}
