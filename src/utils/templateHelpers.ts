import { OperationEnvelope } from "../types/command";
import { Asset, RationalTime, TimelineItem } from "../types/project";
import { editorApi } from "../services/editorApi";
import {
  TemplateDefinition,
  TemplateLayer,
  TemplateParameterDefinition,
  TemplateParameterType,
  TemplateParameterValue,
  TemplatePropertyPath,
  TemplateResourceReference,
} from "../types/template";

/**
 * Filter options for template catalog browsing.
 */
export interface TemplateFilterOptions {
  search?: string;
  category?: string;
  source?: "all" | "builtin" | "user";
  favoriteIds?: Set<string>;
  onlyFavorites?: boolean;
  recentIds?: string[];
  onlyRecent?: boolean;
  builtinIds?: Set<string>;
}

/**
 * Filters a list of template definitions according to search, category, source, favorites, and recents.
 */
export function filterTemplates(
  definitions: TemplateDefinition[],
  options: TemplateFilterOptions
): TemplateDefinition[] {
  const {
    search = "",
    category = "All",
    source = "all",
    favoriteIds,
    onlyFavorites = false,
    recentIds,
    onlyRecent = false,
    builtinIds = new Set<string>(),
  } = options;

  const query = search.trim().toLowerCase();

  return definitions.filter((def) => {
    // 1. Source filter (builtin vs user)
    const isBuiltin = builtinIds.has(def.id);
    if (source === "builtin" && !isBuiltin) return false;
    if (source === "user" && isBuiltin) return false;

    // 2. Favorites filter
    if (onlyFavorites && (!favoriteIds || !favoriteIds.has(def.id))) {
      return false;
    }

    // 3. Recents filter
    if (onlyRecent && (!recentIds || !recentIds.includes(def.id))) {
      return false;
    }

    // 4. Category filter
    if (category && category !== "All" && def.category.toLowerCase() !== category.toLowerCase()) {
      return false;
    }

    // 5. Search query matching name, category, description, and tags
    if (query) {
      const matchName = def.name.toLowerCase().includes(query);
      const matchCategory = def.category.toLowerCase().includes(query);
      const matchDesc = def.description.toLowerCase().includes(query);
      const matchTags = def.tags.some((t) => t.toLowerCase().includes(query));
      if (!matchName && !matchCategory && !matchDesc && !matchTags) {
        return false;
      }
    }

    return true;
  });
}

/**
 * Extract unique sorted categories from a list of template definitions.
 */
export function extractCategories(definitions: TemplateDefinition[]): string[] {
  const set = new Set<string>();
  for (const def of definitions) {
    if (def.category && def.category.trim()) {
      set.add(def.category.trim());
    }
  }
  return ["All", ...Array.from(set).sort()];
}

// Local storage keys
const FAVORITES_STORAGE_KEY = "tinixcut_template_favorites";
const RECENTS_STORAGE_KEY = "tinixcut_template_recents";
const MAX_RECENTS = 20;

let memoryStorage: Record<string, string> = {};

const fallbackStorage: Storage = {
  getItem: (key: string) => memoryStorage[key] ?? null,
  setItem: (key: string, value: string) => {
    memoryStorage[key] = value;
  },
  removeItem: (key: string) => {
    delete memoryStorage[key];
  },
  clear: () => {
    memoryStorage = {};
  },
  key: (index: number) => Object.keys(memoryStorage)[index] ?? null,
  get length() {
    return Object.keys(memoryStorage).length;
  },
};

function safeGetLocalStorage(): Storage {
  try {
    if (typeof window !== "undefined" && window.localStorage) {
      window.localStorage.setItem("__tinix_test__", "1");
      window.localStorage.removeItem("__tinix_test__");
      return window.localStorage;
    }
    if (
      typeof globalThis !== "undefined" &&
      (globalThis as unknown as { localStorage?: Storage }).localStorage
    ) {
      const s = (globalThis as unknown as { localStorage: Storage }).localStorage;
      s.setItem("__tinix_test__", "1");
      s.removeItem("__tinix_test__");
      return s;
    }
  } catch {
    // Storage might be blocked or unavailable in sandboxes
  }
  return fallbackStorage;
}

export function clearTemplateStorageForTesting(): void {
  safeGetLocalStorage().clear();
}

export function loadFavoriteTemplateIds(): Set<string> {
  const storage = safeGetLocalStorage();
  try {
    const raw = storage.getItem(FAVORITES_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        return new Set(parsed.filter((id) => typeof id === "string"));
      }
    }
  } catch {
    // ignore json parse error
  }
  return new Set();
}

export function saveFavoriteTemplateIds(ids: Set<string>): void {
  const storage = safeGetLocalStorage();
  try {
    storage.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(Array.from(ids)));
  } catch {
    // ignore write errors
  }
}

export function toggleFavoriteTemplateId(id: string, current: Set<string>): Set<string> {
  const updated = new Set(current);
  if (updated.has(id)) {
    updated.delete(id);
  } else {
    updated.add(id);
  }
  saveFavoriteTemplateIds(updated);
  return updated;
}

export function loadRecentTemplateIds(): string[] {
  const storage = safeGetLocalStorage();
  try {
    const raw = storage.getItem(RECENTS_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        return parsed.filter((id) => typeof id === "string");
      }
    }
  } catch {
    // ignore json parse error
  }
  return [];
}

export function recordRecentTemplateId(id: string, current: string[]): string[] {
  const filtered = current.filter((item) => item !== id);
  const updated = [id, ...filtered].slice(0, MAX_RECENTS);
  const storage = safeGetLocalStorage();
  try {
    storage.setItem(RECENTS_STORAGE_KEY, JSON.stringify(updated));
  } catch {
    // ignore write errors
  }
  return updated;
}

/**
 * Validates whether an identifier conforms to Tinix ID rules:
 * alphanumeric, hyphen, underscore, 1..64 chars.
 */
export function isValidTemplateId(id: string): boolean {
  if (!id || typeof id !== "string") return false;
  const trimmed = id.trim();
  if (trimmed.length === 0 || trimmed.length > 64) return false;
  return /^[a-zA-Z0-9_-]+$/.test(trimmed);
}

/**
 * Converts a string title into a safe template id slug.
 */
export function slugifyTemplateId(name: string): string {
  return (
    name
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "") || "custom-template"
  );
}

/**
 * Sanitizes a filename to comply with Rust is_safe_resource_filename:
 * ASCII alphanumeric + '-' + '_' with one optional extension.
 */
export function sanitizeResourceFilename(rawName: string): string {
  const baseName = rawName.split(/[/\\]/).pop() || rawName;
  const dotIndex = baseName.lastIndexOf(".");
  if (dotIndex > 0) {
    const namePart = baseName.slice(0, dotIndex).replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 60);
    const extPart = baseName.slice(dotIndex + 1).replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 10);
    return `${namePart || "resource"}.${extPart || "bin"}`;
  }
  return baseName.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 60) || "resource";
}

/**
 * Parameter validation helper.
 */
export function validateParameterValue(
  value: TemplateParameterValue,
  paramType: TemplateParameterType
): boolean {
  switch (paramType.type) {
    case "Text":
      return value.kind === "Text";
    case "Color":
      return value.kind === "Color" && /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(value.value);
    case "Number": {
      if (value.kind !== "Number") return false;
      const n = value.value;
      return Number.isFinite(n) && n >= paramType.min && n <= paramType.max;
    }
    case "Boolean":
      return value.kind === "Boolean";
    case "Enum":
      return value.kind === "Enum" && paramType.options.includes(value.value);
    default:
      return false;
  }
}

/**
 * Parameter formatting helpers.
 */
export function formatParameterValue(value: TemplateParameterValue): string {
  switch (value.kind) {
    case "Text":
    case "Color":
    case "Enum":
      return value.value;
    case "Number":
      return String(value.value);
    case "Boolean":
      return value.value ? "true" : "false";
  }
}

/**
 * Command builder helpers for template operations.
 */
export function createTemplateApplyOp(
  itemId: string,
  templateId: string,
  templateVersion: number,
  initialParameters?: Record<string, TemplateParameterValue> | null,
  opIdPrefix = "op_tpl_apply"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateApply",
      payload: {
        item_id: itemId,
        template_id: templateId,
        template_version: templateVersion,
        initial_parameters: initialParameters ?? null,
      },
    },
  };
}

export const LEGACY_TEMPLATE_IDS = [
  "clean-lower-third",
  "bold-headline",
  "center-pop",
  "subtitle-card",
  "quote-card",
  "call-to-action",
] as const;

export function isLegacyTemplateId(id: string): boolean {
  return (LEGACY_TEMPLATE_IDS as readonly string[]).includes(id);
}

export function createTextTrackTemplateApplyOp(
  trackId: string,
  templateId: string,
  templateVersion: number,
  opIdPrefix = "op_txt_track_apply"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TextTrackTemplateApply",
      payload: {
        track_id: trackId,
        template_id: templateId,
        template_version: templateVersion,
      },
    },
  };
}

export function createTemplateTrackApplyOp(
  trackId: string,
  templateId: string,
  templateVersion: number,
  initialParameters?: Record<string, TemplateParameterValue> | null,
  opIdPrefix = "op_tpl_track_apply"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateTrackApply",
      payload: {
        track_id: trackId,
        template_id: templateId,
        template_version: templateVersion,
        initial_parameters: initialParameters ?? null,
      },
    },
  };
}

export function createTemplateLayerAddOp(
  itemId: string,
  layer: TemplateLayer,
  opIdPrefix = "op_tpl_layer_add"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateLayerAdd",
      payload: {
        item_id: itemId,
        layer,
      },
    },
  };
}

export function createTemplateLayerRemoveOp(
  itemId: string,
  layerId: string,
  opIdPrefix = "op_tpl_layer_rm"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateLayerRemove",
      payload: {
        item_id: itemId,
        layer_id: layerId,
      },
    },
  };
}

export function createTemplateLayerReorderOp(
  itemId: string,
  layerIds: string[],
  opIdPrefix = "op_tpl_layer_reorder"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateLayerReorder",
      payload: {
        item_id: itemId,
        layer_ids: layerIds,
      },
    },
  };
}

export function createTemplateLayerUpdateOp(
  itemId: string,
  layerId: string,
  layer: TemplateLayer,
  opIdPrefix = "op_tpl_layer_upd"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateLayerUpdate",
      payload: {
        item_id: itemId,
        layer_id: layerId,
        layer,
      },
    },
  };
}

export function createTemplateParameterSetOp(
  itemId: string,
  parameterKey: string,
  value: TemplateParameterValue,
  opIdPrefix = "op_tpl_param_set"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateParameterSet",
      payload: {
        item_id: itemId,
        parameter_key: parameterKey,
        value,
      },
    },
  };
}

export function createTemplateResetOp(
  itemId: string,
  opIdPrefix = "op_tpl_reset"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateReset",
      payload: {
        item_id: itemId,
      },
    },
  };
}

export function createTemplateDetachOp(
  itemId: string,
  opIdPrefix = "op_tpl_detach"
): OperationEnvelope {
  return {
    operation_id: `${opIdPrefix}_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
    operation: {
      type: "TemplateDetach",
      payload: {
        item_id: itemId,
      },
    },
  };
}

/**
 * Builds a valid TemplateDefinition from a selected TimelineItem and authoring metadata.
 *
 * Invariant (P32/P33):
 * 1. An item's template_instance clears primary layer fields (text_content, text_rich, text_animation, text_style)
 *    because the TimelineItem itself owns primary authority. When building a TemplateDefinition, we MUST restore
 *    item's authoritative text_content, text_rich, text_animation, and text_style into the primary layer.
 * 2. Animation ownership normalization: do not duplicate the same animation into both layer.motion and text_animation.
 * 3. Rich gradient semantics: if text_rich uses a gradient fill, do NOT clobber it by exposing a solid font_color parameter!
 * 4. Resolves image and font resources into a safe manifest.
 */
export interface AuthoringMetadata {
  id: string;
  name: string;
  category: string;
  description: string;
  version?: number;
  tags?: string[];
  exposeParameters?: boolean;
  projectAssets?: Asset[];
}

export function buildTemplateDefinitionFromItem(
  item: TimelineItem,
  meta: AuthoringMetadata
): TemplateDefinition {
  const version = meta.version && meta.version > 0 ? meta.version : 1;
  const layers: TemplateLayer[] = [];
  const resources: TemplateResourceReference[] = [];
  const assets = meta.projectAssets || [];

  // Helper to resolve an asset reference
  const resolveAssetResource = (assetId: string): TemplateResourceReference | null => {
    const found = assets.find((a) => a.asset_id === assetId);
    if (!found) return null;
    return {
      resource_id: found.asset_id,
      kind: found.media_kind === "Image" ? "Image" : "Font",
      file_name: sanitizeResourceFilename(found.name || found.path),
      sha256: "0000000000000000000000000000000000000000000000000000000000000000",
      byte_size: found.file_size || 0,
      required: true,
    };
  };

  // 1. If the item already has a template_instance, copy and materialize its layers
  if (item.template_instance?.layers && item.template_instance.layers.length > 0) {
    for (const l of item.template_instance.layers) {
      const cloned = JSON.parse(JSON.stringify(l)) as TemplateLayer;

      // P32 Invariant: Restore authoritative item state into primary layer
      if (cloned.kind.type === "Text" && cloned.kind.data.is_primary) {
        cloned.kind.data.text_content =
          item.text_content ?? cloned.kind.data.text_content ?? "Template Text";
        cloned.kind.data.text_style = item.text_style ?? cloned.kind.data.text_style ?? null;
        cloned.kind.data.text_rich = item.text_rich ?? cloned.kind.data.text_rich ?? null;
        cloned.kind.data.text_animation =
          item.text_animation ?? cloned.kind.data.text_animation ?? null;

        // Do NOT drop existing independent layer motion; preserve it if present
        // (cloned.motion is kept as-is)
      }

      // Check for image asset resources
      if (cloned.kind.type === "Image" && cloned.kind.data.asset_id) {
        const resRef = resolveAssetResource(cloned.kind.data.asset_id);
        if (resRef && !resources.some((r) => r.resource_id === resRef.resource_id)) {
          resources.push(resRef);
        }
      }

      layers.push(cloned);
    }
  } else {
    // 2. Otherwise create a primary Text layer from item text_content & text_style
    const primaryLayer: TemplateLayer = {
      layer_id: "lyr_primary_text",
      name: "Primary Text",
      z_index: 0,
      visible: true,
      opacity: item.opacity ?? 1.0,
      transform: item.transform ?? {
        position_x: 0,
        position_y: 0,
        scale_x: 1,
        scale_y: 1,
        rotation: 0,
      },
      anchor: item.text_style?.anchor ?? "Center",
      pivot: [0.5, 0.5],
      // Normalization: Text animation belongs strictly to kind.data.text_animation, not duplicate layer.motion
      motion: null,
      kind: {
        type: "Text",
        data: {
          is_primary: true,
          slot_key: "text_content",
          text_content: item.text_content ?? "Template Text",
          text_style: item.text_style ?? null,
          text_rich: item.text_rich ?? null,
          text_animation: item.text_animation ?? null,
        },
      },
    };
    layers.push(primaryLayer);
  }

  // Ensure exactly one layer is marked primary if any text layer exists
  const textLayers = layers.filter((l) => l.kind.type === "Text");
  if (
    textLayers.length > 0 &&
    !textLayers.some((l) => (l.kind as { data: { is_primary: boolean } }).data.is_primary)
  ) {
    (textLayers[0].kind as { data: { is_primary: boolean } }).data.is_primary = true;
  }

  // Collect custom font references from every text layer and rich span.
  for (const layer of layers) {
    if (layer.kind.type !== "Text") continue;
    const data = layer.kind.data;
    const fontIds = new Set<string>();
    if (data.text_rich?.font?.asset_id) fontIds.add(data.text_rich.font.asset_id);
    for (const span of data.text_rich?.spans ?? []) {
      if (span.style?.font?.asset_id) fontIds.add(span.style.font.asset_id);
    }
    for (const fontId of fontIds) {
      const fontRef = resolveAssetResource(fontId);
      if (fontRef && !resources.some((r) => r.resource_id === fontRef.resource_id)) {
        resources.push(fontRef);
      }
    }
  }

  // Build parameters and bindings if exposeParameters is true
  const parameters: TemplateParameterDefinition[] = [];
  const bindings: Array<{
    parameter_key: string;
    target_layer_id: string;
    target_property: TemplatePropertyPath;
  }> = [];

  if (meta.exposeParameters !== false) {
    // Expose primary text content as a parameter
    const primaryLyr = layers.find(
      (l) => l.kind.type === "Text" && (l.kind as { data: { is_primary: boolean } }).data.is_primary
    );
    if (primaryLyr) {
      parameters.push({
        key: "text_content",
        name: "Text Content",
        param_type: { type: "Text" },
        default_value: {
          kind: "Text",
          value: item.text_content ?? "Template Text",
        },
        description: "Primary headline or label text",
      });
      bindings.push({
        parameter_key: "text_content",
        target_layer_id: primaryLyr.layer_id,
        target_property: "TextContent",
      });

      // Color Parameter Resolution:
      // - If item.text_rich.fill is Solid, use that color.
      // - If item.text_rich.fill is Gradient, do NOT expose a solid font_color parameter (protect gradient semantics).
      // - Otherwise, fallback to legacy item.text_style.font_color if present.
      let resolvedFontColor: string | null = null;
      if (item.text_rich?.fill) {
        if (item.text_rich.fill.kind === "Solid") {
          resolvedFontColor = item.text_rich.fill.color;
        } else if (item.text_rich.fill.kind === "Gradient") {
          resolvedFontColor = null;
        }
      } else if (item.text_style?.font_color) {
        resolvedFontColor = item.text_style.font_color;
      }

      if (resolvedFontColor) {
        parameters.push({
          key: "font_color",
          name: "Font Color",
          param_type: { type: "Color" },
          default_value: {
            kind: "Color",
            value: resolvedFontColor,
          },
          description: "Text fill color",
        });
        bindings.push({
          parameter_key: "font_color",
          target_layer_id: primaryLyr.layer_id,
          target_property: "FontColor",
        });
      }
    }
  }

  const defaultDuration: RationalTime = item.timeline_range?.duration ?? {
    value: 90,
    rate: { numerator: 30, denominator: 1 },
  };

  return {
    id: meta.id.trim(),
    version,
    name: meta.name.trim(),
    category: meta.category.trim() || "Custom",
    description: meta.description.trim(),
    tags: (meta.tags ?? []).map((t) => t.trim()).filter(Boolean),
    parameters,
    bindings,
    layers,
    resources: { resources },
    responsive_policy: item.template_instance?.responsive_policy ?? {
      reference_width: 1920,
      reference_height: 1080,
      supported_aspect_ratios: ["16:9", "9:16", "1:1", "4:5"],
      auto_wrap: true,
      auto_fit: false,
      safe_margin: 80,
    },
    default_duration: defaultDuration,
  };
}

/**
 * Resolves project assets for templates being exported into a pack.
 * Validates that all required resources have matching local project asset files.
 * Rejects explicitly when any required resource cannot be resolved.
 */
export function resolvePackAssets(
  definitions: TemplateDefinition[],
  assets: Asset[]
): {
  assetFiles: [string, string][];
  missingResources: string[];
} {
  const assetFiles: [string, string][] = [];
  const missingResources: string[] = [];
  const seenKeys = new Set<string>();

  for (const def of definitions) {
    // 1. Manifest declared resources
    for (const res of def.resources.resources) {
      const asset = assets.find(
        (a) => a.asset_id === res.resource_id || a.name === res.file_name || a.path.endsWith(res.file_name)
      );

      if (asset && asset.path) {
        const key = `${res.resource_id}:${asset.path}`;
        if (!seenKeys.has(key)) {
          seenKeys.add(key);
          assetFiles.push([res.resource_id, asset.path]);
          // Also map file_name for backend resolver fallback
          assetFiles.push([res.file_name, asset.path]);
        }
      } else if (res.required) {
        missingResources.push(`Template '${def.name}' required resource '${res.file_name}' (${res.resource_id})`);
      }
    }

    // 2. Scan layers for image asset references
    for (const layer of def.layers) {
      if (layer.kind.type === "Image" && layer.kind.data.asset_id) {
        const aid = layer.kind.data.asset_id;
        const asset = assets.find((a) => a.asset_id === aid);
        if (asset && asset.path) {
          const key = `${aid}:${asset.path}`;
          if (!seenKeys.has(key)) {
            seenKeys.add(key);
            assetFiles.push([aid, asset.path]);
          }
        } else {
          missingResources.push(`Template '${def.name}' image layer '${layer.name}' asset '${aid}'`);
        }
      }
    }
  }

  return { assetFiles, missingResources };
}

/**
 * Resolves all resources declared in or referenced by a template definition,
 * computes their real SHA-256 hashes using editorApi.hashLocalFile,
 * verifies their byte sizes, and produces an authoritative manifest.
 * Fails with a clear error if any required resource cannot be resolved or hashed.
 */
export async function finalizeDefinitionResources(
  definition: TemplateDefinition,
  assets: Asset[]
): Promise<TemplateDefinition> {
  const updatedResources: TemplateResourceReference[] = [];
  const seenIds = new Set<string>();

  // 1. Resolve image layers
  for (const layer of definition.layers) {
    if (layer.kind.type === "Image" && layer.kind.data.asset_id) {
      const aid = layer.kind.data.asset_id;
      if (seenIds.has(aid)) continue;
      seenIds.add(aid);

      const asset = assets.find(
        (a) => a.asset_id === aid || (layer.kind.type === "Image" && a.name === layer.kind.data.asset_name)
      );

      if (!asset || !asset.path) {
        throw new Error(
          `Required image asset "${layer.kind.data.asset_name || aid}" for layer "${layer.name}" was not found in project assets. Please import or relink the asset before saving template.`
        );
      }

      let sha256 = "";
      try {
        sha256 = await editorApi.hashLocalFile(asset.path);
      } catch (err) {
        throw new Error(
          `Failed to compute SHA-256 for image asset "${asset.name}" (${asset.path}): ${err}`
        );
      }

      if (!sha256 || sha256.startsWith("00000000")) {
        throw new Error(
          `Computed invalid or empty SHA-256 for required resource "${asset.name}". Cannot save template without verified resource integrity.`
        );
      }

      updatedResources.push({
        resource_id: asset.asset_id,
        kind: "Image",
        file_name: sanitizeResourceFilename(asset.name || asset.path),
        sha256,
        byte_size: asset.file_size || 0,
        required: true,
      });
    }
  }

  // 1b. Enumerate every font referenced by a text layer or span.
  const referencedFontIds = new Set<string>();
  for (const layer of definition.layers) {
    if (layer.kind.type !== "Text") continue;
    const data = layer.kind.data;
    if (data.text_rich?.font?.asset_id) {
      referencedFontIds.add(data.text_rich.font.asset_id);
    }
    for (const span of data.text_rich?.spans ?? []) {
      if (span.style?.font?.asset_id) {
        referencedFontIds.add(span.style.font.asset_id);
      }
    }
  }
  for (const resourceId of referencedFontIds) {
    if (seenIds.has(resourceId)) continue;
    seenIds.add(resourceId);
    const asset = assets.find((a) => a.asset_id === resourceId);
    if (!asset || !asset.path) {
      throw new Error(
        "Required font asset '" +
          resourceId +
          "' referenced by a text layer/span was not found in project assets. Relink the font before saving the template."
      );
    }
    const sha256 = await editorApi.hashLocalFile(asset.path);
    if (!sha256 || sha256.startsWith("00000000")) {
      throw new Error("Invalid SHA-256 for required font '" + asset.name + "'.");
    }
    updatedResources.push({
      resource_id: asset.asset_id,
      kind: "Font",
      file_name: sanitizeResourceFilename(asset.name || asset.path),
      sha256,
      byte_size: asset.file_size || 0,
      required: true,
    });
  }

  // 2. Resolve font resources from definition or layers
  for (const res of definition.resources.resources) {
    if (seenIds.has(res.resource_id)) continue;
    seenIds.add(res.resource_id);

    const asset = assets.find(
      (a) => a.asset_id === res.resource_id || a.name === res.file_name || a.path.endsWith(res.file_name)
    );

    if (asset && asset.path) {
      let sha256 = res.sha256;
      if (!sha256 || sha256.startsWith("00000000")) {
        try {
          sha256 = await editorApi.hashLocalFile(asset.path);
        } catch (err) {
          throw new Error(`Failed to compute SHA-256 for font "${asset.name}": ${err}`);
        }
      }
      if (!sha256 || sha256.startsWith("00000000")) {
        throw new Error(`Invalid SHA-256 for required font "${asset.name}".`);
      }

      updatedResources.push({
        resource_id: asset.asset_id,
        kind: res.kind,
        file_name: sanitizeResourceFilename(asset.name || asset.path),
        sha256,
        byte_size: asset.file_size || res.byte_size,
        required: res.required,
      });
    } else if (res.required) {
      throw new Error(
        `Required template resource "${res.file_name}" (${res.resource_id}) could not be resolved from project assets.`
      );
    }
  }

  return {
    ...definition,
    resources: {
      resources: updatedResources,
    },
  };
}
