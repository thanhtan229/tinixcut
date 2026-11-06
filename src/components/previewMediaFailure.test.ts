import { describe, expect, it } from "vitest";
import { currentVideoPreviewFailure, hasPendingCompatiblePreview, previewMediaFailurePresentation } from "./previewMediaFailure";

describe("currentVideoPreviewFailure", () => {
  it("keeps a failure for the source that produced it", () => {
    expect(currentVideoPreviewFailure({
      sourcePath: "D:/media/old.mp4",
      message: "proxy failed",
    }, "D:/media/old.mp4")).toBe("proxy failed");
  });

  it("ignores a stale failure after the clip is relinked", () => {
    expect(currentVideoPreviewFailure({
      sourcePath: "D:/media/old.mp4",
      message: "proxy failed",
    }, "D:/media/new.mp4")).toBeNull();
  });
});

describe("hasPendingCompatiblePreview", () => {
  it("stays pending until each requested asset is ready or has failed", () => {
    expect(hasPendingCompatiblePreview(["a", "b"], new Set(["a"]), new Set())).toBe(true);
  });

  it("stops the preparing state when proxy generation fails", () => {
    expect(hasPendingCompatiblePreview(["a"], new Set(), new Set(["a"]))).toBe(false);
  });

  it("stops the preparing state when a compatible source is ready", () => {
    expect(hasPendingCompatiblePreview(["a"], new Set(["a"]), new Set())).toBe(false);
  });
});

describe("previewMediaFailurePresentation", () => {
  it("reports offline media only when the source is actually missing", () => {
    expect(previewMediaFailurePresentation({
      isMissingMedia: true,
      isVideo: true,
      sourcePath: "D:/media/missing.mp4",
      videoPreviewFailure: "The WebView could not decode the source.",
    })).toEqual({
      title: "Offline / Missing Media",
      detail: "D:/media/missing.mp4",
    });
  });

  it("distinguishes an existing source with an unavailable video preview", () => {
    expect(previewMediaFailurePresentation({
      isMissingMedia: false,
      isVideo: true,
      sourcePath: "D:/media/source.mp4",
      videoPreviewFailure: "Proxy generation failed: unsupported codec",
    })).toEqual({
      title: "Video preview unavailable",
      detail: "Proxy generation failed: unsupported codec",
    });
  });

  it("does not display a failure while a compatible preview is still pending", () => {
    expect(previewMediaFailurePresentation({
      isMissingMedia: false,
      isVideo: true,
    })).toBeNull();
  });

  it("keeps image decode failures separate from missing files", () => {
    expect(previewMediaFailurePresentation({
      isMissingMedia: false,
      isVideo: false,
      imageFailed: true,
    })).toEqual({
      title: "Image preview unavailable",
      detail: "The image could not be loaded or decoded.",
    });
  });
});
