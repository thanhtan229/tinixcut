import { describe, expect, it } from "vitest";
import { policyProxyAssetIds, proxyAssetIdsToPoll, resolvePreviewVideoPath } from "./previewProxyPolicy";

const assets = [
  { asset_id: "video-a", media_kind: "Video" },
  { asset_id: "audio-a", media_kind: "Audio" },
  { asset_id: "image-a", media_kind: "Image" },
  { asset_id: "video-b", media_kind: "Video" },
];

describe("preview proxy policy", () => {
  it("requests proxies only for video assets and only in auto mode", () => {
    expect(policyProxyAssetIds(assets, "auto")).toEqual(["video-a", "video-b"]);
    expect(policyProxyAssetIds(assets, "off")).toEqual([]);
    expect(policyProxyAssetIds(undefined, "auto")).toEqual([]);
  });

  it("uses a ready proxy only for the exact source it was derived from", () => {
    const proxies = { "video-a": { proxyPath: "C:/cache/a_proxy.mp4", sourcePath: "D:/media/a.mp4" } };
    expect(resolvePreviewVideoPath("video-a", "D:/media/a.mp4", proxies)).toBe("C:/cache/a_proxy.mp4");
    // After a relink the asset path changes; the old proxy must not be shown.
    expect(resolvePreviewVideoPath("video-a", "D:/media/a-relinked.mp4", proxies)).toBe("D:/media/a-relinked.mp4");
    expect(resolvePreviewVideoPath("video-b", "D:/media/b.mp4", proxies)).toBe("D:/media/b.mp4");
    expect(resolvePreviewVideoPath(null, "D:/media/c.mp4", proxies)).toBe("D:/media/c.mp4");
  });

  it("stops polling ready proxies and unavailable policy proxies but keeps codec fallbacks", () => {
    const polled = proxyAssetIdsToPoll(
      ["fallback-unavailable", "fallback-pending"],
      ["policy-pending", "policy-ready", "policy-unavailable", "fallback-pending"],
      new Set(["policy-ready"]),
      new Set(["policy-unavailable", "fallback-unavailable"])
    );
    expect(polled.sort()).toEqual(["fallback-pending", "fallback-unavailable", "policy-pending"]);
  });
});
