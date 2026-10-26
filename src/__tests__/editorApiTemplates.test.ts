import { describe, it, expect, vi, beforeEach } from "vitest";
import { editorApi } from "../services/editorApi";
import { invoke } from "@tauri-apps/api/core";
import { TemplateDefinition, TemplatePackManifest } from "../types/template";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("editorApi Template Methods (Phase 33)", () => {
  const mockInvoke = vi.mocked(invoke);

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls template_list_definitions", async () => {
    const dummyDefs: TemplateDefinition[] = [];
    mockInvoke.mockResolvedValueOnce(dummyDefs);

    const result = await editorApi.templateListDefinitions();
    expect(mockInvoke).toHaveBeenCalledWith("template_list_definitions");
    expect(result).toEqual(dummyDefs);
  });

  it("calls template_get_definition with id and version", async () => {
    mockInvoke.mockResolvedValueOnce(null);

    await editorApi.templateGetDefinition("headline-bold", 1);
    expect(mockInvoke).toHaveBeenCalledWith("template_get_definition", {
      id: "headline-bold",
      version: 1,
    });
  });

  it("calls template_save_user_definition with definition", async () => {
    const def: TemplateDefinition = {
      id: "user-def-1",
      version: 1,
      name: "User Def",
      category: "Custom",
      description: "",
      tags: [],
      parameters: [],
      bindings: [],
      layers: [],
      resources: { resources: [] },
      responsive_policy: {
        reference_width: 1920,
        reference_height: 1080,
        supported_aspect_ratios: ["16:9"],
        auto_wrap: true,
        auto_fit: false,
        safe_margin: 80,
      },
      default_duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
    };
    mockInvoke.mockResolvedValueOnce("user-def-1_v1");

    const res = await editorApi.templateSaveUserDefinition(def);
    expect(mockInvoke).toHaveBeenCalledWith("template_save_user_definition", {
      definition: def,
    });
    expect(res).toBe("user-def-1_v1");
  });

  it("calls template_delete_user_definition with id and version", async () => {
    mockInvoke.mockResolvedValueOnce(true);

    const res = await editorApi.templateDeleteUserDefinition("user-def-1", 1);
    expect(mockInvoke).toHaveBeenCalledWith("template_delete_user_definition", {
      id: "user-def-1",
      version: 1,
    });
    expect(res).toBe(true);
  });

  it("calls template_import_pack with packPath", async () => {
    const manifest: TemplatePackManifest = {
      pack_id: "test-pack",
      pack_version: 1,
      name: "Test Pack",
      template_ids: ["tpl_1"],
      created_at: "2026-10-26T00:00:00Z",
    };
    mockInvoke.mockResolvedValueOnce(manifest);

    const res = await editorApi.templateImportPack("C:/packs/test.tinixtpl");
    expect(mockInvoke).toHaveBeenCalledWith("template_import_pack", {
      packPath: "C:/packs/test.tinixtpl",
    });
    expect(res).toEqual(manifest);
  });

  it("calls template_export_pack with manifest, definitions, assetFiles, outputPath", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    const manifest: TemplatePackManifest = {
      pack_id: "export-pack",
      pack_version: 1,
      name: "Export Pack",
      template_ids: [],
      created_at: "2026-10-26T00:00:00Z",
    };

    await editorApi.templateExportPack(manifest, [], [], "C:/packs/out.tinixtpl");
    expect(mockInvoke).toHaveBeenCalledWith("template_export_pack", {
      manifest,
      definitions: [],
      assetFiles: [],
      outputPath: "C:/packs/out.tinixtpl",
    });
  });
});
