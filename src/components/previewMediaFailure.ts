export type PreviewMediaFailurePresentation = {
  title: string;
  detail: string;
};

export type VideoPreviewFailure = {
  sourcePath: string | null;
  message: string;
};

export function currentVideoPreviewFailure(
  failure: VideoPreviewFailure | undefined,
  sourcePath?: string | null
): string | null {
  return failure && failure.sourcePath === (sourcePath ?? null) ? failure.message : null;
}

export function hasPendingCompatiblePreview(
  requestedAssetIds: string[],
  readyAssetIds: ReadonlySet<string>,
  failedAssetIds: ReadonlySet<string>
): boolean {
  return requestedAssetIds.some(
    (assetId) => !readyAssetIds.has(assetId) && !failedAssetIds.has(assetId)
  );
}

export function previewMediaFailurePresentation(input: {
  isMissingMedia: boolean;
  isVideo: boolean;
  sourcePath?: string | null;
  imageFailed?: boolean;
  videoPreviewFailure?: string | null;
}): PreviewMediaFailurePresentation | null {
  if (input.isMissingMedia) {
    return {
      title: "Offline / Missing Media",
      detail: input.sourcePath || "Unknown source path",
    };
  }

  if (input.isVideo && input.videoPreviewFailure) {
    return {
      title: "Video preview unavailable",
      detail: input.videoPreviewFailure,
    };
  }

  if (input.imageFailed) {
    return {
      title: "Image preview unavailable",
      detail: "The image could not be loaded or decoded.",
    };
  }

  return null;
}
