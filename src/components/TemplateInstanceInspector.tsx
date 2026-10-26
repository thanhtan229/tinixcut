import React, { useState, useEffect, useRef } from "react";
import { TimelineItem } from "../types/project";
import {
  TemplateDefinition,
  TemplateLayer,
  TemplateParameterDefinition,
  TemplateParameterValue,
  TemplateShapeKind,
} from "../types/template";
import { editorApi } from "../services/editorApi";
import { validateParameterValue } from "../utils/templateHelpers";

interface TemplateInstanceInspectorProps {
  item: TimelineItem;
  onParameterSet?: (key: string, value: TemplateParameterValue) => Promise<any> | void;
  onReset?: () => void;
  onDetach?: () => void;
  onLayerAdd?: (layer: TemplateLayer) => void;
  onLayerRemove?: (layerId: string) => void;
  onLayerReorder?: (layerIds: string[]) => void;
  onLayerUpdate?: (layerId: string, patch: Partial<TemplateLayer>) => Promise<unknown> | void;
  onSaveAsTemplate?: () => void;
}

export const TemplateInstanceInspector: React.FC<TemplateInstanceInspectorProps> = ({
  item,
  onParameterSet,
  onReset,
  onDetach,
  onLayerAdd,
  onLayerRemove,
  onLayerReorder,
  onLayerUpdate,
  onSaveAsTemplate,
}) => {
  const instance = item.template_instance;

  // Progressive disclosure collapsible states
  const [paramsExpanded, setParamsExpanded] = useState(true);
  const [layersExpanded, setLayersExpanded] = useState(true);
  const [policyExpanded, setPolicyExpanded] = useState(false);
  const [selectedLayerId, setSelectedLayerId] = useState<string | null>(null);

  // Definition lookup state (for exact declared parameter constraints)
  const [definition, setDefinition] = useState<TemplateDefinition | null>(null);
  const [defLoading, setDefLoading] = useState(false);
  const [defNotFound, setDefNotFound] = useState(false);

  // Transient drafts for parameter editing to prevent per-keystroke command flood & excess history
  const [paramDrafts, setParamDrafts] = useState<Record<string, TemplateParameterValue>>({});
  const lastCommittedParamRef = useRef<Record<string, TemplateParameterValue>>({});

  // Transient drafts for layer detail editing
  const [layerDrafts, setLayerDrafts] = useState<Record<string, Partial<TemplateLayer>>>({});
  const commitSignaturesRef = useRef<Set<string>>(new Set());
  const draftGenerationRef = useRef<Record<string, number>>({});

  useEffect(() => {
    let active = true;
    if (!instance?.template_id) {
      setDefinition(null);
      setDefNotFound(false);
      return;
    }

    setDefLoading(true);
    setDefNotFound(false);
    editorApi
      .templateGetDefinition(instance.template_id, instance.template_version)
      .then((def) => {
        if (!active) return;
        if (def) {
          setDefinition(def);
          setDefNotFound(false);
        } else {
          setDefinition(null);
          setDefNotFound(true);
        }
      })
      .catch(() => {
        if (!active) return;
        setDefinition(null);
        setDefNotFound(true);
      })
      .finally(() => {
        if (active) setDefLoading(false);
      });

    return () => {
      active = false;
    };
  }, [instance?.template_id, instance?.template_version]);

  // Synchronize parameter drafts when external instance parameters update
  useEffect(() => {
    if (instance?.parameters) {
      setParamDrafts(instance.parameters);
      lastCommittedParamRef.current = instance.parameters;
    }
  }, [instance?.parameters]);

  if (!instance) {
    return (
      <div style={{ padding: "8px 0", borderTop: "1px solid var(--border-color)" }}>
        <button
          type="button"
          className="btn-action"
          onClick={onSaveAsTemplate}
          style={{
            width: "100%",
            padding: "6px 10px",
            fontSize: "11px",
            fontWeight: 600,
            backgroundColor: "var(--bg-surface)",
            color: "var(--accent-primary)",
            border: "1px solid var(--border-color)",
            borderRadius: "4px",
            cursor: "pointer",
          }}
        >
          💾 Save as Reusable Template...
        </button>
      </div>
    );
  }

  const paramEntries = Object.entries(instance.parameters || {});
  const layers = instance.layers || [];
  const policy = instance.responsive_policy;

  // Helper to commit a parameter value only if changed from committed state
  const commitParam = async (key: string, value: TemplateParameterValue) => {
    const prev = lastCommittedParamRef.current[key];
    const unchanged =
      prev &&
      prev.kind === value.kind &&
      (prev as any).value === (value as any).value;
    if (unchanged) return;

    if (onParameterSet) {
      try {
        await onParameterSet(key, value);
        lastCommittedParamRef.current = { ...lastCommittedParamRef.current, [key]: value };
      } catch (err) {
        console.error(`Failed to commit parameter ${key}:`, err);
        // revert draft on failure
        if (prev) {
          setParamDrafts((d) => ({ ...d, [key]: prev }));
        }
      }
    }
  };

  // Helper to get freshest layer draft merging canonical layer with local transient edits
  const getLayerDraft = (layer: TemplateLayer): TemplateLayer => {
    const patch = layerDrafts[layer.layer_id];
    if (!patch) return layer;
    return {
      ...layer,
      ...patch,
      transform: patch.transform ? { ...layer.transform, ...patch.transform } : layer.transform,
      kind: patch.kind ? patch.kind : layer.kind,
    };
  };

  // Stage a local change for a layer without committing command
  const updateLayerDraft = (layerId: string, patch: Partial<TemplateLayer>) => {
    draftGenerationRef.current[layerId] = (draftGenerationRef.current[layerId] ?? 0) + 1;
    setLayerDrafts((prev) => ({
      ...prev,
      [layerId]: {
        ...(prev[layerId] || {}),
        ...patch,
      },
    }));
  };

  // Commit only the changed fields. App resolves them against canonical project
  // state inside the serialized command queue, so queued edits cannot restore
  // stale sibling fields.
  const commitLayerPatch = async (layerId: string, patch: Partial<TemplateLayer>) => {
    if (!onLayerUpdate) return;
    const signature = layerId + ":" + JSON.stringify(patch);
    if (commitSignaturesRef.current.has(signature)) return;
    commitSignaturesRef.current.add(signature);
    const committedGeneration = draftGenerationRef.current[layerId] ?? 0;

    try {
      await onLayerUpdate(layerId, patch);
      if ((draftGenerationRef.current[layerId] ?? 0) === committedGeneration) {
        setLayerDrafts((prev) => {
          const current = prev[layerId];
          if (!current) return prev;
          const nextDraft: Partial<TemplateLayer> = { ...current };
          for (const key of Object.keys(patch) as Array<keyof TemplateLayer>) {
            delete nextDraft[key];
          }
          const next = { ...prev };
          if (Object.keys(nextDraft).length === 0) {
            delete next[layerId];
          } else {
            next[layerId] = nextDraft;
          }
          return next;
        });
      }
    } catch (err) {
      console.error("Failed to commit layer patch for " + layerId + ":", err);
    } finally {
      setTimeout(() => commitSignaturesRef.current.delete(signature), 250);
    }
  };

  const handleMoveLayer = (index: number, direction: "up" | "down") => {
    if (!onLayerReorder) return;
    const targetIndex = direction === "up" ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= layers.length) return;

    const newOrder = [...layers];
    const [moved] = newOrder.splice(index, 1);
    newOrder.splice(targetIndex, 0, moved);
    onLayerReorder(newOrder.map((l) => l.layer_id));
  };

  const handleAddDefaultShapeLayer = (shapeKind: TemplateShapeKind = "RoundedRectangle") => {
    if (!onLayerAdd) return;
    const now = Date.now();
    const newLayer: TemplateLayer = {
      layer_id: `lyr_shape_${now}`,
      name: `Shape Layer ${layers.length + 1}`,
      z_index: layers.length,
      visible: true,
      opacity: 0.85,
      transform: {
        position_x: 0,
        position_y: 0,
        scale_x: 1,
        scale_y: 1,
        rotation: 0,
      },
      anchor: "Center",
      pivot: [0.5, 0.5],
      kind: {
        type: "Shape",
        data: {
          shape_kind: shapeKind,
          width: 480,
          height: 120,
          fill_color: "#1E293B",
          stroke_color: "#3B82F6",
          stroke_width: 2,
          corner_radius: 8,
        },
      },
    };
    onLayerAdd(newLayer);
    setSelectedLayerId(newLayer.layer_id);
  };

  const handleAddSecondaryTextLayer = () => {
    if (!onLayerAdd) return;
    const now = Date.now();
    const newLayer: TemplateLayer = {
      layer_id: `lyr_txt_${now}`,
      name: `Subtext ${layers.length + 1}`,
      z_index: layers.length + 1,
      visible: true,
      opacity: 1.0,
      transform: {
        position_x: 0,
        position_y: 40,
        scale_x: 1,
        scale_y: 1,
        rotation: 0,
      },
      anchor: "Center",
      pivot: [0.5, 0.5],
      kind: {
        type: "Text",
        data: {
          is_primary: false,
          slot_key: `slot_${now}`,
          text_content: "Secondary Text",
          text_style: {
            font_size: 28,
            font_weight: 500,
            font_color: "#E2E8F0",
            alignment: "Center",
            anchor: "Center",
            font_family: "Roboto",
            padding_x: 0,
            padding_y: 0,
            line_height: 1.2,
            background_color: null,
            background_opacity: 0,
          },
        },
      },
    };
    onLayerAdd(newLayer);
    setSelectedLayerId(newLayer.layer_id);
  };

  return (
    <div
      className="template-instance-inspector"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "10px",
        padding: "10px 0",
        borderTop: "1px solid var(--border-color)",
      }}
    >
      {/* Template Header / Identity */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "8px",
          padding: "6px 8px",
          backgroundColor: "rgba(59, 130, 246, 0.1)",
          border: "1px solid rgba(59, 130, 246, 0.25)",
          borderRadius: "4px",
        }}
      >
        <div style={{ display: "flex", flexDirection: "column" }}>
          <span style={{ fontSize: "11px", fontWeight: 700, color: "var(--accent-primary)" }}>
            🎨 {definition?.name ?? instance.template_id}
          </span>
          <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
            v{instance.template_version} • {layers.length} layers • {paramEntries.length} params
          </span>
        </div>

        <div style={{ display: "flex", gap: "4px" }}>
          {onReset && (
            <button
              type="button"
              className="btn-action"
              onClick={onReset}
              title="Reset template parameters and layers to catalog defaults"
              style={{
                padding: "3px 6px",
                fontSize: "10px",
                backgroundColor: "var(--bg-surface)",
                color: "var(--text-main)",
                border: "1px solid var(--border-color)",
                borderRadius: "3px",
                cursor: "pointer",
              }}
            >
              Reset
            </button>
          )}
          {onDetach && (
            <button
              type="button"
              className="btn-action"
              onClick={onDetach}
              title="Detach template, leaving current styles as independent clip"
              style={{
                padding: "3px 6px",
                fontSize: "10px",
                backgroundColor: "var(--bg-surface)",
                color: "var(--text-muted)",
                border: "1px solid var(--border-color)",
                borderRadius: "3px",
                cursor: "pointer",
              }}
            >
              Detach
            </button>
          )}
        </div>
      </div>

      {/* Missing definition warning banner */}
      {defNotFound && !defLoading && (
        <div
          role="status"
          style={{
            padding: "6px 8px",
            fontSize: "10px",
            backgroundColor: "rgba(234, 179, 8, 0.12)",
            border: "1px solid rgba(234, 179, 8, 0.4)",
            borderRadius: "4px",
            color: "var(--warning, #eab308)",
            lineHeight: 1.4,
          }}
        >
          ⚠️ Template definition <code>{instance.template_id}</code> (v{instance.template_version}) could not be found in the catalog. Using stored instance parameters.
        </div>
      )}

      {/* Progressive Disclosure Section 1: Parameters */}
      <section style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <button
          type="button"
          onClick={() => setParamsExpanded(!paramsExpanded)}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            background: "none",
            border: "none",
            color: "var(--text-muted)",
            fontSize: "11px",
            fontWeight: 600,
            cursor: "pointer",
            padding: "2px 0",
            textAlign: "left",
          }}
        >
          <span>TEMPLATE PARAMETERS ({paramEntries.length})</span>
          <span>{paramsExpanded ? "▼" : "▶"}</span>
        </button>

        {paramsExpanded && (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", marginTop: "2px" }}>
            {paramEntries.length === 0 ? (
              <span style={{ fontSize: "11px", color: "var(--text-muted)", fontStyle: "italic" }}>
                No customizable parameters declared for this template.
              </span>
            ) : (
              paramEntries.map(([key, val]) => {
                const paramDef: TemplateParameterDefinition | undefined =
                  definition?.parameters.find((p) => p.key === key);
                const label =
                  paramDef?.name ||
                  key.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
                const draft = paramDrafts[key] ?? val;

                switch (val.kind) {
                  case "Text": {
                    const textVal = draft.kind === "Text" ? draft.value : val.value;
                    return (
                      <label
                        key={key}
                        style={{
                          display: "flex",
                          flexDirection: "column",
                          gap: "4px",
                          fontSize: "11px",
                          color: "var(--text-muted)",
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between" }}>
                          <span>{label}</span>
                          {paramDef?.description && (
                            <span style={{ fontSize: "9px", opacity: 0.7 }}>
                              {paramDef.description}
                            </span>
                          )}
                        </div>
                        <input
                          type="text"
                          value={textVal}
                          onChange={(e) => {
                            setParamDrafts((prev) => ({
                              ...prev,
                              [key]: { kind: "Text", value: e.target.value },
                            }));
                          }}
                          onBlur={() => {
                            commitParam(key, { kind: "Text", value: textVal });
                          }}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              commitParam(key, { kind: "Text", value: textVal });
                            }
                          }}
                          style={{
                            padding: "4px 6px",
                            fontSize: "12px",
                            backgroundColor: "var(--bg-surface)",
                            color: "var(--text-main)",
                            border: "1px solid var(--border-color)",
                            borderRadius: "3px",
                          }}
                        />
                      </label>
                    );
                  }

                  case "Color": {
                    const colorVal = draft.kind === "Color" ? draft.value : val.value;
                    return (
                      <label
                        key={key}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "space-between",
                          gap: "8px",
                          fontSize: "11px",
                          color: "var(--text-muted)",
                        }}
                      >
                        <span>{label}</span>
                        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                          <input
                            type="text"
                            value={colorVal}
                            onChange={(e) => {
                              const upper = e.target.value.toUpperCase();
                              setParamDrafts((prev) => ({
                                ...prev,
                                [key]: { kind: "Color", value: upper },
                              }));
                            }}
                            onBlur={() => {
                              if (/^#([0-9A-F]{6}|[0-9A-F]{8})$/.test(colorVal)) {
                                commitParam(key, { kind: "Color", value: colorVal });
                              }
                            }}
                            onKeyDown={(e) => {
                              if (e.key === "Enter" && /^#([0-9A-F]{6}|[0-9A-F]{8})$/.test(colorVal)) {
                                commitParam(key, { kind: "Color", value: colorVal });
                              }
                            }}
                            style={{
                              width: "70px",
                              padding: "2px 4px",
                              fontSize: "10px",
                              fontFamily: "monospace",
                              backgroundColor: "var(--bg-surface)",
                              color: "var(--text-main)",
                              border: "1px solid var(--border-color)",
                              borderRadius: "3px",
                            }}
                          />
                          <input
                            type="color"
                            value={colorVal.slice(0, 7)}
                            onChange={(e) => {
                              const upper = e.target.value.toUpperCase();
                              setParamDrafts((prev) => ({
                                ...prev,
                                [key]: { kind: "Color", value: upper },
                              }));
                            }}
                            onBlur={() => {
                              commitParam(key, { kind: "Color", value: colorVal });
                            }}
                            onPointerUp={() => {
                              commitParam(key, { kind: "Color", value: colorVal });
                            }}
                            style={{
                              width: "32px",
                              height: "24px",
                              padding: "1px",
                              border: "none",
                              cursor: "pointer",
                            }}
                          />
                        </div>
                      </label>
                    );
                  }

                  case "Number": {
                    // Support negative and fractional ranges from definition constraints
                    const numberConstraint =
                      paramDef?.param_type.type === "Number" ? paramDef.param_type : null;
                    const min = numberConstraint?.min ?? 0;
                    const max = numberConstraint?.max ?? 200;
                    const step = numberConstraint?.step ?? 1;

                    const numVal = draft.kind === "Number" ? draft.value : val.value;

                    return (
                      <div
                        key={key}
                        style={{ display: "flex", flexDirection: "column", gap: "4px" }}
                      >
                        <div
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            fontSize: "11px",
                            color: "var(--text-muted)",
                          }}
                        >
                          <span>{label}</span>
                          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                            <input
                              type="number"
                              min={min}
                              max={max}
                              step={step}
                              value={numVal}
                              onChange={(e) => {
                                const parsed = parseFloat(e.target.value);
                                const valid = Number.isFinite(parsed) ? parsed : 0;
                                setParamDrafts((prev) => ({
                                  ...prev,
                                  [key]: { kind: "Number", value: valid },
                                }));
                              }}
                              onBlur={() => {
                                const clamped = Math.max(min, Math.min(max, numVal));
                                commitParam(key, { kind: "Number", value: clamped });
                              }}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") {
                                  const clamped = Math.max(min, Math.min(max, numVal));
                                  commitParam(key, { kind: "Number", value: clamped });
                                }
                              }}
                              style={{
                                width: "60px",
                                padding: "2px 4px",
                                fontSize: "11px",
                                backgroundColor: "var(--bg-surface)",
                                color: "var(--text-main)",
                                border: "1px solid var(--border-color)",
                                borderRadius: "3px",
                                textAlign: "right",
                              }}
                            />
                            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                              [{min}..{max}]
                            </span>
                          </div>
                        </div>
                        <input
                          type="range"
                          min={min}
                          max={max}
                          step={step}
                          value={numVal}
                          onChange={(e) => {
                            const parsed = parseFloat(e.target.value) || 0;
                            setParamDrafts((prev) => ({
                              ...prev,
                              [key]: { kind: "Number", value: parsed },
                            }));
                          }}
                          onPointerUp={() => {
                            commitParam(key, { kind: "Number", value: numVal });
                          }}
                          onKeyUp={() => {
                            commitParam(key, { kind: "Number", value: numVal });
                          }}
                          style={{ width: "100%", cursor: "pointer" }}
                        />
                      </div>
                    );
                  }

                  case "Boolean": {
                    const boolVal = draft.kind === "Boolean" ? draft.value : val.value;
                    return (
                      <label
                        key={key}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: "8px",
                          fontSize: "11px",
                          color: "var(--text-main)",
                          cursor: "pointer",
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={boolVal}
                          onChange={(e) => {
                            const next = e.target.checked;
                            setParamDrafts((prev) => ({
                              ...prev,
                              [key]: { kind: "Boolean", value: next },
                            }));
                            commitParam(key, { kind: "Boolean", value: next });
                          }}
                        />
                        <span>{label}</span>
                      </label>
                    );
                  }

                  case "Enum": {
                    const enumVal = draft.kind === "Enum" ? draft.value : val.value;
                    const enumConstraint =
                      paramDef?.param_type.type === "Enum" ? paramDef.param_type : null;
                    const options = enumConstraint?.options ?? [];

                    return (
                      <label
                        key={key}
                        style={{
                          display: "flex",
                          flexDirection: "column",
                          gap: "4px",
                          fontSize: "11px",
                          color: "var(--text-muted)",
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between" }}>
                          <span>{label}</span>
                          {options.length > 0 && (
                            <span style={{ fontSize: "9px", opacity: 0.7 }}>
                              {options.length} options
                            </span>
                          )}
                        </div>
                        {options.length > 0 ? (
                          <select
                            value={enumVal}
                            onChange={(e) => {
                              const chosen = e.target.value;
                              if (
                                enumConstraint &&
                                !validateParameterValue(
                                  { kind: "Enum", value: chosen },
                                  enumConstraint
                                )
                              ) {
                                return;
                              }
                              setParamDrafts((prev) => ({
                                ...prev,
                                [key]: { kind: "Enum", value: chosen },
                              }));
                              commitParam(key, { kind: "Enum", value: chosen });
                            }}
                            style={{
                              padding: "4px 6px",
                              fontSize: "11px",
                              backgroundColor: "var(--bg-surface)",
                              color: "var(--text-main)",
                              border: "1px solid var(--border-color)",
                              borderRadius: "3px",
                            }}
                          >
                            {options.map((opt) => (
                              <option key={opt} value={opt}>
                                {opt}
                              </option>
                            ))}
                          </select>
                        ) : (
                          <input
                            type="text"
                            value={enumVal}
                            onChange={(e) => {
                              setParamDrafts((prev) => ({
                                ...prev,
                                [key]: { kind: "Enum", value: e.target.value },
                              }));
                            }}
                            onBlur={() => {
                              commitParam(key, { kind: "Enum", value: enumVal });
                            }}
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                commitParam(key, { kind: "Enum", value: enumVal });
                              }
                            }}
                            style={{
                              padding: "4px 6px",
                              fontSize: "12px",
                              backgroundColor: "var(--bg-surface)",
                              color: "var(--text-main)",
                              border: "1px solid var(--border-color)",
                              borderRadius: "3px",
                            }}
                          />
                        )}
                      </label>
                    );
                  }

                  default:
                    return null;
                }
              })
            )}
          </div>
        )}
      </section>

      {/* Progressive Disclosure Section 2: Layers */}
      <section style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <button
          type="button"
          onClick={() => setLayersExpanded(!layersExpanded)}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            background: "none",
            border: "none",
            color: "var(--text-muted)",
            fontSize: "11px",
            fontWeight: 600,
            cursor: "pointer",
            padding: "2px 0",
            textAlign: "left",
          }}
        >
          <span>TEMPLATE LAYERS ({layers.length})</span>
          <span>{layersExpanded ? "▼" : "▶"}</span>
        </button>

        {layersExpanded && (
          <div style={{ display: "flex", flexDirection: "column", gap: "6px", marginTop: "2px" }}>
            {layers.map((layer, index) => {
              const isPrimary =
                layer.kind.type === "Text" && Boolean(layer.kind.data.is_primary);
              const isSelected = selectedLayerId === layer.layer_id;
              const draftLayer = getLayerDraft(layer);

              return (
                <div
                  key={layer.layer_id}
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "4px",
                    backgroundColor: isSelected ? "rgba(59, 130, 246, 0.08)" : "var(--bg-surface)",
                    borderRadius: "4px",
                    border: isSelected
                      ? "1px solid var(--accent-primary)"
                      : "1px solid var(--border-color)",
                    padding: "4px 6px",
                    fontSize: "11px",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                    }}
                  >
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "6px",
                        overflow: "hidden",
                        flex: 1,
                        cursor: "pointer",
                      }}
                      onClick={() =>
                        setSelectedLayerId(isSelected ? null : layer.layer_id)
                      }
                    >
                      {/* Visibility toggle button */}
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          commitLayerPatch(layer.layer_id, { visible: !layer.visible });
                        }}
                        title={layer.visible ? "Hide layer" : "Show layer"}
                        style={{
                          background: "none",
                          border: "none",
                          cursor: "pointer",
                          color: layer.visible ? "var(--accent-primary)" : "var(--text-muted)",
                          padding: "0 2px",
                        }}
                      >
                        {layer.visible ? "👁" : "🚫"}
                      </button>

                      <span
                        style={{
                          fontWeight: isPrimary ? 700 : 500,
                          color: layer.visible ? "var(--text-main)" : "var(--text-muted)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                        title={draftLayer.name}
                      >
                        {draftLayer.name}
                      </span>
                      <span
                        style={{
                          fontSize: "9px",
                          padding: "1px 4px",
                          borderRadius: "2px",
                          backgroundColor: "rgba(255,255,255,0.06)",
                          color: "var(--text-muted)",
                        }}
                      >
                        {layer.kind.type}
                      </span>
                      {isPrimary && (
                        <span
                          style={{
                            fontSize: "9px",
                            padding: "1px 4px",
                            borderRadius: "2px",
                            backgroundColor: "rgba(59, 130, 246, 0.2)",
                            color: "var(--accent-primary)",
                          }}
                        >
                          Primary
                        </span>
                      )}
                    </div>

                    {/* Layer ordering and removal actions */}
                    <div style={{ display: "flex", gap: "2px", alignItems: "center" }}>
                      <button
                        type="button"
                        disabled={index === 0}
                        onClick={() => handleMoveLayer(index, "up")}
                        style={{
                          background: "none",
                          border: "none",
                          cursor: index === 0 ? "default" : "pointer",
                          color: index === 0 ? "rgba(255,255,255,0.2)" : "var(--text-main)",
                          padding: "1px 3px",
                        }}
                        title="Move layer up"
                      >
                        ▲
                      </button>
                      <button
                        type="button"
                        disabled={index === layers.length - 1}
                        onClick={() => handleMoveLayer(index, "down")}
                        style={{
                          background: "none",
                          border: "none",
                          cursor: index === layers.length - 1 ? "default" : "pointer",
                          color:
                            index === layers.length - 1
                              ? "rgba(255,255,255,0.2)"
                              : "var(--text-main)",
                          padding: "1px 3px",
                        }}
                        title="Move layer down"
                      >
                        ▼
                      </button>

                      <button
                        type="button"
                        onClick={() =>
                          setSelectedLayerId(isSelected ? null : layer.layer_id)
                        }
                        title="Edit layer details"
                        style={{
                          background: "none",
                          border: "none",
                          cursor: "pointer",
                          color: isSelected ? "var(--accent-primary)" : "var(--text-muted)",
                          padding: "1px 3px",
                          fontSize: "10px",
                        }}
                      >
                        {isSelected ? "▲" : "✏️"}
                      </button>

                      {!isPrimary && onLayerRemove && (
                        <button
                          type="button"
                          onClick={() => onLayerRemove(layer.layer_id)}
                          style={{
                            background: "none",
                            border: "none",
                            cursor: "pointer",
                            color: "var(--text-muted)",
                            padding: "1px 3px",
                          }}
                          title="Remove layer"
                        >
                          ✕
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Inline Layer Detail Surface (P33-AC-02 Surface) */}
                  {isSelected && (
                    <div
                      style={{
                        marginTop: "4px",
                        padding: "6px",
                        backgroundColor: "rgba(0, 0, 0, 0.2)",
                        borderRadius: "3px",
                        display: "flex",
                        flexDirection: "column",
                        gap: "6px",
                        fontSize: "10px",
                      }}
                    >
                      {/* Layer Name */}
                      <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
                        <span>Name:</span>
                        <input
                          type="text"
                          value={draftLayer.name}
                          onChange={(e) => {
                            updateLayerDraft(layer.layer_id, { name: e.target.value });
                          }}
                          onBlur={() => {
                            commitLayerPatch(layer.layer_id, { name: draftLayer.name });
                          }}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              commitLayerPatch(layer.layer_id, { name: draftLayer.name });
                            }
                          }}
                          style={{
                            flex: 1,
                            maxWidth: "140px",
                            padding: "2px 4px",
                            fontSize: "10px",
                            backgroundColor: "var(--bg-surface)",
                            color: "var(--text-main)",
                            border: "1px solid var(--border-color)",
                            borderRadius: "2px",
                          }}
                        />
                      </label>

                      {/* Layer Opacity */}
                      <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
                        <span>Opacity:</span>
                        <div style={{ display: "flex", alignItems: "center", gap: "6px", flex: 1, maxWidth: "140px" }}>
                          <input
                            type="range"
                            min="0"
                            max="1"
                            step="0.05"
                            value={draftLayer.opacity}
                            onChange={(e) => {
                              updateLayerDraft(layer.layer_id, {
                                opacity: parseFloat(e.target.value) || 0,
                              });
                            }}
                            onPointerUp={() => {
                              commitLayerPatch(layer.layer_id, { opacity: draftLayer.opacity });
                            }}
                            onKeyUp={() => {
                              commitLayerPatch(layer.layer_id, { opacity: draftLayer.opacity });
                            }}
                            style={{ flex: 1 }}
                          />
                          <span style={{ width: "26px", textAlign: "right" }}>
                            {Math.round(draftLayer.opacity * 100)}%
                          </span>
                        </div>
                      </label>

                      {/* Transform: Position X, Y */}
                      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px" }}>
                        <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                          <span>Pos X:</span>
                          <input
                            type="number"
                            value={draftLayer.transform.position_x}
                            onChange={(e) => {
                              const val = parseFloat(e.target.value) || 0;
                              updateLayerDraft(layer.layer_id, {
                                transform: { position_x: val } as TemplateLayer["transform"],
                              });
                            }}
                            onBlur={() => {
                              commitLayerPatch(layer.layer_id, {
                                transform: { position_x: draftLayer.transform.position_x } as TemplateLayer["transform"],
                              });
                            }}
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                commitLayerPatch(layer.layer_id, {
                                  transform: { position_x: draftLayer.transform.position_x } as TemplateLayer["transform"],
                                });
                              }
                            }}
                            style={{
                              width: "50px",
                              padding: "2px 4px",
                              fontSize: "10px",
                              backgroundColor: "var(--bg-surface)",
                              color: "var(--text-main)",
                              border: "1px solid var(--border-color)",
                              borderRadius: "2px",
                            }}
                          />
                        </label>
                        <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                          <span>Pos Y:</span>
                          <input
                            type="number"
                            value={draftLayer.transform.position_y}
                            onChange={(e) => {
                              const val = parseFloat(e.target.value) || 0;
                              updateLayerDraft(layer.layer_id, {
                                transform: { position_y: val } as TemplateLayer["transform"],
                              });
                            }}
                            onBlur={() => {
                              commitLayerPatch(layer.layer_id, {
                                transform: { position_y: draftLayer.transform.position_y } as TemplateLayer["transform"],
                              });
                            }}
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                commitLayerPatch(layer.layer_id, {
                                  transform: { position_y: draftLayer.transform.position_y } as TemplateLayer["transform"],
                                });
                              }
                            }}
                            style={{
                              width: "50px",
                              padding: "2px 4px",
                              fontSize: "10px",
                              backgroundColor: "var(--bg-surface)",
                              color: "var(--text-main)",
                              border: "1px solid var(--border-color)",
                              borderRadius: "2px",
                            }}
                          />
                        </label>
                      </div>

                      {/* Kind-specific properties */}
                      {draftLayer.kind.type === "Shape" && (
                        <div style={{ display: "flex", flexDirection: "column", gap: "4px", borderTop: "1px solid rgba(255,255,255,0.06)", paddingTop: "4px" }}>
                          <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                            <span>Shape:</span>
                            <select
                              value={draftLayer.kind.data.shape_kind}
                              onChange={(e) => {
                                const sk = e.target.value as TemplateShapeKind;
                                if (draftLayer.kind.type === "Shape") {
                                  commitLayerPatch(layer.layer_id, {
                                    kind: {
                                      type: "Shape",
                                      data: { shape_kind: sk } as any,
                                    },
                                  });
                                }
                              }}
                              style={{
                                width: "110px",
                                padding: "2px 4px",
                                fontSize: "10px",
                                backgroundColor: "var(--bg-surface)",
                                color: "var(--text-main)",
                                border: "1px solid var(--border-color)",
                                borderRadius: "2px",
                              }}
                            >
                              <option value="Rectangle">Rectangle</option>
                              <option value="RoundedRectangle">Rounded Rect</option>
                              <option value="Circle">Circle</option>
                              <option value="Ellipse">Ellipse</option>
                              <option value="Line">Line</option>
                            </select>
                          </label>

                          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px" }}>
                            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                              <span>W:</span>
                              <input
                                type="number"
                                value={draftLayer.kind.data.width}
                                onChange={(e) => {
                                  const val = parseFloat(e.target.value) || 0;
                                  if (draftLayer.kind.type === "Shape") {
                                    updateLayerDraft(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { ...draftLayer.kind.data, width: val },
                                      },
                                    });
                                  }
                                }}
                                onBlur={() => {
                                  if (draftLayer.kind.type === "Shape") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { width: draftLayer.kind.data.width } as any,
                                      },
                                    });
                                  }
                                }}
                                style={{
                                  width: "50px",
                                  padding: "2px 4px",
                                  fontSize: "10px",
                                  backgroundColor: "var(--bg-surface)",
                                  color: "var(--text-main)",
                                  border: "1px solid var(--border-color)",
                                  borderRadius: "2px",
                                }}
                              />
                            </label>
                            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                              <span>H:</span>
                              <input
                                type="number"
                                value={draftLayer.kind.data.height}
                                onChange={(e) => {
                                  const val = parseFloat(e.target.value) || 0;
                                  if (draftLayer.kind.type === "Shape") {
                                    updateLayerDraft(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { ...draftLayer.kind.data, height: val },
                                      },
                                    });
                                  }
                                }}
                                onBlur={() => {
                                  if (draftLayer.kind.type === "Shape") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { height: draftLayer.kind.data.height } as any,
                                      },
                                    });
                                  }
                                }}
                                style={{
                                  width: "50px",
                                  padding: "2px 4px",
                                  fontSize: "10px",
                                  backgroundColor: "var(--bg-surface)",
                                  color: "var(--text-main)",
                                  border: "1px solid var(--border-color)",
                                  borderRadius: "2px",
                                }}
                              />
                            </label>
                          </div>

                          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px" }}>
                            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                              <span>Fill:</span>
                              <input
                                type="color"
                                value={(draftLayer.kind.data.fill_color ?? "#1E293B").slice(0, 7)}
                                onChange={(e) => {
                                  const val = e.target.value.toUpperCase();
                                  if (draftLayer.kind.type === "Shape") {
                                    updateLayerDraft(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { ...draftLayer.kind.data, fill_color: val },
                                      },
                                    });
                                  }
                                }}
                                onBlur={() => {
                                  if (draftLayer.kind.type === "Shape") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { fill_color: draftLayer.kind.data.fill_color } as any,
                                      },
                                    });
                                  }
                                }}
                                onPointerUp={() => {
                                  if (draftLayer.kind.type === "Shape") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { fill_color: draftLayer.kind.data.fill_color } as any,
                                      },
                                    });
                                  }
                                }}
                                style={{ width: "40px", height: "20px", border: "none", cursor: "pointer" }}
                              />
                            </label>
                            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                              <span>Stroke:</span>
                              <input
                                type="color"
                                value={(draftLayer.kind.data.stroke_color ?? "#3B82F6").slice(0, 7)}
                                onChange={(e) => {
                                  const val = e.target.value.toUpperCase();
                                  if (draftLayer.kind.type === "Shape") {
                                    updateLayerDraft(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { ...draftLayer.kind.data, stroke_color: val },
                                      },
                                    });
                                  }
                                }}
                                onBlur={() => {
                                  if (draftLayer.kind.type === "Shape") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { stroke_color: draftLayer.kind.data.stroke_color } as any,
                                      },
                                    });
                                  }
                                }}
                                onPointerUp={() => {
                                  if (draftLayer.kind.type === "Shape") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Shape",
                                        data: { stroke_color: draftLayer.kind.data.stroke_color } as any,
                                      },
                                    });
                                  }
                                }}
                                style={{ width: "40px", height: "20px", border: "none", cursor: "pointer" }}
                              />
                            </label>
                          </div>
                        </div>
                      )}

                      {draftLayer.kind.type === "Text" && !isPrimary && (
                        <div style={{ display: "flex", flexDirection: "column", gap: "4px", borderTop: "1px solid rgba(255,255,255,0.06)", paddingTop: "4px" }}>
                          <label style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
                            <span>Secondary Text Content:</span>
                            <input
                              type="text"
                              value={draftLayer.kind.data.text_content ?? ""}
                              onChange={(e) => {
                                if (draftLayer.kind.type === "Text") {
                                  updateLayerDraft(layer.layer_id, {
                                    kind: {
                                      type: "Text",
                                      data: { ...draftLayer.kind.data, text_content: e.target.value },
                                    },
                                  });
                                }
                              }}
                              onBlur={() => {
                                if (draftLayer.kind.type === "Text") {
                                  commitLayerPatch(layer.layer_id, {
                                    kind: {
                                      type: "Text",
                                      data: { text_content: draftLayer.kind.data.text_content } as any,
                                    },
                                  });
                                }
                              }}
                              onKeyDown={(e) => {
                                if (e.key === "Enter" && draftLayer.kind.type === "Text") {
                                  commitLayerPatch(layer.layer_id, {
                                    kind: {
                                      type: "Text",
                                      data: { text_content: draftLayer.kind.data.text_content } as any,
                                    },
                                  });
                                }
                              }}
                              style={{
                                padding: "3px 5px",
                                fontSize: "10px",
                                backgroundColor: "var(--bg-surface)",
                                color: "var(--text-main)",
                                border: "1px solid var(--border-color)",
                                borderRadius: "2px",
                              }}
                            />
                          </label>

                          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px" }}>
                            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                              <span>Size:</span>
                              <input
                                type="number"
                                value={draftLayer.kind.data.text_style?.font_size ?? 28}
                                onChange={(e) => {
                                  if (draftLayer.kind.type === "Text") {
                                    const prevStyle = draftLayer.kind.data.text_style ?? {
                                      font_size: 28,
                                      font_weight: 400,
                                      font_color: "#FFFFFF",
                                      alignment: "Center",
                                      anchor: "Center",
                                      font_family: "sans-serif",
                                      padding_x: 0,
                                      padding_y: 0,
                                      line_height: 1.2,
                                      background_color: null,
                                      background_opacity: 0,
                                    };
                                    updateLayerDraft(layer.layer_id, {
                                      kind: {
                                        type: "Text",
                                        data: {
                                          ...draftLayer.kind.data,
                                          text_style: { ...prevStyle, font_size: parseFloat(e.target.value) || 12 },
                                        },
                                      },
                                    });
                                  }
                                }}
                                onBlur={() => {
                                  if (draftLayer.kind.type === "Text") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Text",
                                        data: {
                                          text_style: {
                                            font_size: draftLayer.kind.data.text_style?.font_size,
                                          },
                                        } as any,
                                      },
                                    });
                                  }
                                }}
                                style={{
                                  width: "45px",
                                  padding: "2px 4px",
                                  fontSize: "10px",
                                  backgroundColor: "var(--bg-surface)",
                                  color: "var(--text-main)",
                                  border: "1px solid var(--border-color)",
                                  borderRadius: "2px",
                                }}
                              />
                            </label>
                            <label style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                              <span>Color:</span>
                              <input
                                type="color"
                                value={(draftLayer.kind.data.text_style?.font_color ?? "#FFFFFF").slice(0, 7)}
                                onChange={(e) => {
                                  if (draftLayer.kind.type === "Text") {
                                    const prevStyle = draftLayer.kind.data.text_style ?? {
                                      font_size: 28,
                                      font_weight: 400,
                                      font_color: "#FFFFFF",
                                      alignment: "Center",
                                      anchor: "Center",
                                      font_family: "sans-serif",
                                      padding_x: 0,
                                      padding_y: 0,
                                      line_height: 1.2,
                                      background_color: null,
                                      background_opacity: 0,
                                    };
                                    updateLayerDraft(layer.layer_id, {
                                      kind: {
                                        type: "Text",
                                        data: {
                                          ...draftLayer.kind.data,
                                          text_style: { ...prevStyle, font_color: e.target.value.toUpperCase() },
                                        },
                                      },
                                    });
                                  }
                                }}
                                onPointerUp={() => {
                                  if (draftLayer.kind.type === "Text") {
                                    commitLayerPatch(layer.layer_id, {
                                      kind: {
                                        type: "Text",
                                        data: {
                                          text_style: {
                                            font_color: draftLayer.kind.data.text_style?.font_color,
                                          },
                                        } as any,
                                      },
                                    });
                                  }
                                }}
                                style={{ width: "40px", height: "20px", border: "none", cursor: "pointer" }}
                              />
                            </label>
                          </div>
                        </div>
                      )}

                      {draftLayer.kind.type === "Image" && (
                        <div style={{ display: "flex", flexDirection: "column", gap: "4px", borderTop: "1px solid rgba(255,255,255,0.06)", paddingTop: "4px" }}>
                          <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                            Image Asset: {draftLayer.kind.data.asset_name || draftLayer.kind.data.asset_id || "Unlinked"}
                          </div>
                        </div>
                      )}

                      {/* Layer Motion / Animation info */}
                      <div style={{ borderTop: "1px solid rgba(255,255,255,0.06)", paddingTop: "4px", display: "flex", flexDirection: "column", gap: "2px" }}>
                        <div style={{ fontSize: "9px", color: "var(--text-muted)", fontWeight: 600 }}>
                          LAYER MOTION
                        </div>
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                          <span>Motion In:</span>
                          <span style={{ color: draftLayer.motion?.in_anim ? "var(--accent-primary)" : "var(--text-muted)" }}>
                            {draftLayer.motion?.in_anim ? "Configured" : "None"}
                          </span>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}

            {/* Add Layer quick buttons */}
            <div style={{ display: "flex", gap: "6px", marginTop: "4px" }}>
              <button
                type="button"
                className="btn-action"
                onClick={() => handleAddDefaultShapeLayer("RoundedRectangle")}
                style={{
                  flex: 1,
                  padding: "4px",
                  fontSize: "10px",
                  backgroundColor: "var(--bg-surface)",
                  color: "var(--text-main)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "3px",
                  cursor: "pointer",
                }}
              >
                + Shape Layer
              </button>
              <button
                type="button"
                className="btn-action"
                onClick={handleAddSecondaryTextLayer}
                style={{
                  flex: 1,
                  padding: "4px",
                  fontSize: "10px",
                  backgroundColor: "var(--bg-surface)",
                  color: "var(--text-main)",
                  border: "1px solid var(--border-color)",
                  borderRadius: "3px",
                  cursor: "pointer",
                }}
              >
                + Text Layer
              </button>
            </div>
          </div>
        )}
      </section>

      {/* Progressive Disclosure Section 3: Responsive Policy */}
      {policy && (
        <section style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
          <button
            type="button"
            onClick={() => setPolicyExpanded(!policyExpanded)}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              background: "none",
              border: "none",
              color: "var(--text-muted)",
              fontSize: "11px",
              fontWeight: 600,
              cursor: "pointer",
              padding: "2px 0",
              textAlign: "left",
            }}
          >
            <span>RESPONSIVE POLICY</span>
            <span>{policyExpanded ? "▼" : "▶"}</span>
          </button>

          {policyExpanded && (
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "4px",
                fontSize: "10px",
                color: "var(--text-muted)",
                backgroundColor: "var(--bg-surface)",
                padding: "6px 8px",
                borderRadius: "4px",
                border: "1px solid var(--border-color)",
              }}
            >
              <div>
                Reference: {policy.reference_width} × {policy.reference_height}
              </div>
              <div>Safe margin: {policy.safe_margin}px</div>
              <div>Aspect ratios: {policy.supported_aspect_ratios.join(", ")}</div>
              <div>Auto wrap: {policy.auto_wrap ? "Enabled" : "Disabled"}</div>
            </div>
          )}
        </section>
      )}

      {/* Explicit P33-AC-02 Scope Deferral Notice */}
      <div
        style={{
          fontSize: "10px",
          color: "var(--text-muted)",
          fontStyle: "italic",
          padding: "6px 8px",
          backgroundColor: "rgba(255, 255, 255, 0.02)",
          borderRadius: "4px",
          border: "1px dashed var(--border-color)",
          lineHeight: 1.4,
        }}
      >
        ℹ️ Phase 33 Scope Note: Layer composition, transforms, and basic shape/text controls are active. Full multi-stop gradient editor, image-layer asset relinking, and secondary multi-slot authoring are deferred under bounded v1 scope.
      </div>

      {/* Save As Template Button */}
      {onSaveAsTemplate && (
        <button
          type="button"
          className="btn-action"
          onClick={onSaveAsTemplate}
          style={{
            marginTop: "6px",
            padding: "5px 8px",
            fontSize: "11px",
            fontWeight: 600,
            backgroundColor: "var(--bg-surface)",
            color: "var(--accent-primary)",
            border: "1px solid var(--border-color)",
            borderRadius: "4px",
            cursor: "pointer",
          }}
        >
          💾 Save as New Template...
        </button>
      )}
    </div>
  );
};
