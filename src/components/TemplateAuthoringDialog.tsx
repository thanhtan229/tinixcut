import React, { useState, useEffect, useRef } from "react";
import { Asset, TimelineItem } from "../types/project";
import { TemplateDefinition } from "../types/template";
import {
  buildTemplateDefinitionFromItem,
  finalizeDefinitionResources,
  isValidTemplateId,
  slugifyTemplateId,
} from "../utils/templateHelpers";
import { editorApi } from "../services/editorApi";

interface TemplateAuthoringDialogProps {
  isOpen: boolean;
  selectedItem: TimelineItem | null;
  assets?: Asset[];
  onClose: () => void;
  onSaved: (definition: TemplateDefinition) => void;
}

export const TemplateAuthoringDialog: React.FC<TemplateAuthoringDialogProps> = ({
  isOpen,
  selectedItem,
  assets = [],
  onClose,
  onSaved,
}) => {
  const [name, setName] = useState("");
  const [templateId, setTemplateId] = useState("");
  const [idTouched, setIdTouched] = useState(false);
  const [version, setVersion] = useState(1);
  const [category, setCategory] = useState("Headline");
  const [customCategory, setCustomCategory] = useState("");
  const [description, setDescription] = useState("");
  const [tagsStr, setTagsStr] = useState("");
  const [exposeParameters, setExposeParameters] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousActiveElementRef = useRef<HTMLElement | null>(null);

  // Focus preservation and initial focus
  useEffect(() => {
    if (isOpen) {
      previousActiveElementRef.current = document.activeElement as HTMLElement | null;
      setTimeout(() => {
        const input = dialogRef.current?.querySelector<HTMLElement>("input[autofocus], input");
        input?.focus();
      }, 10);
    } else if (previousActiveElementRef.current) {
      previousActiveElementRef.current.focus();
    }
  }, [isOpen]);

  // Handle escape key and focus trap cycling
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key === "Tab" && dialogRef.current) {
        const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];

        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen || !selectedItem) return null;

  const handleNameChange = (val: string) => {
    setName(val);
    if (!idTouched) {
      setTemplateId(slugifyTemplateId(val));
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMsg(null);

    const trimmedName = name.trim();
    if (!trimmedName) {
      setErrorMsg("Template name cannot be empty");
      return;
    }

    const trimmedId = templateId.trim();
    if (!isValidTemplateId(trimmedId)) {
      setErrorMsg(
        "Invalid template ID. Must be 1-64 characters using alphanumeric, dash (-), or underscore (_)."
      );
      return;
    }

    const finalCategory =
      category === "Other" ? customCategory.trim() || "Custom" : category;

    const tags = tagsStr
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);

    try {
      setIsSaving(true);
      const definition = buildTemplateDefinitionFromItem(selectedItem, {
        id: trimmedId,
        name: trimmedName,
        category: finalCategory,
        description,
        version,
        tags,
        exposeParameters,
        projectAssets: assets,
      });

      // Compute and verify resource SHA-256 hashes using real project files
      const finalizedDef = await finalizeDefinitionResources(definition, assets);

      await editorApi.templateSaveUserDefinition(finalizedDef);
      setIsSaving(false);
      onSaved(finalizedDef);
      onClose();
    } catch (err) {
      setIsSaving(false);
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const existingLayers = selectedItem.template_instance?.layers;

  return (
    <div
      className="dialog-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="template-authoring-title"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="modal-dialog"
        style={{
          maxWidth: "520px",
          width: "100%",
          maxHeight: "90vh",
          overflowY: "auto",
        }}
      >
        <div className="modal-dialog-header">
          <div>
            <h2 id="template-authoring-title" style={{ margin: 0, fontSize: "16px" }}>
              Save as Template
            </h2>
            <p style={{ margin: "4px 0 0 0", fontSize: "12px", color: "var(--text-muted)" }}>
              Publish selected text item as a reusable native template definition.
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="btn-action"
            style={{
              background: "none",
              border: "none",
              color: "var(--text-muted)",
              fontSize: "16px",
              cursor: "pointer",
            }}
            aria-label="Close dialog"
          >
            ✕
          </button>
        </div>

        <form onSubmit={handleSave} style={{ display: "flex", flexDirection: "column", gap: "12px", marginTop: "16px" }}>
          {errorMsg && (
            <div
              style={{
                padding: "8px 12px",
                borderRadius: "4px",
                backgroundColor: "rgba(239, 68, 68, 0.15)",
                border: "1px solid rgba(239, 68, 68, 0.4)",
                color: "#ef4444",
                fontSize: "12px",
              }}
            >
              {errorMsg}
            </div>
          )}

          <label className="dialog-field">
            <span>Template Name *</span>
            <input
              type="text"
              required
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="e.g. Minimal Title"
              autoFocus
            />
          </label>

          <div style={{ display: "grid", gridTemplateColumns: "2fr 1fr", gap: "8px" }}>
            <label className="dialog-field">
              <span>Template ID *</span>
              <input
                type="text"
                required
                value={templateId}
                onChange={(e) => {
                  setTemplateId(e.target.value);
                  setIdTouched(true);
                }}
                placeholder="slug-id"
              />
            </label>

            <label className="dialog-field">
              <span>Version</span>
              <input
                type="number"
                min="1"
                step="1"
                value={version}
                onChange={(e) => setVersion(Math.max(1, parseInt(e.target.value, 10) || 1))}
              />
            </label>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: category === "Other" ? "1fr 1fr" : "1fr", gap: "8px" }}>
            <label className="dialog-field">
              <span>Category</span>
              <select value={category} onChange={(e) => setCategory(e.target.value)}>
                <option value="Headline">Headline</option>
                <option value="Lower Third">Lower Third</option>
                <option value="Social">Social</option>
                <option value="Editorial">Editorial</option>
                <option value="Badge">Badge</option>
                <option value="Card">Card</option>
                <option value="Other">Other (Custom)...</option>
              </select>
            </label>

            {category === "Other" && (
              <label className="dialog-field">
                <span>Custom Category</span>
                <input
                  type="text"
                  value={customCategory}
                  onChange={(e) => setCustomCategory(e.target.value)}
                  placeholder="e.g. Outro"
                />
              </label>
            )}
          </div>

          <label className="dialog-field">
            <span>Description</span>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Brief description for the catalog"
            />
          </label>

          <label className="dialog-field">
            <span>Tags (comma-separated)</span>
            <input
              type="text"
              value={tagsStr}
              onChange={(e) => setTagsStr(e.target.value)}
              placeholder="headline, modern, bold"
            />
          </label>

          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              fontSize: "12px",
              color: "var(--text-main)",
              cursor: "pointer",
            }}
          >
            <input
              type="checkbox"
              checked={exposeParameters}
              onChange={(e) => setExposeParameters(e.target.checked)}
            />
            Expose text content and font colors as editable parameters
          </label>

          {/* Layer review snippet */}
          <div
            style={{
              padding: "8px",
              backgroundColor: "var(--bg-surface)",
              borderRadius: "4px",
              border: "1px solid var(--border-color)",
              fontSize: "11px",
            }}
          >
            <span style={{ fontWeight: 600, color: "var(--text-muted)" }}>
              Layers to capture ({existingLayers ? existingLayers.length : 1}):
            </span>
            <ul style={{ margin: "4px 0 0 16px", padding: 0, color: "var(--text-main)" }}>
              {existingLayers && existingLayers.length > 0 ? (
                existingLayers.map((l) => (
                  <li key={l.layer_id}>
                    {l.name} ({l.kind.type})
                  </li>
                ))
              ) : (
                <li>Primary Text: "{selectedItem.text_content ?? "Template Text"}"</li>
              )}
            </ul>
          </div>

          <div className="modal-dialog-actions" style={{ marginTop: "12px" }}>
            <button
              type="button"
              className="btn-action"
              onClick={onClose}
              disabled={isSaving}
              style={{
                padding: "6px 12px",
                border: "1px solid var(--border-color)",
                borderRadius: "4px",
                background: "none",
                color: "var(--text-main)",
                cursor: "pointer",
              }}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="btn-action"
              disabled={isSaving}
              style={{
                padding: "6px 14px",
                backgroundColor: "var(--accent-primary)",
                color: "#ffffff",
                border: "none",
                borderRadius: "4px",
                fontWeight: 600,
                cursor: "pointer",
              }}
            >
              {isSaving ? "Saving..." : "Save to Catalog"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
