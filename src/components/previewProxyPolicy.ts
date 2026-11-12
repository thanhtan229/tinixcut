// Preview proxy selection. Proxies are disposable cache: the project and export
// always reference the original media, so these helpers only choose what the
// preview <video> element loads.

export type PreviewProxyMode = "off" | "auto";

export type PreviewProxySource = {
  proxyPath: string;
  // Original asset path the proxy was derived from. A relink changes the asset
  // path, which must invalidate the cached proxy choice.
  sourcePath: string | null;
};

type ProxyCandidateAsset = { asset_id: string; media_kind: string };

/** Video assets the preview should request proxies for under the given mode. */
export function policyProxyAssetIds(
  assets: readonly ProxyCandidateAsset[] | undefined,
  mode: PreviewProxyMode
): string[] {
  if (mode !== "auto" || !assets) return [];
  return assets.filter((asset) => asset.media_kind === "Video").map((asset) => asset.asset_id);
}

/** The path the preview should load: a ready proxy for this exact source, else the original. */
export function resolvePreviewVideoPath(
  assetId: string | null | undefined,
  sourcePath: string | null | undefined,
  proxies: Readonly<Record<string, PreviewProxySource>>
): string | null | undefined {
  const proxy = assetId ? proxies[assetId] : undefined;
  return proxy && proxy.sourcePath === (sourcePath ?? null) ? proxy.proxyPath : sourcePath;
}

/**
 * Assets that still need a proxy status poll: requested by the codec fallback or the
 * policy, not yet ready, and (for policy requests) not already reported unavailable.
 * A policy proxy that cannot be built must not keep polling or block the original.
 */
export function proxyAssetIdsToPoll(
  fallbackAssetIds: readonly string[],
  policyAssetIds: readonly string[],
  readyAssetIds: ReadonlySet<string>,
  policyUnavailableAssetIds: ReadonlySet<string>
): string[] {
  const fallback = new Set(fallbackAssetIds);
  const ids = new Set([...fallbackAssetIds, ...policyAssetIds]);
  return [...ids].filter(
    (assetId) =>
      !readyAssetIds.has(assetId) && (fallback.has(assetId) || !policyUnavailableAssetIds.has(assetId))
  );
}
