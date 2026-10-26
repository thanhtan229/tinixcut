import React, { useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { TimelineItem } from "../types/project";
import { editorApi, CapCutInspectionResult } from "../services/editorApi";

interface CapCutTemplateImportDialogProps {
  isOpen: boolean;
  selectedItem: TimelineItem | null;
  projectRevision: number;
  onClose: () => void;
  onApplied: (message: string) => Promise<void> | void;
  onSaved: () => Promise<void> | void;
}

export const CapCutTemplateImportDialog: React.FC<CapCutTemplateImportDialogProps> = ({
  isOpen,
  selectedItem,
  projectRevision,
  onClose,
  onApplied,
  onSaved,
}) => {
  const [sourcePath, setSourcePath] = useState("");
  const [resourceRoot, setResourceRoot] = useState("");
  const [inspection, setInspection] = useState<CapCutInspectionResult | null>(null);
  const [busy, setBusy] = useState<"inspect" | "save" | "apply" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const activeToken = useRef<string | null>(null);

  if (!isOpen) return null;

  const chooseSource = async () => {
    const chosen = await open({
      title: "Select CapCut draft or template archive",
      filters: [{ name: "CapCut draft", extensions: ["json", "zip"] }],
      multiple: false,
    });
    if (typeof chosen === "string") {
      setSourcePath(chosen);
      setInspection(null);
      setError(null);
      setNotice(null);
    }
  };

  const chooseFolder = async () => {
    const chosen = await open({ title: "Select CapCut draft folder", directory: true, multiple: false });
    if (typeof chosen === "string") {
      setSourcePath(chosen);
      setInspection(null);
      setError(null);
      setNotice(null);
    }
  };

  const chooseResourceRoot = async () => {
    const chosen = await open({ title: "Select a folder containing CapCut resources", directory: true, multiple: false });
    if (typeof chosen === "string") setResourceRoot(chosen);
  };

  const inspect = async () => {
    if (!sourcePath) return;
    setBusy("inspect");
    setError(null);
    setNotice(null);
    setInspection(null);
    let token: string | null = null;
    try {
      token = await editorApi.capcutCreateInspectionToken();
      activeToken.current = token;
      const result = await editorApi.capcutInspectTemplateDraft(
        sourcePath,
        resourceRoot ? [resourceRoot] : [],
        token
      );
      setInspection(result);
      setNotice(`Inspection complete: ${result.report.status.replace(/_/g, " ")}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      activeToken.current = null;
      setBusy(null);
    }
  };

  const findManifestResourceSource = (
    result: CapCutInspectionResult,
    resource: NonNullable<CapCutInspectionResult["converted_definition"]>["resources"]["resources"][number]
  ) => {
    const expectedName = resource.file_name.toLowerCase();
    return result.profile.resources.find((diagnostic) => {
      if (!diagnostic.resolved_path || !diagnostic.sha256) return false;
      const basename = diagnostic.resolved_path.replace(/\\/g, "/").split("/").pop()?.toLowerCase();
      if (basename !== expectedName) return false;
      return diagnostic.resource_id === resource.resource_id ||
        !diagnostic.resource_id || diagnostic.panel.toLowerCase().includes("font");
    });
  };

  const resourceInputs = (result: CapCutInspectionResult): [string, string][] =>
    (result.converted_definition?.resources.resources ?? [])
      .map((resource) => {
        const source = findManifestResourceSource(result, resource);
        return source?.resolved_path ? [resource.file_name, source.resolved_path] as [string, string] : null;
      })
      .filter((pair): pair is [string, string] => pair !== null);

  const saveToLibrary = async () => {
    if (!inspection?.report.converted_definition) return;
    setBusy("save");
    setError(null);
    setNotice(null);
    try {
      const definition = inspection.report.converted_definition;
      const assets = resourceInputs(inspection);
      const unresolvedRequired = definition.resources.resources.filter(
        (resource) => resource.required && !assets.some(([name]) => name === resource.file_name)
      );
      if (unresolvedRequired.length) {
        throw new Error(`Required resource is unresolved: ${unresolvedRequired.map((resource) => resource.file_name).join(", ")}`);
      }
      await editorApi.capcutSaveTemplateToLibrary(inspection.report, assets, inspection.report.source_sha256);
      await onSaved();
      setNotice(`Saved “${definition.name}” to the template library.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const applyToSelectedText = async () => {
    if (!inspection?.report.converted_definition || selectedItem?.kind !== "Text") return;
    setBusy("apply");
    setError(null);
    setNotice(null);
    try {
      const definition = inspection.report.converted_definition;
      const assets = resourceInputs(inspection);
      const unresolvedRequired = definition.resources.resources.filter(
        (resource) => resource.required && !assets.some(([name]) => name === resource.file_name)
      );
      if (unresolvedRequired.length) {
        throw new Error(`Required resource is unresolved: ${unresolvedRequired.map((resource) => resource.file_name).join(", ")}`);
      }
      await editorApi.capcutSaveTemplateToLibrary(inspection.report, assets, inspection.report.source_sha256);
      await onSaved();
      // ZIP inspection resources live in temporary staging that save-to-library cleans.
      // The archive hash covers those extracted bytes; folder imports can fingerprint live files.
      const fingerprints: Record<string, string> = {};
      if (!sourcePath.toLowerCase().endsWith(".zip")) {
        for (const resource of definition.resources.resources) {
          const source = findManifestResourceSource(inspection, resource);
          if (source?.resolved_path && source.sha256) fingerprints[source.resolved_path] = source.sha256;
        }
      }
      const receipt = await editorApi.capcutApplyTemplateToProject({
        itemId: selectedItem.item_id,
        templateId: definition.id,
        templateVersion: definition.version,
        textVariant: selectedItem.text_content || undefined,
        expectedRevision: projectRevision,
        expectedSourceHash: inspection.report.source_sha256,
        sourceDraftPath: inspection.report.source_path,
        expectedResourceFingerprints: fingerprints,
        stagingToken: inspection.report.staging_token ?? "",
      });
      await onApplied(`Applied imported template “${definition.name}” (revision ${receipt.new_revision}).`);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const cancelInspection = async () => {
    if (busy !== "inspect" || !activeToken.current) return;
    try {
      const accepted = await editorApi.capcutCancelInspection(activeToken.current);
      setNotice(accepted
        ? "Cancellation requested. The importer will stop at its next bounded read checkpoint."
        : "The inspection has already finished.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const canSave = !!inspection?.report.converted_definition &&
    ["FULL_CONVERSION", "PARTIAL_CONVERSION"].includes(inspection.report.status);
  const isTextSelected = selectedItem?.kind === "Text";

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
      <section className="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="capcut-import-title" style={{ width: "min(720px, 92vw)", maxHeight: "88vh", overflow: "auto" }}>
        <div className="modal-dialog-header">
          <h2 id="capcut-import-title">Import CapCut template</h2>
          <button type="button" aria-label="Close dialog" disabled={!!busy} onClick={onClose}>×</button>
        </div>
        <div style={{ display: "grid", gap: 12, padding: 16, color: "var(--text-main)", fontSize: 12 }}>
          <p style={{ margin: 0, color: "var(--text-muted)" }}>
            Tinix reads draft JSON or a ZIP archive and reports what can be converted. It does not run CapCut or import the reference video as an editable template.
          </p>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" className="btn-action" onClick={chooseSource} disabled={!!busy}>Choose draft / ZIP…</button>
            <button type="button" className="btn-action" onClick={chooseFolder} disabled={!!busy}>Choose draft folder…</button>
          </div>
          <div style={{ overflowWrap: "anywhere", color: sourcePath ? "var(--text-main)" : "var(--text-muted)" }}>{sourcePath || "No source selected"}</div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <button type="button" className="btn-action" onClick={chooseResourceRoot} disabled={!!busy}>Add resource folder…</button>
            <span style={{ overflowWrap: "anywhere", color: "var(--text-muted)" }}>{resourceRoot || "Optional: locate missing resources"}</span>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" className="btn-primary" onClick={inspect} disabled={!sourcePath || !!busy}>{busy === "inspect" ? "Inspecting…" : "Inspect source"}</button>
            {busy === "inspect" && <button type="button" className="btn-action" onClick={cancelInspection}>Cancel inspection</button>}
          </div>

          {notice && <div role="status" style={{ color: "var(--accent-primary)" }}>{notice}</div>}
          {error && <div role="alert" style={{ color: "#ef4444", whiteSpace: "pre-wrap" }}>{error}</div>}
          {inspection && <div style={{ display: "grid", gap: 8, borderTop: "1px solid var(--border-color)", paddingTop: 12 }}>
            <strong>{inspection.report.template_name} · {inspection.report.status.replace(/_/g, " ")}</strong>
            <div>Profile: {inspection.report.recognized_profile} · {inspection.profile.layers_count} layers · {inspection.profile.canvas_width}×{inspection.profile.canvas_height}</div>
            <div>Source SHA-256: <code style={{ overflowWrap: "anywhere" }}>{inspection.report.source_sha256}</code></div>
            {inspection.report.imported_elements.length > 0 && <div><strong>Imported:</strong> {inspection.report.imported_elements.join(", ")}</div>}
            {inspection.report.loss_details.length > 0 && <div><strong>Conversion details</strong><ul>{inspection.report.loss_details.map((loss, index) => <li key={`${loss.component}-${index}`}>{loss.component}: {loss.treatment} ({loss.impact})</li>)}</ul></div>}
            {inspection.profile.missing_resources.length > 0 && <div style={{ color: "#f59e0b" }}><strong>Missing resources:</strong> {inspection.profile.missing_resources.join(", ")}</div>}
            {inspection.report.warnings.length > 0 && <ul>{inspection.report.warnings.map((warning, index) => <li key={index}>{warning}</li>)}</ul>}
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <button type="button" className="btn-action" onClick={saveToLibrary} disabled={!canSave || !!busy}>{busy === "save" ? "Saving…" : "Save to template library"}</button>
              <button type="button" className="btn-primary" onClick={applyToSelectedText} disabled={!canSave || !isTextSelected || !!busy} title={isTextSelected ? "Apply through the project command service" : "Select a text clip first"}>{busy === "apply" ? "Applying…" : "Apply to selected text clip"}</button>
            </div>
            {!canSave && <div style={{ color: "var(--text-muted)" }}>This source did not produce a supported editable template.</div>}
            {!isTextSelected && <div style={{ color: "var(--text-muted)" }}>Select a text clip on the timeline to apply this conversion.</div>}
          </div>}
        </div>
      </section>
    </div>
  );
};
