import React, { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { editorApi, ProjectCompareReport, ProjectVariantCreateReceipt } from "../services/editorApi";
import { Project } from "../types/project";

interface Props {
  project: Project;
  onClose: () => void;
  onOpenVariant: (path: string) => Promise<void>;
}

export const ProjectVariantDialog: React.FC<Props> = ({ project, onClose, onOpenVariant }) => {
  const [name, setName] = useState(`${project.name} Variant`);
  const [receipt, setReceipt] = useState<ProjectVariantCreateReceipt | null>(null);
  const [report, setReport] = useState<ProjectCompareReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const create = async () => {
    setBusy(true); setError(null);
    try {
      const path = await save({ title: "Create Project Variant", defaultPath: `${name.trim() || "variant"}.tinix`, filters: [{ name: "TinixCut Project", extensions: ["tinix", "tcproj"] }] });
      if (!path) return;
      const created = await editorApi.createProjectVariant(path, name, project.project_id, project.revision);
      setReceipt(created);
      setReport(await editorApi.compareCurrentWithPath(path, project.project_id, project.revision));
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };

  return <div className="dialog-backdrop" role="presentation" onMouseDown={() => !busy && onClose()}>
    <section className="modal-dialog edit-plan-dialog" role="dialog" aria-modal="true" aria-labelledby="project-variant-title" onMouseDown={event => event.stopPropagation()}>
      <div className="modal-dialog-header"><h2 id="project-variant-title">Project variant</h2><button className="btn-action" onClick={onClose} disabled={busy}>Close</button></div>
      <p>Create a separate editable copy from the current revision. The open project stays unchanged.</p>
      {!receipt && <label className="dialog-field"><span>Variant name</span><input value={name} onChange={event => setName(event.target.value)} disabled={busy} /></label>}
      {receipt && <div className="dialog-review-summary"><strong>{receipt.project.name}</strong><div>{receipt.path}</div><div>New project ID; revision 0. Source revision {receipt.sourceRevision} remains open.</div></div>}
      {report && <div className="dialog-review-summary" aria-label="Variant comparison"><strong>{report.contentEquivalent ? "Content matches source" : "Content differs"}</strong><div>Tracks +{report.tracks.added.length} / −{report.tracks.removed.length} / changed {report.tracks.modified.length}</div><div>Items +{report.items.added.length} / −{report.items.removed.length} / changed {report.items.modified.length}</div></div>}
      {error && <p role="alert">{error}</p>}
      <div className="modal-dialog-actions">{!receipt ? <button className="btn-primary" onClick={create} disabled={busy || !name.trim()}>{busy ? "Creating…" : "Create variant"}</button> : <button className="btn-primary" disabled={busy} onClick={async () => { setBusy(true); try { await onOpenVariant(receipt.path); onClose(); } catch (cause) { setError(String(cause)); } finally { setBusy(false); } }}>Open variant</button>}</div>
    </section>
  </div>;
};
