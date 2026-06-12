import { describe, expect, it } from "vitest";
import {
  GRAPH_SERVER_PORT,
  graphServerSpawnCandidates,
  graphServerUrl,
} from "./serverConfig";

describe("desktop graph-server integration", () => {
  it("launches server on port 9847", () => {
    expect(GRAPH_SERVER_PORT).toBe(9847);
    expect(graphServerUrl()).toBe("http://127.0.0.1:9847");
  });

  it("defines spawn candidates for bundled and dev binaries", () => {
    const candidates = graphServerSpawnCandidates();
    expect(candidates).toContain("graph-server");
    expect(candidates.some((c) => c.includes("target/debug"))).toBe(true);
  });
});
