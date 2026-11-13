import { useState } from "react";
import { editorApi, type ChapterReport } from "../services/editorApi";

/** Builds the YouTube chapter list from the project's markers and copies it on request. */
export function ChaptersSection() {
  const [report, setReport] = useState<ChapterReport | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const copy = async () => {
    try {
      const next = await editorApi.projectChapters();
      setReport(next);
      if (next.violations.length > 0) {
        setMessage("Not copied: the markers do not form valid YouTube chapters.");
        return;
      }
      await navigator.clipboard.writeText(next.text);
      setMessage(`Copied ${next.chapters.length} chapters.`);
    } catch (err) {
      setMessage(`Chapters unavailable: ${err}`);
    }
  };

  return (
    <section style={{ display: "flex", flexDirection: "column", gap: "6px", marginTop: "12px" }}>
      <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-muted)" }}>YOUTUBE CHAPTERS</div>
      <button type="button" onClick={copy} style={{ fontSize: "11px", alignSelf: "flex-start" }}>
        Copy YouTube chapters
      </button>
      {message && (
        <div role="status" style={{ fontSize: "11px" }}>
          {message}
        </div>
      )}
      {report && report.violations.length > 0 && (
        <ul style={{ margin: 0, paddingLeft: "16px", fontSize: "11px" }}>
          {report.violations.map((violation) => (
            <li key={violation}>{violation}</li>
          ))}
        </ul>
      )}
      {report && report.violations.length === 0 && (
        <pre style={{ margin: 0, fontSize: "11px", whiteSpace: "pre-wrap" }}>{report.text}</pre>
      )}
    </section>
  );
}
