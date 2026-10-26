import { describe, it, expect, beforeEach } from "vitest";
import {
  filterTemplates,
  extractCategories,
  isValidTemplateId,
  slugifyTemplateId,
  formatParameterValue,
  createTemplateApplyOp,
  createTemplateTrackApplyOp,
  createTemplateLayerAddOp,
  createTemplateLayerRemoveOp,
  createTemplateLayerReorderOp,
  createTemplateLayerUpdateOp,
  createTemplateParameterSetOp,
  createTemplateResetOp,
  createTemplateDetachOp,
  buildTemplateDefinitionFromItem,
  clearTemplateStorageForTesting,
  loadFavoriteTemplateIds,
  saveFavoriteTemplateIds,
  toggleFavoriteTemplateId,
  loadRecentTemplateIds,
  recordRecentTemplateId,
} from "../utils/templateHelpers";
import { TemplateDefinition, TemplateLayer } from "../types/template";
import { TimelineItem } from "../types/project";

const SAMPLE_DEFINITIONS: TemplateDefinition[] = [
  {
    id: "headline-bold",
    version: 1,
    name: "Bold Headline",
    category: "Headline",
    description: "Bold title with subtitle and rounded card",
    tags: ["headline", "title", "bold"],
    parameters: [
      {
        key: "title",
        name: "Title Text",
        param_type: { type: "Text" },
        default_value: { kind: "Text", value: "BIG HEADLINE" },
      },
    ],
    bindings: [
      {
        parameter_key: "title",
        target_layer_id: "lyr_title",
        target_property: "TextContent",
      },
    ],
    layers: [
      {
        layer_id: "lyr_title",
        name: "Headline",
        z_index: 0,
        visible: true,
        opacity: 1,
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        anchor: "Center",
        pivot: [0.5, 0.5],
        kind: {
          type: "Text",
          data: { is_primary: true, text_content: "BIG HEADLINE" },
        },
      },
    ],
    resources: { resources: [] },
    responsive_policy: {
      reference_width: 1920,
      reference_height: 1080,
      supported_aspect_ratios: ["16:9", "9:16"],
      auto_wrap: true,
      auto_fit: false,
      safe_margin: 80,
    },
    default_duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
  },
  {
    id: "clean-lower-third-v2",
    version: 1,
    name: "Clean Lower Third",
    category: "Lower Third",
    description: "Modern lower third with name and title",
    tags: ["news", "broadcast", "lower-third"],
    parameters: [],
    bindings: [],
    layers: [
      {
        layer_id: "lyr_name",
        name: "Presenter Name",
        z_index: 1,
        visible: true,
        opacity: 1,
        transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
        anchor: "BottomLeft",
        pivot: [0, 1],
        kind: {
          type: "Text",
          data: { is_primary: true, text_content: "John Doe" },
        },
      },
    ],
    resources: { resources: [] },
    responsive_policy: {
      reference_width: 1920,
      reference_height: 1080,
      supported_aspect_ratios: ["16:9", "9:16"],
      auto_wrap: true,
      auto_fit: false,
      safe_margin: 80,
    },
    default_duration: { value: 90, rate: { numerator: 30, denominator: 1 } },
  },
  {
    id: "my-custom-badge",
    version: 1,
    name: "My Custom Badge",
    category: "Badge",
    description: "User created sticker badge",
    tags: ["sticker", "social", "custom"],
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
    default_duration: { value: 60, rate: { numerator: 30, denominator: 1 } },
  },
];

describe("Template Helpers & Catalog Filtering (Phase 33)", () => {
  const builtinIds = new Set(["headline-bold", "clean-lower-third-v2"]);

  it("extracts unique sorted categories with 'All' prepended", () => {
    const cats = extractCategories(SAMPLE_DEFINITIONS);
    expect(cats).toEqual(["All", "Badge", "Headline", "Lower Third"]);
  });

  it("filters templates by text search query matching name, category, or tags", () => {
    const matchName = filterTemplates(SAMPLE_DEFINITIONS, { search: "bold" });
    expect(matchName.map((d) => d.id)).toEqual(["headline-bold"]);

    const matchTag = filterTemplates(SAMPLE_DEFINITIONS, { search: "broadcast" });
    expect(matchTag.map((d) => d.id)).toEqual(["clean-lower-third-v2"]);

    const matchCategory = filterTemplates(SAMPLE_DEFINITIONS, { search: "badge" });
    expect(matchCategory.map((d) => d.id)).toEqual(["my-custom-badge"]);

    const noMatch = filterTemplates(SAMPLE_DEFINITIONS, { search: "nonexistent query" });
    expect(noMatch).toEqual([]);
  });

  it("filters templates by category chip", () => {
    const headlineOnly = filterTemplates(SAMPLE_DEFINITIONS, { category: "Headline" });
    expect(headlineOnly.map((d) => d.id)).toEqual(["headline-bold"]);

    const allCats = filterTemplates(SAMPLE_DEFINITIONS, { category: "All" });
    expect(allCats.length).toBe(3);
  });

  it("filters templates by source (builtin vs user)", () => {
    const builtinOnly = filterTemplates(SAMPLE_DEFINITIONS, {
      source: "builtin",
      builtinIds,
    });
    expect(builtinOnly.map((d) => d.id)).toEqual(["headline-bold", "clean-lower-third-v2"]);

    const userOnly = filterTemplates(SAMPLE_DEFINITIONS, {
      source: "user",
      builtinIds,
    });
    expect(userOnly.map((d) => d.id)).toEqual(["my-custom-badge"]);
  });

  it("filters templates by favorites and recents", () => {
    const favoriteIds = new Set(["headline-bold"]);
    const favOnly = filterTemplates(SAMPLE_DEFINITIONS, {
      favoriteIds,
      onlyFavorites: true,
    });
    expect(favOnly.map((d) => d.id)).toEqual(["headline-bold"]);

    const recentIds = ["my-custom-badge", "clean-lower-third-v2"];
    const recentOnly = filterTemplates(SAMPLE_DEFINITIONS, {
      recentIds,
      onlyRecent: true,
    });
    expect(recentOnly.map((d) => d.id)).toEqual(["clean-lower-third-v2", "my-custom-badge"]);
  });
});

describe("ID Validation & Slugification", () => {
  it("validates template ID characters and length", () => {
    expect(isValidTemplateId("valid-id_123")).toBe(true);
    expect(isValidTemplateId("Headline")).toBe(true);
    expect(isValidTemplateId("")).toBe(false);
    expect(isValidTemplateId("invalid id with spaces")).toBe(false);
    expect(isValidTemplateId("bad/slash")).toBe(false);
    expect(isValidTemplateId("a".repeat(65))).toBe(false);
  });

  it("slugifies user title into a valid safe ID", () => {
    expect(slugifyTemplateId("My Cool Template 2026!")).toBe("my-cool-template-2026");
    expect(slugifyTemplateId("   Lower   Third   ")).toBe("lower-third");
    expect(slugifyTemplateId("!@#$%^")).toBe("custom-template");
  });
});

describe("Parameter Formatting", () => {
  it("formats different parameter types to display string", () => {
    expect(formatParameterValue({ kind: "Text", value: "Hello" })).toBe("Hello");
    expect(formatParameterValue({ kind: "Color", value: "#FF0000" })).toBe("#FF0000");
    expect(formatParameterValue({ kind: "Number", value: 42.5 })).toBe("42.5");
    expect(formatParameterValue({ kind: "Boolean", value: true })).toBe("true");
    expect(formatParameterValue({ kind: "Boolean", value: false })).toBe("false");
    expect(formatParameterValue({ kind: "Enum", value: "OptionA" })).toBe("OptionA");
  });
});

describe("Template Command Envelope Builders", () => {
  it("creates TemplateApply operation envelope matching Rust serde schema", () => {
    const op = createTemplateApplyOp("item_1", "headline-bold", 1, {
      title: { kind: "Text", value: "Custom Title" },
    });
    expect(op.operation.type).toBe("TemplateApply");
    if (op.operation.type === "TemplateApply") {
      expect(op.operation.payload.item_id).toBe("item_1");
      expect(op.operation.payload.template_id).toBe("headline-bold");
      expect(op.operation.payload.template_version).toBe(1);
      expect(op.operation.payload.initial_parameters).toEqual({
        title: { kind: "Text", value: "Custom Title" },
      });
    }
  });

  it("creates TemplateTrackApply operation envelope", () => {
    const op = createTemplateTrackApplyOp("trk_1", "subtitle-card", 2);
    expect(op.operation.type).toBe("TemplateTrackApply");
    if (op.operation.type === "TemplateTrackApply") {
      expect(op.operation.payload.track_id).toBe("trk_1");
      expect(op.operation.payload.template_id).toBe("subtitle-card");
      expect(op.operation.payload.template_version).toBe(2);
    }
  });

  it("creates layer manipulation operations: Add, Remove, Reorder, Update", () => {
    const layer: TemplateLayer = {
      layer_id: "lyr_shape_1",
      name: "Box",
      z_index: 0,
      visible: true,
      opacity: 1,
      transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
      anchor: "Center",
      pivot: [0.5, 0.5],
      kind: {
        type: "Shape",
        data: {
          shape_kind: "Rectangle",
          width: 200,
          height: 100,
          stroke_width: 0,
          corner_radius: 0,
        },
      },
    };

    const addOp = createTemplateLayerAddOp("item_1", layer);
    expect(addOp.operation.type).toBe("TemplateLayerAdd");

    const rmOp = createTemplateLayerRemoveOp("item_1", "lyr_shape_1");
    expect(rmOp.operation.type).toBe("TemplateLayerRemove");

    const reorderOp = createTemplateLayerReorderOp("item_1", ["lyr_2", "lyr_1"]);
    expect(reorderOp.operation.type).toBe("TemplateLayerReorder");

    const updOp = createTemplateLayerUpdateOp("item_1", "lyr_shape_1", layer);
    expect(updOp.operation.type).toBe("TemplateLayerUpdate");
  });

  it("creates ParameterSet, Reset, and Detach operations", () => {
    const paramOp = createTemplateParameterSetOp("item_1", "title_color", {
      kind: "Color",
      value: "#FF5500",
    });
    expect(paramOp.operation.type).toBe("TemplateParameterSet");

    const resetOp = createTemplateResetOp("item_1");
    expect(resetOp.operation.type).toBe("TemplateReset");

    const detachOp = createTemplateDetachOp("item_1");
    expect(detachOp.operation.type).toBe("TemplateDetach");
  });
});

describe("Template Authoring Definition Builder", () => {
  const sampleItem: TimelineItem = {
    item_id: "item_txt_1",
    name: "Breaking News Text",
    kind: "Text",
    text_content: "BREAKING: Phase 33 UI Complete",
    text_style: {
      font_family: "Roboto",
      font_size: 52,
      font_weight: 700,
      font_color: "#FFDD00",
      background_opacity: 0,
      padding_x: 0,
      padding_y: 0,
      alignment: "Center",
      line_height: 1.2,
      anchor: "Center",
    },
    timeline_range: {
      start: { value: 0, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 120, rate: { numerator: 30, denominator: 1 } },
    },
    source_range: {
      start: { value: 0, rate: { numerator: 30, denominator: 1 } },
      duration: { value: 120, rate: { numerator: 30, denominator: 1 } },
    },
    transform: { position_x: 0, position_y: 100, scale_x: 1, scale_y: 1, rotation: 0 },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    opacity: 1,
    volume: 1,
  };

  it("builds a valid TemplateDefinition from a TimelineItem without template instance", () => {
    const def = buildTemplateDefinitionFromItem(sampleItem, {
      id: "breaking-news-banner",
      name: "Breaking News Banner",
      category: "News",
      description: "Yellow alert banner for breaking stories",
      tags: ["news", "alert"],
      exposeParameters: true,
    });

    expect(def.id).toBe("breaking-news-banner");
    expect(def.version).toBe(1);
    expect(def.name).toBe("Breaking News Banner");
    expect(def.category).toBe("News");
    expect(def.tags).toEqual(["news", "alert"]);
    expect(def.layers.length).toBe(1);

    const primaryLayer = def.layers[0];
    expect(primaryLayer.kind.type).toBe("Text");
    if (primaryLayer.kind.type === "Text") {
      expect(primaryLayer.kind.data.is_primary).toBe(true);
      expect(primaryLayer.kind.data.text_content).toBe("BREAKING: Phase 33 UI Complete");
    }

    // Parameters should be exposed for primary text & font color
    expect(def.parameters.length).toBe(2);
    expect(def.parameters.find((p) => p.key === "text_content")?.default_value).toEqual({
      kind: "Text",
      value: "BREAKING: Phase 33 UI Complete",
    });
    expect(def.parameters.find((p) => p.key === "font_color")?.default_value).toEqual({
      kind: "Color",
      value: "#FFDD00",
    });

    // Duration should match item duration
    expect(def.default_duration.value).toBe(120);
  });

  it("builds a TemplateDefinition from an item with existing multi-layer template instance", () => {
    const itemWithLayers: TimelineItem = {
      ...sampleItem,
      template_instance: {
        template_id: "banner-multi",
        template_version: 1,
        layers: [
          {
            layer_id: "lyr_bg",
            name: "Background Box",
            z_index: 0,
            visible: true,
            opacity: 0.9,
            transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
            anchor: "Center",
            pivot: [0.5, 0.5],
            kind: {
              type: "Shape",
              data: {
                shape_kind: "RoundedRectangle",
                width: 600,
                height: 150,
                stroke_width: 2,
                corner_radius: 8,
              },
            },
          },
          {
            layer_id: "lyr_txt",
            name: "Main Text",
            z_index: 1,
            visible: true,
            opacity: 1,
            transform: { position_x: 0, position_y: 0, scale_x: 1, scale_y: 1, rotation: 0 },
            anchor: "Center",
            pivot: [0.5, 0.5],
            kind: {
              type: "Text",
              data: { is_primary: true, text_content: "Saved with Box" },
            },
          },
        ],
        parameters: {},
        initial_parameters: {},
        overrides: [],
      },
    };

    const def = buildTemplateDefinitionFromItem(itemWithLayers, {
      id: "banner-with-box",
      name: "Banner with Box",
      category: "Headline",
      description: "Two layer template",
      version: 2,
    });

    expect(def.version).toBe(2);
    expect(def.layers.length).toBe(2);
    expect(def.layers[0].layer_id).toBe("lyr_bg");
    expect(def.layers[1].layer_id).toBe("lyr_txt");
  });
});

describe("Favorites and Recents Management", () => {
  beforeEach(() => {
    clearTemplateStorageForTesting();
  });

  it("toggles and persists favorites", () => {
    const current = new Set<string>();
    const added = toggleFavoriteTemplateId("tpl_1", current);
    expect(added.has("tpl_1")).toBe(true);

    const loaded = loadFavoriteTemplateIds();
    expect(loaded.has("tpl_1")).toBe(true);

    const removed = toggleFavoriteTemplateId("tpl_1", added);
    expect(removed.has("tpl_1")).toBe(false);
  });

  it("saves and loads favorite template IDs directly", () => {
    saveFavoriteTemplateIds(new Set(["fav_1", "fav_2"]));
    const loaded = loadFavoriteTemplateIds();
    expect(loaded.has("fav_1")).toBe(true);
    expect(loaded.has("fav_2")).toBe(true);
  });

  it("records recent template applications with max recents bounded", () => {
    let recents: string[] = [];
    recents = recordRecentTemplateId("tpl_A", recents);
    recents = recordRecentTemplateId("tpl_B", recents);
    recents = recordRecentTemplateId("tpl_A", recents); // moves to front

    expect(recents[0]).toBe("tpl_A");
    expect(recents[1]).toBe("tpl_B");
    expect(recents.length).toBe(2);

    const loaded = loadRecentTemplateIds();
    expect(loaded).toEqual(["tpl_A", "tpl_B"]);
  });
});
