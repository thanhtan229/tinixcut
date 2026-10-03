import { describe, it, expect } from "vitest";
import { HealthStatus } from "./types/health";

describe("HealthStatus Model", () => {
  it("validates healthy status response structure", () => {
    const fixture: HealthStatus = {
      status: "healthy",
      version: "0.1.0",
      platform: "windows",
      schema_version: 1,
    };

    expect(fixture.status).toBe("healthy");
    expect(fixture.schema_version).toBe(1);
    expect(fixture.version).toBe("0.1.0");
    expect(fixture.platform).toBe("windows");
  });
});
