import { describe, expect, it } from "vitest";
import { MEDIA_FILE_EXTENSIONS } from "./mediaFileTypes";

describe("media file picker filters", () => {
  it("includes all supported audio containers, including M4A", () => {
    expect(MEDIA_FILE_EXTENSIONS).toEqual(
      expect.arrayContaining(["mp3", "wav", "aac", "flac", "ogg", "m4a"])
    );
  });
});
