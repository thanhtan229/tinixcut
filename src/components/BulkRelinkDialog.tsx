import { Asset } from "../types/project";

interface BulkRelinkDialogProps {
  assets: Asset[];
  offlineAssetIds: string[];
  mappings: Record<string, string>;
  onChoose: (asset: Asset) => void;
  onApply: () => void;
  onClose: () => void;
  applying: boolean;
}

export function BulkRelinkDialog({ assets, offlineAssetIds, mappings, onChoose, onApply, onClose, applying }: BulkRelinkDialogProps) {
  const offlineAssets = assets.filter((asset) => offlineAssetIds.includes(asset.asset_id));
  const mappedCount = offlineAssets.filter((asset) => Boolean(mappings[asset.asset_id])).length;

  return (
    <div className="dialog-backdrop">
      <section className="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="bulk-relink-title">
        <header className="modal-dialog-header">
          <div>
            <h2 id="bulk-relink-title">Relink offline media</h2>
            <p>Choose a replacement file for each asset. No project change is applied until all mappings are reviewed below.</p>
          </div>
        </header>
        <div className="dialog-review-summary">
          {offlineAssets.map((asset) => (
            <div className="bulk-relink-row" key={asset.asset_id}>
              <div>
                <strong>{asset.name}</strong>
                <span>{mappings[asset.asset_id] || "No replacement selected"}</span>
              </div>
              <button type="button" className="btn-action" disabled={applying} onClick={() => onChoose(asset)}>
                {mappings[asset.asset_id] ? "Change file…" : "Choose file…"}
              </button>
            </div>
          ))}
        </div>
        <footer className="modal-dialog-actions">
          <button type="button" className="btn-primary" disabled={applying || offlineAssets.length === 0 || mappedCount !== offlineAssets.length} onClick={onApply}>
            {applying ? "Applying…" : `Apply ${mappedCount} mapping(s)`}
          </button>
          <button type="button" className="btn-action" disabled={applying} onClick={onClose}>Cancel</button>
        </footer>
      </section>
    </div>
  );
}
