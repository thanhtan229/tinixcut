import React, { useMemo, useState } from "react";
import { Asset } from "../types/project";
import { filterAndSortMediaAssets, MediaLibraryKindFilter, MediaLibrarySort, MediaLibraryStatusFilter } from "./mediaLibrary";
import { AudioLevelReport } from "../services/editorApi";

interface MediaPanelProps {
  locatedOccurrence?: {assetId:string;index:number;total:number;trackName:string} | null;
  onLocateAsset?: (asset: Asset, direction: 1 | -1) => void;
  usageCounts?: Record<string, number>;
  assets: Asset[];
  onAddSampleAsset: () => void;
  onAddAssetToTimeline?: (asset: Asset) => void;
  onInsertAssetAtPlayhead?: (asset: Asset) => void;
  onOverwriteAssetAtPlayhead?: (asset: Asset) => void;
  offlineMedia?: Record<string, { last_known_path: string; permission_required: boolean }>;
  onRelinkAsset?: (asset: Asset) => void;
  onAnalyzeVisualScenes?: (asset: Asset) => void;
  onViewAsset?: (asset: Asset) => void;
  importProgress?: string | null;
  onCancelImport?: () => void;
  onRelinkMultiple?: () => void;
  onCollectMedia?: () => void;
  onSuggestSilenceCuts?: (asset: Asset) => void;
  onAnalyzeAudioLevels?: (asset: Asset) => Promise<AudioLevelReport>;
}

export const MediaPanel: React.FC<MediaPanelProps> = ({
  locatedOccurrence,
  onLocateAsset,
  usageCounts,
  assets,
  onAddSampleAsset,
  onAddAssetToTimeline,
  onInsertAssetAtPlayhead,
  onOverwriteAssetAtPlayhead,
  offlineMedia = {},
  onRelinkAsset,
  onAnalyzeVisualScenes,
  onViewAsset,
  importProgress = null,
  onCancelImport,
  onRelinkMultiple,
  onCollectMedia,
  onAnalyzeAudioLevels,
  onSuggestSilenceCuts,
}) => {
  const [query, setQuery] = useState("");
  const [usageFilter, setUsageFilter] = useState("all");
  const [kindFilter, setKindFilter] = useState<MediaLibraryKindFilter>("all");
  const [statusFilter, setStatusFilter] = useState<MediaLibraryStatusFilter>("all");
  const [sort, setSort] = useState<MediaLibrarySort>("name");
  const [audioLevels, setAudioLevels] = useState<Record<string, AudioLevelReport>>({});
  const [audioAnalysisBusy, setAudioAnalysisBusy] = useState<string | null>(null);
  const [audioAnalysisError, setAudioAnalysisError] = useState<Record<string, string>>({});
  const visibleAssets = useMemo(
    () => filterAndSortMediaAssets(assets, new Set(Object.keys(offlineMedia)), query, kindFilter, statusFilter, sort).filter(asset => !usageCounts || usageFilter === "all" || (usageFilter === "used" ? (usageCounts[asset.asset_id] ?? 0) > 0 : !usageCounts[asset.asset_id])),
    [assets, offlineMedia, query, kindFilter, statusFilter, sort, usageCounts, usageFilter],
  );

  return (
    <aside className="left-panel">
      <div className="panel-header" style={{ justifyContent: "space-between" }}>
        <span>Media Library</span>
        {importProgress ? (
          <button onClick={onCancelImport} className="btn-action" aria-label="Cancel media import">Cancel</button>
        ) : <button
          onClick={onAddSampleAsset}
          style={{
            background: "none",
            border: "none",
            color: "var(--accent-primary)",
            cursor: "pointer",
            fontSize: "11px",
            fontWeight: 600,
          }}
        >
          + Import
        </button>}
      </div>
      {assets.length > 0 && (
        <div className="media-library-controls" aria-label="Media library filters">
          <input
            aria-label="Search media"
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search media…"
          />
          <div className="media-library-filter-row">
            {usageCounts && <select aria-label="Filter media usage" value={usageFilter} onChange={event=>setUsageFilter(event.target.value)}>
              <option value="all">All usage</option><option value="used">Used</option><option value="unused">Unused</option>
            </select>}
            <select aria-label="Filter media type" value={kindFilter} onChange={(event) => setKindFilter(event.target.value as MediaLibraryKindFilter)}>
              <option value="all">All types</option>
              <option value="Video">Video</option>
              <option value="Audio">Audio</option>
              <option value="Image">Image</option>
            </select>
            <select aria-label="Filter media status" value={statusFilter} onChange={(event) => setStatusFilter(event.target.value as MediaLibraryStatusFilter)}>
              <option value="all">All status</option>
              <option value="online">Online</option>
              <option value="offline">Offline</option>
            </select>
            <select aria-label="Sort media" value={sort} onChange={(event) => setSort(event.target.value as MediaLibrarySort)}>
              <option value="name">Name</option>
              <option value="kind">Type</option>
              <option value="size">Size</option>
            </select>
          </div>
        </div>
      )}
      {importProgress && <div role="status" className="media-import-progress">{importProgress}</div>}
      {Object.keys(offlineMedia).length > 1 && onRelinkMultiple && (
        <div className="media-library-bulk-relink">
          <button type="button" className="btn-action" onClick={onRelinkMultiple}>Relink multiple…</button>
        </div>
      )}
      {assets.length > 0 && onCollectMedia && (
        <div className="media-library-bulk-relink">
          <button type="button" className="btn-action" onClick={onCollectMedia}>Collect project media…</button>
        </div>
      )}
      <div className="panel-content">
        {assets.length === 0 ? (
          <div style={{ textAlign: "center", marginTop: "24px" }}>
            <p>No media imported.</p>
            <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
              Import local footage or fixtures.
            </p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
            {visibleAssets.length === 0 ? (
              <p role="status" className="media-library-empty">No media matches these filters.</p>
            ) : visibleAssets.map((a) => {
              const unavailableMedia = offlineMedia[a.asset_id];
              return (
              <div
                key={a.asset_id}
                style={{
                  padding: "8px",
                  backgroundColor: "var(--bg-surface)",
                  borderRadius: "4px",
                  border: "1px solid var(--border-color)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "4px",
                }}
              >
                <div style={{ fontWeight: 600, fontSize: "12px", color: "var(--text-main)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {a.media_kind === "Audio" ? "♫" : a.media_kind === "Image" ? "▧" : "▶"} {a.name}
                </div>
                {usageCounts && <span style={{fontSize:11,color:"var(--text-muted)"}}>{usageCounts[a.asset_id] ? `Used in ${usageCounts[a.asset_id]} timeline clip${usageCounts[a.asset_id] === 1 ? "" : "s"}` : "Unused in timeline"}</span>}
                {onLocateAsset && !!usageCounts?.[a.asset_id] && <div style={{display:"flex",gap:4,flexWrap:"wrap"}}>
                  <button type="button" className="btn-action" title="Select the previous clip using this source, wrapping to the last" onClick={()=>onLocateAsset(a,-1)}>Previous use</button>
                  <button type="button" className="btn-action" title="Select the next clip using this source, wrapping to the first" onClick={()=>onLocateAsset(a,1)}>Locate on timeline</button>
                </div>}
                {locatedOccurrence?.assetId===a.asset_id && <span role="status" style={{fontSize:11,color:"var(--text-muted)",overflowWrap:"anywhere"}}>Clip {locatedOccurrence.index}/{locatedOccurrence.total} · {locatedOccurrence.trackName}</span>}
                {unavailableMedia && (
                  <div role="status" style={{ color: "var(--text-warning, #f59e0b)", fontSize: "10px", overflowWrap: "anywhere" }}>
                    {unavailableMedia.permission_required ? "Media access required" : "Media offline"} · Last path: {unavailableMedia.last_known_path}
                  </div>
                )}
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "4px" }}>
                  <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                    {a.media_kind} • {(a.file_size / (1024 * 1024)).toFixed(1)} MB
                    {a.dimensions ? ` • ${a.dimensions.width}×${a.dimensions.height}` : ""}
                    {a.duration ? ` • ${(a.duration.value * a.duration.rate.denominator / a.duration.rate.numerator).toFixed(1)} s` : ""}
                  </span>
                  <div style={{ display: "flex", gap: "4px" }}>
                  {onViewAsset && !unavailableMedia && (
                    <button className="btn-action" onClick={() => onViewAsset(a)}>
                      Preview
                    </button>
                  )}
                  {unavailableMedia && onRelinkAsset && (
                    <button className="btn-action" onClick={() => onRelinkAsset(a)}>
                      Relink…
                    </button>
                  )}
                  {!unavailableMedia && onAddAssetToTimeline && (
                    <button
                      className="btn-action"
                      onClick={() => onAddAssetToTimeline(a)}
                      title={`Thêm ${a.name} vào Timeline để biên tập`}
                      style={{
                        padding: "2px 8px",
                        fontSize: "10px",
                        fontWeight: 600,
                        backgroundColor: "var(--accent-primary)",
                        color: "#fff",
                        border: "none",
                        borderRadius: "3px",
                        cursor: "pointer",
                      }}
                    >
                      + Timeline
                    </button>
                  )}
                  {!unavailableMedia && onInsertAssetAtPlayhead && (
                    <button
                      className="btn-action"
                      onClick={() => onInsertAssetAtPlayhead(a)}
                      title={`Insert ${a.name} at the playhead and shift later clips`}
                    >
                      Insert
                    </button>
                  )}
                  {!unavailableMedia && onOverwriteAssetAtPlayhead && (
                    <button
                      className="btn-action"
                      onClick={() => onOverwriteAssetAtPlayhead(a)}
                      title={`Overwrite the timeline range at the playhead with ${a.name}`}
                    >
                      Overwrite
                    </button>
                  )}
                  {!unavailableMedia && a.media_kind !== "Image" && onSuggestSilenceCuts && <button className="btn-action" onClick={() => onSuggestSilenceCuts(a)}>Silence cuts…</button>}
                  {!unavailableMedia && a.media_kind === "Video" && onAnalyzeVisualScenes && (
                    <button
                      className="btn-action"
                      onClick={() => onAnalyzeVisualScenes(a)}
                      title="Tìm cảnh trong video bằng mô hình hình ảnh chạy cục bộ"
                    >
                      Tìm cảnh
                    </button>
                  )}
                  </div>
                </div>
                {!unavailableMedia && a.media_kind !== "Image" && onAnalyzeAudioLevels && (
                  <div style={{ display: "flex", flexDirection: "column", gap: "3px", fontSize: "10px" }}>
                    <button
                      type="button"
                      className="btn-action"
                      disabled={audioAnalysisBusy !== null}
                      onClick={async () => {
                        setAudioAnalysisBusy(a.asset_id);
                        setAudioAnalysisError((current) => ({ ...current, [a.asset_id]: "" }));
                        try {
                          const result = await onAnalyzeAudioLevels(a);
                          setAudioLevels((current) => ({ ...current, [a.asset_id]: result }));
                        } catch (error) {
                          setAudioAnalysisError((current) => ({ ...current, [a.asset_id]: String(error) }));
                        } finally {
                          setAudioAnalysisBusy(null);
                        }
                      }}
                    >
                      {audioAnalysisBusy === a.asset_id ? "Analyzing audio…" : "Analyze audio levels"}
                    </button>
                    {audioLevels[a.asset_id] && (
                      <span role="status" title={audioLevels[a.asset_id].measurement}>
                        Mean {audioLevels[a.asset_id].mean_dbfs?.toFixed(1) ?? "−∞"} dBFS · Peak {audioLevels[a.asset_id].peak_dbfs?.toFixed(1) ?? "−∞"} dBFS
                      </span>
                    )}
                    {audioAnalysisError[a.asset_id] && <span role="alert">{audioAnalysisError[a.asset_id]}</span>}
                  </div>
                )}
              </div>
              );
            })}
          </div>
        )}
      </div>
    </aside>
  );
};
