import { describe, expect, it } from "vitest";
import { dbToLinearGain, formatGainDb, linearGainToDb } from "./audioGain";

describe("audio gain conversion", () => {
  it("keeps unity at 0 dB and converts half amplitude", () => {
    expect(dbToLinearGain(0)).toBeCloseTo(1);
    expect(linearGainToDb(0.5)).toBeCloseTo(-6.0206, 3);
  });
  it("maps mute and caps positive gain", () => {
    expect(dbToLinearGain(-60)).toBe(0);
    expect(dbToLinearGain(12)).toBeLessThanOrEqual(2);
    expect(formatGainDb(0)).toBe("−∞ dB");
  });
  it("round trips representative fader positions", () => {
    for (const db of [-48, -18, -6, 0, 6]) expect(linearGainToDb(dbToLinearGain(db))).toBeCloseTo(db, 5);
  });
});
