import React, { useState, useEffect, useRef } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { Asset } from "../types/project";
import { TemplateDefinition, TemplatePackManifest } from "../types/template";
import {
  finalizeDefinitionResources,
  isValidTemplateId,
  resolvePackAssets,
  slugifyTemplateId,
} from "../utils/templateHelpers";
import { editorApi } from "../services/editorApi";

interface TemplatePackExportDialogProps {
  isOpen: boolean;
  definitions: TemplateDefinition[];
  assets?: Asset[];
  onClose: () => void;
  onExportSuccess: (packPath: string) => void;
}

export const TemplatePackExportDialog: React.FC<TemplatePackExportDialogProps> = ({
  isOpen,
  definitions,
  assets = [],
  onClose,
  onExportSuccess,
}) => {
  const [packName, setPackName] = useState("Custom Templates Pack");
  const [packId, setPackId] = useState("custom-templates-pack");
  const [idTouched, setIdTouched] = useState(false);
  const [packVersion, setPackVersion] = useState(1);
  const [author, setAuthor] = useState("");
  const [description, setDescription] = useState("");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [isExporting, setIsExporting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);
  const previousActiveElementRef = useRef<HTMLElement | null>(null);

  // Focus preservation and initial focus
  useEffect(() => {
    if (isOpen) {
      previousActiveElementRef.current = document.activeElement as HTMLElement | null;
      setPackName("Custom Templates Pack");
      setPackId("custom-templates-pack");
      setIdTouched(false);
      setPackVersion(1);
      setAuthor("");
      setDescription("Exported template pack from TinixCut");
      setSelectedIds(new Set(definitions.map((d) => d.id)));
      setErrorMsg(null);

      setTimeout(() => {
        const input = dialogRef.current?.querySelector<HTMLElement>("input[autofocus], input");
        input?.focus();
      }, 10);
    } else if (previousActiveElementRef.current) {
      previousActiveElementRef.current.focus();
    }
  }, [isOpen, definitions]);

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

  if (!isOpen) return null;

  const toggleSelectId = (id: string) => {
    const updated = new Set(selectedIds);
    if (updated.has(id)) {
      updated.delete(id);
    } else {
      updated.add(id);
    }
    setSelectedIds(updated);
  };

  const handleExport = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMsg(null);

    const trimmedName = packName.trim();
    if (!trimmedName) {
      setErrorMsg("Pack name cannot be empty");
      return;
    }

    const trimmedId = packId.trim();
    if (!isValidTemplateId(trimmedId)) {
      setErrorMsg("Invalid pack ID. Must be 1-64 characters using alphanumeric, dash (-), or underscore (_).");
      return;
    }

    if (selectedIds.size === 0) {
      setErrorMsg("Please select at least one template to include in the pack");
      return;
    }

    const defsToExport = definitions.filter((d) => selectedIds.has(d.id));

    // Resolve resource assets and validate missing ones
    const { assetFiles, missingResources } = resolvePackAssets(defsToExport, assets);
    if (missingResources.length > 0) {
      setErrorMsg(
        `Cannot export pack: the following required resources could not be resolved from project assets:\n${missingResources.join(
          "\n"
        )}`
      );
      return;
    }

    try {
      const outputPath = await save({
        title: "Export Tinix Template Pack",
        defaultPath: `${trimmedId}.tinixtpl`,
        filters: [{ name: "Tinix Template Pack", extensions: ["tinixtpl"] }],
      });

      if (!outputPath) return; // User cancelled dialog

      setIsExporting(true);

      const finalizedDefs: TemplateDefinition[] = [];
      for (const def of defsToExport) {
        const finalized = await finalizeDefinitionResources(def, assets);
        finalizedDefs.push(finalized);
      }

      const manifest: TemplatePackManifest = {
        pack_id: trimmedId,
        pack_version: packVersion,
        name: trimmedName,
        author: author.trim() || undefined,
        description: description.trim() || undefined,
        template_ids: finalizedDefs.map((d) => d.id),
        created_at: new Date().toISOString(),
      };

      await editorApi.templateExportPack(manifest, finalizedDefs, assetFiles, outputPath);

      setIsExporting(false);
      onExportSuccess(outputPath);
      onClose();
    } catch (err) {
      setIsExporting(false);
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div
      className="dialog-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="template-pack-export-title"
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
            <h2 id="template-pack-export-title" style={{ margin: 0, fontSize: "16px" }}>
              Export Template Pack (.tinixtpl)
            </h2>
            <p style={{ margin: "4px 0 0 0", fontSize: "12px", color: "var(--text-muted)" }}>
              Bundle templates into a shareable, secure native pack.
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

        <form onSubmit={handleExport} style={{ display: "flex", flexDirection: "column", gap: "12px", marginTop: "16px" }}>
          {errorMsg && (
            <div
              style={{
                padding: "8px 12px",
                borderRadius: "4px",
                backgroundColor: "rgba(239, 68, 68, 0.15)",
                border: "1px solid rgba(239, 68, 68, 0.4)",
                color: "#ef4444",
                fontSize: "12px",
                whiteSpace: "pre-line",
              }}
            >
              {errorMsg}
            </div>
          )}

          <label className="dialog-field">
            <span>Pack Name *</span>
            <input
              type="text"
              required
              value={packName}
              onChange={(e) => {
                setPackName(e.target.value);
                if (!idTouched) {
                  setPackId(slugifyTemplateId(e.target.value));
                }
              }}
              placeholder="e.g. My Pack"
              autoFocus
            />
          </label>

          <div style={{ display: "grid", gridTemplateColumns: "2fr 1fr", gap: "8px" }}>
            <label className="dialog-field">
              <span>Pack ID *</span>
              <input
                type="text"
                required
                value={packId}
                onChange={(e) => {
                  setPackId(e.target.value);
                  setIdTouched(true);
                }}
                placeholder="slug-pack-id"
              />
            </label>

            <label className="dialog-field">
              <span>Version</span>
              <input
                type="number"
                min="1"
                step="1"
                value={packVersion}
                onChange={(e) => setPackVersion(Math.max(1, parseInt(e.target.value, 10) || 1))}
              />
            </label>
          </div>

          <label className="dialog-field">
            <span>Author</span>
            <input
              type="text"
              value={author}
              onChange={(e) => setAuthor(e.target.value)}
              placeholder="Your name or team"
            />
          </label>

          <label className="dialog-field">
            <span>Description</span>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Description of the template pack"
            />
          </label>

          {/* Templates selection checklist */}
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>
                SELECT TEMPLATES TO INCLUDE ({selectedIds.size}/{definitions.length}):
              </span>
              <div style={{ display: "flex", gap: "8px" }}>
                <button
                  type="button"
                  onClick={() => setSelectedIds(new Set(definitions.map((d) => d.id)))}
                  style={{
                    background: "none",
                    border: "none",
                    color: "var(--accent-primary)",
                    fontSize: "11px",
                    cursor: "pointer",
                    padding: 0,
                  }}
                >
                  Select All
                </button>
                <button
                  type="button"
                  onClick={() => setSelectedIds(new Set())}
                  style={{
                    background: "none",
                    border: "none",
                    color: "var(--text-muted)",
                    fontSize: "11px",
                    cursor: "pointer",
                    padding: 0,
                  }}
                >
                  Clear
                </button>
              </div>
            </div>

            <div
              style={{
                maxHeight: "150px",
                overflowY: "auto",
                border: "1px solid var(--border-color)",
                borderRadius: "4px",
                backgroundColor: "var(--bg-surface)",
                padding: "6px",
                display: "flex",
                flexDirection: "column",
                gap: "4px",
              }}
            >
              {definitions.map((def) => (
                <label
                  key={def.id}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "8px",
                    fontSize: "12px",
                    padding: "3px 4px",
                    borderRadius: "3px",
                    cursor: "pointer",
                    color: "var(--text-main)",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selectedIds.has(def.id)}
                    onChange={() => toggleSelectId(def.id)}
                  />
                  <span>{def.name}</span>
                  <span style={{ fontSize: "10px", color: "var(--text-muted)", marginLeft: "auto" }}>
                    {def.category} v{def.version}
                  </span>
                </label>
              ))}
            </div>
          </div>

          <div className="modal-dialog-actions" style={{ marginTop: "12px" }}>
            <button
              type="button"
              className="btn-action"
              onClick={onClose}
              disabled={isExporting}
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
              disabled={isExporting || selectedIds.size === 0}
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
              {isExporting ? "Exporting..." : "Choose Destination & Export"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
