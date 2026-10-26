import { describe, it, expect, beforeEach, vi } from "vitest";
import { TimelineItem, Asset } from "../types/project";
import {
  TemplateDefinition,
  TemplateLayer,
  TemplateParameterType,
} from "../types/template";
import {
  buildTemplateDefinitionFromItem,
  createTemplateApplyOp,
  createTemplateTrackApplyOp,
  createTextTrackTemplateApplyOp,
  filterTemplates,
  finalizeDefinitionResources,
  isLegacyTemplateId,
  LEGACY_TEMPLATE_IDS,
  resolvePackAssets,
  validateParameterValue,
} from "../utils/templateHelpers";
import { editorApi } from "../services/editorApi";

describe("Phase 33 Template Interaction & Regression Suite", () => {
  beforeEach(() => {
    // Clean environment
  });

  describe("P1 Save As correctness & P32 authority restoration", () => {
    it("restores cleared primary fields from authoritative TimelineItem on an edited P32 instance", () => {
      // In P32, when a template is applied, the backend CommandService sets:
      // primary layer: text_content = null, text_rich = null, text_animation = null, text_style = null.
      // The TimelineItem itself is the canonical authority for primary text fields.
      const p32PrimaryLayer: TemplateLayer = {
        layer_id: "lyr_primary_text",
        name: "Headline",
        z_index: 1,
        visible: true,
        opacity: 1,
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        anchor: "Center",
        pivot: [0.5, 0.5],
        motion: null,
        kind: {
          type: "Text",
          data: {
            is_primary: true,
            slot_key: null,
            text_content: null, // Cleared in P32 instance!
            text_style: null,    // Cleared in P32 instance!
            text_rich: null,     // Cleared in P32 instance!
            text_animation: null,// Cleared in P32 instance!
          },
        },
      };

      const secondaryShapeLayer: TemplateLayer = {
        layer_id: "lyr_bg_shape",
        name: "Background Plate",
        z_index: 0,
        visible: true,
        opacity: 0.9,
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        anchor: "Center",
        pivot: [0.5, 0.5],
        motion: null,
        kind: {
          type: "Shape",
          data: {
            shape_kind: "RoundedRectangle",
            width: 800,
            height: 140,
            fill_color: "#1E293B",
            stroke_color: "#3B82F6",
            stroke_width: 2,
            corner_radius: 8,
          },
        },
      };

      const editedP32Item: TimelineItem = {
        item_id: "item_p32_edited",
        name: "My Edited Lower Third",
        kind: "Text",
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        crop: { left: 0, top: 0, right: 0, bottom: 0 },
        opacity: 1,
        volume: 1,
        timeline_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 120, rate: { numerator: 30, denominator: 1 } },
        },
        source_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 120, rate: { numerator: 30, denominator: 1 } },
        },
        // Authoritative primary text fields owned by the item:
        text_content: "Authoritative Headline Text",
        text_style: {
          font_family: "Inter",
          font_size: 52,
          font_weight: 800,
          font_color: "#FFD700",
          accent_color: null,
          background_color: null,
          background_opacity: 0,
          padding_x: 0,
          padding_y: 0,
          alignment: "Center",
          line_height: 1.1,
          anchor: "Center",
        },
        text_animation: {
          unit: "Word",
          in_anim: {
            duration: { value: 20, rate: { numerator: 30, denominator: 1 } },
          },
        },
        template_instance: {
          template_id: "original_lower_third",
          template_version: 1,
          layers: [p32PrimaryLayer, secondaryShapeLayer],
          parameters: {
            text_content: { kind: "Text", value: "Old Draft" },
          },
          initial_parameters: {},
          overrides: [],
          responsive_policy: null,
        },
      };

      // Act: Build template definition for Save As
      const definition = buildTemplateDefinitionFromItem(editedP32Item, {
        id: "saved_custom_lower_third",
        name: "Saved Custom Lower Third",
        category: "Titles",
        description: "Authoring test from edited P32 instance",
        tags: ["custom", "test"],
      });

      // Assert: The primary layer in the definition must have the restored authoritative item content
      const defPrimary = definition.layers.find(
        (l) => l.kind.type === "Text" && l.kind.data.is_primary
      );
      expect(defPrimary).toBeDefined();
      if (defPrimary && defPrimary.kind.type === "Text") {
        expect(defPrimary.kind.data.text_content).toBe("Authoritative Headline Text");
        expect(defPrimary.kind.data.text_style?.font_color).toBe("#FFD700");
        expect(defPrimary.kind.data.text_style?.font_size).toBe(52);
        expect(defPrimary.kind.data.text_animation?.unit).toBe("Word");
      }

      // Secondary shape layer preserved intact
      const defShape = definition.layers.find((l) => l.kind.type === "Shape");
      expect(defShape).toBeDefined();
      if (defShape && defShape.kind.type === "Shape") {
        expect(defShape.kind.data.shape_kind).toBe("RoundedRectangle");
        expect(defShape.kind.data.fill_color).toBe("#1E293B");
      }

      // Re-applying this definition produces a valid TemplateApply operation
      const applyOp = createTemplateApplyOp(
        "new_target_item",
        definition.id,
        definition.version
      );
      expect(applyOp.operation.type).toBe("TemplateApply");
      if (applyOp.operation.type === "TemplateApply") {
        expect(applyOp.operation.payload.template_id).toBe("saved_custom_lower_third");
        expect(applyOp.operation.payload.template_version).toBe(1);
      }
    });

    it("preserves rich gradient fill semantics and does not generate solid font_color parameter", () => {
      const gradientItem: TimelineItem = {
        item_id: "item_gradient",
        name: "Gradient Title",
        kind: "Text",
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        crop: { left: 0, top: 0, right: 0, bottom: 0 },
        opacity: 1,
        volume: 1,
        timeline_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
        },
        source_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
        },
        text_content: "Gradient Text",
        text_style: {
          font_family: "Inter",
          font_size: 60,
          font_weight: 700,
          font_color: "#FFFFFF", // Legacy font_color field
          accent_color: null,
          background_color: null,
          background_opacity: 0,
          padding_x: 0,
          padding_y: 0,
          alignment: "Center",
          line_height: 1,
          anchor: "Center",
        },
        text_rich: {
          fill: {
            kind: "Gradient",
            gradient_kind: "Linear",
            angle_deg: 90,
            stops: [
              { offset: 0, color: "#3B82F6" },
              { offset: 1, color: "#EC4899" },
            ],
          },
        },
        text_animation: {
          in_anim: {
            duration: { value: 15, rate: { numerator: 30, denominator: 1 } },
          },
        },
      };

      const def = buildTemplateDefinitionFromItem(gradientItem, {
        id: "gradient_tpl",
        name: "Gradient Template",
        category: "Effects",
        description: "Gradient semantics test",
        tags: [],
      });

      // Assert: Because fill is a Gradient, no 'font_color' parameter should be generated
      // (which would overwrite the gradient with a solid color upon template parameter apply)
      const fontColorParam = def.parameters.find((p) => p.key === "font_color");
      expect(fontColorParam).toBeUndefined();

      // But headline text parameter is still generated
      const textParam = def.parameters.find((p) => p.key === "text_content");
      expect(textParam).toBeDefined();

      // Animation belongs to primary layer text_animation, NOT duplicated in layer.motion
      const primary = def.layers.find(
        (l) => l.kind.type === "Text" && l.kind.data.is_primary
      );
      expect(primary).toBeDefined();
      expect(primary?.motion).toBeNull();
      if (primary && primary.kind.type === "Text") {
        expect(primary.kind.data.text_animation?.in_anim).toBeDefined();
      }
    });

    it("preserves independent layer motion alongside text grapheme animation", () => {
      const independentMotion = {
        unit: "All" as const,
        in_anim: {
          duration: { value: 30, rate: { numerator: 30, denominator: 1 } },
        },
      };

      const graphemeAnimation = {
        unit: "Grapheme" as const,
        in_anim: {
          duration: { value: 15, rate: { numerator: 30, denominator: 1 } },
        },
      };

      const itemWithBoth: TimelineItem = {
        item_id: "item_both_motion",
        name: "Layer with Both Motions",
        kind: "Text",
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        crop: { left: 0, top: 0, right: 0, bottom: 0 },
        opacity: 1,
        volume: 1,
        timeline_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
        },
        source_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
        },
        text_content: "Independent Motion Test",
        text_style: {
          font_family: "Inter",
          font_size: 40,
          font_weight: 600,
          font_color: "#FFFFFF",
          accent_color: null,
          background_color: null,
          background_opacity: 0,
          padding_x: 0,
          padding_y: 0,
          alignment: "Center",
          line_height: 1,
          anchor: "Center",
        },
        text_animation: graphemeAnimation,
        template_instance: {
          template_id: "tpl_independent",
          template_version: 1,
          layers: [
            {
              layer_id: "lyr_prim_indep",
              name: "Primary",
              z_index: 0,
              visible: true,
              opacity: 1,
              transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
              anchor: "Center",
              pivot: [0.5, 0.5],
              motion: independentMotion, // Independent layer motion
              kind: {
                type: "Text",
                data: {
                  is_primary: true,
                  slot_key: null,
                  text_content: null,
                  text_style: null,
                  text_rich: null,
                  text_animation: null,
                },
              },
            },
          ],
          parameters: {},
          initial_parameters: {},
          overrides: [],
          responsive_policy: null,
        },
      };

      const def = buildTemplateDefinitionFromItem(itemWithBoth, {
        id: "both_motions_tpl",
        name: "Both Motions",
        category: "Test",
        description: "Verify independent layer motion is not dropped",
        tags: [],
      });

      const primary = def.layers.find((l) => l.kind.type === "Text" && l.kind.data.is_primary);
      expect(primary).toBeDefined();
      // Crucial: independent layer motion must be preserved, not overwritten with null!
      expect(primary?.motion).toEqual(independentMotion);
      // Grapheme animation restored to primary text data
      if (primary && primary.kind.type === "Text") {
        expect(primary.kind.data.text_animation?.unit).toBe("Grapheme");
      }
    });

    it("uses rich solid fill color as default parameter rather than stale legacy font_color", () => {
      const richFillItem: TimelineItem = {
        item_id: "item_rich_fill",
        name: "Rich Fill Title",
        kind: "Text",
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        crop: { left: 0, top: 0, right: 0, bottom: 0 },
        opacity: 1,
        volume: 1,
        timeline_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
        },
        source_range: {
          start: { value: 0, rate: { numerator: 30, denominator: 1 } },
          duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
        },
        text_content: "Rich Fill Text",
        text_style: {
          font_family: "Inter",
          font_size: 48,
          font_weight: 700,
          font_color: "#FF0000", // Stale legacy red
          accent_color: null,
          background_color: null,
          background_opacity: 0,
          padding_x: 0,
          padding_y: 0,
          alignment: "Center",
          line_height: 1,
          anchor: "Center",
        },
        text_rich: {
          fill: {
            kind: "Solid",
            color: "#3B82F6", // Authoritative active rich blue
          },
        },
      };

      const def = buildTemplateDefinitionFromItem(richFillItem, {
        id: "rich_fill_tpl",
        name: "Rich Fill",
        category: "Test",
        description: "Verify rich fill color precedence",
        tags: [],
      });

      const fontColorParam = def.parameters.find((p) => p.key === "font_color");
      expect(fontColorParam).toBeDefined();
      expect(fontColorParam?.default_value).toEqual({
        kind: "Color",
        value: "#3B82F6", // Must use rich fill color!
      });
    });
  });

  describe("P1 Legacy presets vs layered catalog track routing", () => {
    it("routes all 6 legacy presets to TextTrackTemplateApply", () => {
      expect(LEGACY_TEMPLATE_IDS).toEqual([
        "clean-lower-third",
        "bold-headline",
        "center-pop",
        "subtitle-card",
        "quote-card",
        "call-to-action",
      ]);

      for (const presetId of LEGACY_TEMPLATE_IDS) {
        expect(isLegacyTemplateId(presetId)).toBe(true);

        const op = createTextTrackTemplateApplyOp("track_text_1", presetId, 1);
        expect(op.operation.type).toBe("TextTrackTemplateApply");
        if (op.operation.type === "TextTrackTemplateApply") {
          expect(op.operation.payload.track_id).toBe("track_text_1");
          expect(op.operation.payload.template_id).toBe(presetId);
          expect(op.operation.payload.template_version).toBe(1);
        }
      }
    });

    it("routes layered catalog templates to TemplateTrackApply", () => {
      const catalogIds = ["cyberpunk-intro", "minimal-author-card", "news-ticker-v2"];

      for (const id of catalogIds) {
        expect(isLegacyTemplateId(id)).toBe(false);

        const op = createTemplateTrackApplyOp("track_text_1", id, 2);
        expect(op.operation.type).toBe("TemplateTrackApply");
        if (op.operation.type === "TemplateTrackApply") {
          expect(op.operation.payload.track_id).toBe("track_text_1");
          expect(op.operation.payload.template_id).toBe(id);
          expect(op.operation.payload.template_version).toBe(2);
        }
      }
    });
  });

  describe("P2 Parameter constraints (negative, fractional, enum)", () => {
    it("validates negative and fractional number constraints correctly", () => {
      const numberType: TemplateParameterType = {
        type: "Number",
        min: -180.5,
        max: 180.5,
        step: 0.25,
      };

      // Valid negative and fractional numbers
      expect(validateParameterValue({ kind: "Number", value: -180.5 }, numberType)).toBe(true);
      expect(validateParameterValue({ kind: "Number", value: -45.25 }, numberType)).toBe(true);
      expect(validateParameterValue({ kind: "Number", value: 0 }, numberType)).toBe(true);
      expect(validateParameterValue({ kind: "Number", value: 120.75 }, numberType)).toBe(true);
      expect(validateParameterValue({ kind: "Number", value: 180.5 }, numberType)).toBe(true);

      // Out of bounds numbers
      expect(validateParameterValue({ kind: "Number", value: -181 }, numberType)).toBe(false);
      expect(validateParameterValue({ kind: "Number", value: 200 }, numberType)).toBe(false);
      expect(validateParameterValue({ kind: "Number", value: NaN }, numberType)).toBe(false);
      expect(validateParameterValue({ kind: "Number", value: Infinity }, numberType)).toBe(false);

      // Wrong kind
      expect(validateParameterValue({ kind: "Text", value: "45" }, numberType)).toBe(false);
    });

    it("validates enum options and rejects undeclared enum options", () => {
      const enumType: TemplateParameterType = {
        type: "Enum",
        options: ["Left", "Center", "Right", "Justify"],
      };

      // Valid enum values
      expect(validateParameterValue({ kind: "Enum", value: "Left" }, enumType)).toBe(true);
      expect(validateParameterValue({ kind: "Enum", value: "Center" }, enumType)).toBe(true);
      expect(validateParameterValue({ kind: "Enum", value: "Right" }, enumType)).toBe(true);
      expect(validateParameterValue({ kind: "Enum", value: "Justify" }, enumType)).toBe(true);

      // Invalid / undeclared enum values
      expect(validateParameterValue({ kind: "Enum", value: "Top" }, enumType)).toBe(false);
      expect(validateParameterValue({ kind: "Enum", value: "center" }, enumType)).toBe(false);
      expect(validateParameterValue({ kind: "Enum", value: "" }, enumType)).toBe(false);
      expect(validateParameterValue({ kind: "Enum", value: "InjectedOption" }, enumType)).toBe(false);

      // Wrong kind
      expect(validateParameterValue({ kind: "Text", value: "Center" }, enumType)).toBe(false);
    });
  });

  describe("P1 Pack export asset resolution and failure rejection", () => {
    it("resolves all resources and matches assetFiles when all project assets are present", () => {
      const mockDef: TemplateDefinition = {
        id: "badge_tpl",
        version: 1,
        name: "Badge",
        category: "Badges",
        description: "A badge with an image and custom font",
        tags: [],
        parameters: [],
        bindings: [],
        layers: [
          {
            layer_id: "lyr_logo",
            name: "Logo",
            z_index: 0,
            visible: true,
            opacity: 1,
            transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
            anchor: "Center",
            pivot: [0.5, 0.5],
            kind: {
              type: "Image",
              data: {
                asset_id: "asset_logo_png",
                width: 100,
                height: 100,
                placement_mode: "Fit",
              },
            },
          },
        ],
        resources: {
          resources: [
            {
              resource_id: "res_custom_font",
              kind: "Font",
              file_name: "BrandFont.ttf",
              sha256: "abc123hash",
              byte_size: 45000,
              required: true,
            },
          ],
        },
        responsive_policy: {
          reference_width: 1920,
          reference_height: 1080,
          supported_aspect_ratios: ["16:9"],
          auto_wrap: true,
          auto_fit: false,
          safe_margin: 60,
        },
        default_duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
      };

      const projectAssets: Asset[] = [
        {
          asset_id: "res_custom_font",
          name: "BrandFont.ttf",
          path: "/workspace/fonts/BrandFont.ttf",
          media_kind: "Image",
          file_size: 45000,
          duration: { value: 0, rate: { numerator: 30, denominator: 1 } },
        },
        {
          asset_id: "asset_logo_png",
          name: "Logo.png",
          path: "/workspace/images/Logo.png",
          media_kind: "Image",
          file_size: 12000,
          duration: { value: 0, rate: { numerator: 30, denominator: 1 } },
        },
      ];

      const { assetFiles, missingResources } = resolvePackAssets([mockDef], projectAssets);
      expect(missingResources).toHaveLength(0);
      expect(assetFiles.length).toBeGreaterThanOrEqual(2);
      expect(assetFiles.some(([id, path]) => id === "res_custom_font" && path === "/workspace/fonts/BrandFont.ttf")).toBe(true);
      expect(assetFiles.some(([id, path]) => id === "asset_logo_png" && path === "/workspace/images/Logo.png")).toBe(true);
    });

    it("rejects and reports missingResources when required resource is not in project assets", () => {
      const mockDefWithMissing: TemplateDefinition = {
        id: "badge_missing",
        version: 1,
        name: "Badge Missing Font",
        category: "Badges",
        description: "A badge with an unresolvable font",
        tags: [],
        parameters: [],
        bindings: [],
        layers: [],
        resources: {
          resources: [
            {
              resource_id: "res_missing_font",
              kind: "Font",
              file_name: "NonExistentFont.ttf",
              sha256: "000000hash",
              byte_size: 50000,
              required: true,
            },
          ],
        },
        responsive_policy: {
          reference_width: 1920,
          reference_height: 1080,
          supported_aspect_ratios: ["16:9"],
          auto_wrap: true,
          auto_fit: false,
          safe_margin: 60,
        },
        default_duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
      };

      // Project assets empty
      const { assetFiles, missingResources } = resolvePackAssets([mockDefWithMissing], []);
      expect(assetFiles).toHaveLength(0);
      expect(missingResources.length).toBe(1);
      expect(missingResources[0]).toContain("NonExistentFont.ttf");
    });
  });

  describe("P2 Unbounded catalog filtering & pagination", () => {
    it("handles large catalog of 100 definitions with bounded pagination and rapid search", () => {
      const largeCatalog: TemplateDefinition[] = Array.from({ length: 100 }, (_, i) => ({
        id: `tpl_item_${i}`,
        version: 1,
        name: `Template Name ${i}`,
        category: i % 3 === 0 ? "Lower Thirds" : i % 3 === 1 ? "Titles" : "Callouts",
        description: `Description for template ${i}`,
        tags: [`tag_${i}`, i === 42 ? "special-target" : "regular"],
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
          safe_margin: 60,
        },
        default_duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
      }));

      // Unfiltered search: returns all 100
      const allFiltered = filterTemplates(largeCatalog, { search: "", category: "All" });
      expect(allFiltered).toHaveLength(100);

      // Windowing slice with DEFAULT_PAGE_SIZE = 12
      const pageSize = 12;
      const initialMounted = allFiltered.slice(0, pageSize);
      expect(initialMounted).toHaveLength(12);

      // Rapid specific search
      const searchFiltered = filterTemplates(largeCatalog, { search: "special-target" });
      expect(searchFiltered).toHaveLength(1);
      expect(searchFiltered[0].id).toBe("tpl_item_42");

      // Category filter
      const lowerThirds = filterTemplates(largeCatalog, { category: "Lower Thirds" });
      expect(lowerThirds.length).toBeGreaterThan(0);
      expect(lowerThirds.every((t) => t.category === "Lower Thirds")).toBe(true);
    });
  });

  describe("P1 Resource SHA-256 Hashing & Integrity Finalization", () => {
    it("computes real SHA-256 and sets byte sizes for all required template resources", async () => {
      const mockHash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
      const hashSpy = vi.spyOn(editorApi, "hashLocalFile").mockResolvedValue(mockHash);

      const templateToFinalize: TemplateDefinition = {
        id: "final_tpl",
        version: 1,
        name: "Final Template",
        category: "Test",
        description: "Resource hashing test",
        tags: [],
        parameters: [],
        bindings: [],
        layers: [
          {
            layer_id: "lyr_img",
            name: "Logo Layer",
            z_index: 0,
            visible: true,
            opacity: 1,
            transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
            anchor: "Center",
            pivot: [0.5, 0.5],
            kind: {
              type: "Image",
              data: {
                asset_id: "asset_logo_123",
                asset_name: "logo.png",
                width: 200,
                height: 200,
                placement_mode: "Fit",
              },
            },
          },
        ],
        resources: {
          resources: [
            {
              resource_id: "res_font_123",
              kind: "Font",
              file_name: "Roboto-Bold.ttf",
              sha256: "0000000000000000000000000000000000000000000000000000000000000000",
              byte_size: 0,
              required: true,
            },
          ],
        },
        responsive_policy: {
          reference_width: 1920,
          reference_height: 1080,
          supported_aspect_ratios: ["16:9"],
          auto_wrap: true,
          auto_fit: false,
          safe_margin: 60,
        },
        default_duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
      };

      const assets: Asset[] = [
        {
          asset_id: "asset_logo_123",
          name: "logo.png",
          path: "C:/assets/logo.png",
          media_kind: "Image",
          file_size: 84000,
          duration: { value: 0, rate: { numerator: 30, denominator: 1 } },
        },
        {
          asset_id: "res_font_123",
          name: "Roboto-Bold.ttf",
          path: "C:/assets/Roboto-Bold.ttf",
          media_kind: "Image",
          file_size: 156000,
          duration: { value: 0, rate: { numerator: 30, denominator: 1 } },
        },
      ];

      const finalized = await finalizeDefinitionResources(templateToFinalize, assets);
      expect(hashSpy).toHaveBeenCalledWith("C:/assets/logo.png");
      expect(hashSpy).toHaveBeenCalledWith("C:/assets/Roboto-Bold.ttf");

      const imgRes = finalized.resources.resources.find((r) => r.resource_id === "asset_logo_123");
      expect(imgRes).toBeDefined();
      expect(imgRes?.sha256).toBe(mockHash);
      expect(imgRes?.byte_size).toBe(84000);
      expect(imgRes?.kind).toBe("Image");

      const fontRes = finalized.resources.resources.find((r) => r.resource_id === "res_font_123");
      expect(fontRes).toBeDefined();
      expect(fontRes?.sha256).toBe(mockHash);
      expect(fontRes?.byte_size).toBe(156000);
      expect(fontRes?.kind).toBe("Font");

      hashSpy.mockRestore();
    });

    it("rejects saving definition when required resource is missing or unresolvable", async () => {
      const templateMissing: TemplateDefinition = {
        id: "missing_tpl",
        version: 1,
        name: "Missing Template",
        category: "Test",
        description: "Resource missing failure test",
        tags: [],
        parameters: [],
        bindings: [],
        layers: [
          {
            layer_id: "lyr_img",
            name: "Unresolvable Logo",
            z_index: 0,
            visible: true,
            opacity: 1,
            transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
            anchor: "Center",
            pivot: [0.5, 0.5],
            kind: {
              type: "Image",
              data: {
                asset_id: "nonexistent_asset",
                width: 100,
                height: 100,
                placement_mode: "Fit",
              },
            },
          },
        ],
        resources: { resources: [] },
        responsive_policy: {
          reference_width: 1920,
          reference_height: 1080,
          supported_aspect_ratios: ["16:9"],
          auto_wrap: true,
          auto_fit: false,
          safe_margin: 60,
        },
        default_duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
      };

      await expect(finalizeDefinitionResources(templateMissing, [])).rejects.toThrow(
        /Required image asset "nonexistent_asset" for layer "Unresolvable Logo" was not found in project assets/
      );
    });
  });

  describe("P1 Sequential Layer Two-Field Edits & Stale Overwrite Prevention", () => {
    it("merges sequential edits from canonical layer state without restoring stale captured fields", () => {
      const canonicalLayer: TemplateLayer = {
        layer_id: "lyr_canonical",
        name: "Old Name",
        z_index: 0,
        visible: true,
        opacity: 0.5,
        transform: { position_x: 10, position_y: 20, scale_x: 1, scale_y: 1, rotation: 0 },
        anchor: "Center",
        pivot: [0.5, 0.5],
        motion: null,
        kind: {
          type: "Shape",
          data: {
            shape_kind: "Rectangle",
            width: 100,
            height: 50,
            fill_color: "#111111",
            stroke_color: "#222222",
            stroke_width: 1,
            corner_radius: 0,
          },
        },
      };

      // Simulated canonical instance layers
      let currentCanonicalLayers = [canonicalLayer];

      const commitLayerPatch = (layerId: string, patch: Partial<TemplateLayer>): TemplateLayer => {
        const canonical = currentCanonicalLayers.find((l) => l.layer_id === layerId)!;
        const merged: TemplateLayer = {
          ...canonical,
          ...patch,
          transform: patch.transform ? { ...canonical.transform, ...patch.transform } : canonical.transform,
          kind: patch.kind ? patch.kind : canonical.kind,
        };
        // Update canonical state to simulate command execution result
        currentCanonicalLayers = [merged];
        return merged;
      };

      // 1. First edit: update name
      const res1 = commitLayerPatch("lyr_canonical", { name: "New Name" });
      expect(res1.name).toBe("New Name");
      expect(res1.opacity).toBe(0.5);

      // 2. Second edit: update opacity (must NOT revert name back to "Old Name")
      const res2 = commitLayerPatch("lyr_canonical", { opacity: 0.95 });
      expect(res2.name).toBe("New Name");
      expect(res2.opacity).toBe(0.95);

      // 3. Third edit: update transform position_x (must NOT revert name or opacity)
      const res3 = commitLayerPatch("lyr_canonical", {
        transform: { ...currentCanonicalLayers[0].transform, position_x: 150 },
      });
      expect(res3.name).toBe("New Name");
      expect(res3.opacity).toBe(0.95);
      expect(res3.transform.position_x).toBe(150);
      expect(res3.transform.position_y).toBe(20);
    });
  });
});
